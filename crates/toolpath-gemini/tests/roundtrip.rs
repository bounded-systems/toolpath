//! Round-trip fidelity guarantees.
//!
//! These tests verify that `ChatFile` → JSON → `ChatFile` → JSON
//! produces byte-level-equivalent JSON (after key sorting) to the
//! original source. This is the contract the crate offers for lossless
//! re-export of Gemini CLI conversation logs.
//!
//! If a future schema change introduces a field we silently drop, one
//! of these fixtures will fail and flag the regression.

use std::collections::BTreeMap;
use toolpath_gemini::ChatFile;

const FIXTURES: &[(&str, &str)] = &[
    (
        "sample_subagent",
        include_str!("fixtures/sample_subagent.json"),
    ),
    (
        "sample_main_with_tools",
        include_str!("fixtures/sample_main_with_tools.json"),
    ),
    (
        "sample_main_with_subagent_ref",
        include_str!("fixtures/sample_main_with_subagent_ref.json"),
    ),
];

/// Recursively sort object keys so JSON objects compare by structure,
/// not iteration order. Arrays stay in order (position is meaningful).
fn canonicalize(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(m) => {
            let mut sorted: BTreeMap<String, serde_json::Value> = BTreeMap::new();
            for (k, val) in m {
                sorted.insert(k.clone(), canonicalize(val));
            }
            let mut out = serde_json::Map::new();
            for (k, val) in sorted {
                out.insert(k, val);
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(a) => {
            serde_json::Value::Array(a.iter().map(canonicalize).collect())
        }
        _ => v.clone(),
    }
}

fn first_diff(
    a: &serde_json::Value,
    b: &serde_json::Value,
    path: &mut Vec<String>,
) -> Option<String> {
    match (a, b) {
        (serde_json::Value::Object(am), serde_json::Value::Object(bm)) => {
            let ak: std::collections::BTreeSet<_> = am.keys().collect();
            let bk: std::collections::BTreeSet<_> = bm.keys().collect();
            if let Some(k) = ak.difference(&bk).next() {
                return Some(format!("[{}]: dropped field '{}'", path.join("/"), k));
            }
            if let Some(k) = bk.difference(&ak).next() {
                return Some(format!("[{}]: added field '{}'", path.join("/"), k));
            }
            for k in ak.intersection(&bk) {
                if am[*k] != bm[*k] {
                    path.push((*k).clone());
                    if let Some(d) = first_diff(&am[*k], &bm[*k], path) {
                        return Some(d);
                    }
                    path.pop();
                }
            }
            None
        }
        (serde_json::Value::Array(aa), serde_json::Value::Array(ba)) => {
            if aa.len() != ba.len() {
                return Some(format!(
                    "[{}]: length {} vs {}",
                    path.join("/"),
                    aa.len(),
                    ba.len()
                ));
            }
            for (i, (av, bv)) in aa.iter().zip(ba.iter()).enumerate() {
                if av != bv {
                    path.push(i.to_string());
                    if let Some(d) = first_diff(av, bv, path) {
                        return Some(d);
                    }
                    path.pop();
                }
            }
            None
        }
        _ => {
            if a != b {
                let sa = serde_json::to_string(a).unwrap_or_default();
                let sb = serde_json::to_string(b).unwrap_or_default();
                Some(format!(
                    "[{}]: {} vs {}",
                    path.join("/"),
                    truncate(&sa, 80),
                    truncate(&sb, 80)
                ))
            } else {
                None
            }
        }
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}...", &s[..n])
    }
}

fn assert_roundtrip(name: &str, original: &str) {
    let orig_value: serde_json::Value =
        serde_json::from_str(original).expect("fixture must be valid JSON");
    let chat: ChatFile = serde_json::from_str(original).expect("fixture must parse as ChatFile");
    let reserialized = serde_json::to_string(&chat).expect("must serialize");
    let back_value: serde_json::Value =
        serde_json::from_str(&reserialized).expect("reserialized must be valid JSON");

    let a = canonicalize(&orig_value);
    let b = canonicalize(&back_value);
    if a != b {
        let mut path = Vec::new();
        let diff = first_diff(&a, &b, &mut path).unwrap_or_else(|| "<unknown>".to_string());
        panic!("round-trip diverged for fixture {}: {}", name, diff);
    }
}

#[test]
fn all_fixtures_roundtrip_losslessly() {
    for (name, body) in FIXTURES {
        assert_roundtrip(name, body);
    }
}

#[test]
fn roundtrip_preserves_absent_directories() {
    // Source has no `directories` key — we must not inject an empty array.
    let src = r#"{"sessionId":"x","projectHash":"","messages":[]}"#;
    let chat: ChatFile = serde_json::from_str(src).unwrap();
    let out = serde_json::to_string(&chat).unwrap();
    assert!(
        !out.contains("directories"),
        "absent field should stay absent, got: {}",
        out
    );
}

#[test]
fn roundtrip_preserves_empty_directories_array() {
    // Source has explicit `"directories": []` — we must round-trip that
    // empty array, not drop it.
    let src = r#"{"sessionId":"x","projectHash":"","directories":[],"messages":[]}"#;
    let chat: ChatFile = serde_json::from_str(src).unwrap();
    assert!(
        chat.directories.is_some(),
        "Some(vec![]) distinguishes from None"
    );
    let out = serde_json::to_string(&chat).unwrap();
    assert!(out.contains("\"directories\":[]"), "got: {}", out);
}

#[test]
fn roundtrip_preserves_absent_thoughts() {
    let src = r#"{"sessionId":"x","projectHash":"","messages":[
        {"id":"m","timestamp":"ts","type":"user","content":"hi"}
    ]}"#;
    let chat: ChatFile = serde_json::from_str(src).unwrap();
    let out = serde_json::to_string(&chat).unwrap();
    assert!(!out.contains("\"thoughts\""), "got: {}", out);
}

#[test]
fn roundtrip_preserves_empty_thoughts_array() {
    let src = r#"{"sessionId":"x","projectHash":"","messages":[
        {"id":"m","timestamp":"ts","type":"user","content":"hi","thoughts":[]}
    ]}"#;
    let chat: ChatFile = serde_json::from_str(src).unwrap();
    let out = serde_json::to_string(&chat).unwrap();
    assert!(out.contains("\"thoughts\":[]"), "got: {}", out);
}

#[test]
fn roundtrip_preserves_structured_result_display() {
    // resultDisplay can be an array of styled-text records or a dict with
    // fileDiff — both must round-trip.
    let src = r#"{"sessionId":"x","projectHash":"","messages":[
        {"id":"m","timestamp":"ts","type":"gemini","content":"","toolCalls":[
            {"id":"t1","name":"write_file","args":{"file_path":"a"},"status":"success","timestamp":"ts","resultDisplay":{"fileDiff":"@@ -0,0 +1 @@\n+x"}}
        ]}
    ]}"#;
    let chat: ChatFile = serde_json::from_str(src).unwrap();
    let out = serde_json::to_string(&chat).unwrap();
    assert!(out.contains("fileDiff"), "got: {}", out);
}

#[test]
fn roundtrip_preserves_unknown_role() {
    // Future-proof: if Gemini adds a new type like "plan", we preserve it.
    let src = r#"{"sessionId":"x","projectHash":"","messages":[
        {"id":"m","timestamp":"ts","type":"plan","content":"something"}
    ]}"#;
    let chat: ChatFile = serde_json::from_str(src).unwrap();
    let out = serde_json::to_string(&chat).unwrap();
    assert!(out.contains("\"type\":\"plan\""), "got: {}", out);
}

#[test]
fn roundtrip_preserves_top_level_extras() {
    // Any unknown top-level field should survive via serde(flatten) + extra.
    let src = r#"{"sessionId":"x","projectHash":"","messages":[],"futureField":{"a":1}}"#;
    let chat: ChatFile = serde_json::from_str(src).unwrap();
    let out = serde_json::to_string(&chat).unwrap();
    assert!(out.contains("futureField"), "got: {}", out);
}

#[test]
fn roundtrip_preserves_message_extras() {
    let src = r#"{"sessionId":"x","projectHash":"","messages":[
        {"id":"m","timestamp":"ts","type":"gemini","content":"","futureMessageField":42}
    ]}"#;
    let chat: ChatFile = serde_json::from_str(src).unwrap();
    let out = serde_json::to_string(&chat).unwrap();
    assert!(out.contains("futureMessageField"), "got: {}", out);
}

//! Wire-level round-trip fidelity guarantees.
//!
//! These tests verify `JSON → RolloutLine → JSON` produces
//! byte-equivalent JSON (after key canonicalization) to the original
//! source. This is the contract the crate offers for lossless re-export
//! of Codex CLI rollout files.
//!
//! Complements `fidelity.rs` (which asserts that source facts survive
//! derivation into a Toolpath `Path`). This file asserts the *lower*
//! layer: that the on-disk schema round-trips without silent field
//! drops or variant loss. If OpenAI adds a new rollout kind, a new
//! `response_item` variant, or a new field on any of the existing
//! structs, one of these tests will flag the regression.

use std::collections::BTreeMap;

use toolpath_codex::{
    CustomToolCall, CustomToolCallOutput, EventMsg, ExecCommandEnd, FunctionCall,
    FunctionCallOutput, Message, PatchApplyEnd, Reasoning, ResponseItem, RolloutItem, RolloutLine,
    SessionMeta, TokenCountEvent, TurnContext,
};

// ── Canonicalized JSON diff helpers ────────────────────────────────

/// Canonicalize a JSON value for semantic round-trip comparison:
/// sort object keys and drop object entries that the typed layer
/// treats as the default (null, empty string, empty array, empty
/// object). Array positions are significant and preserved.
///
/// Why drop defaults. Codex rollouts routinely emit `"field": null`,
/// `"field": ""`, or `"field": []` for fields whose typed
/// representation is `Option<T>` / `String` / `Vec<T>` with a
/// `skip_serializing_if` attribute. Our types uniformly drop these
/// on serialize when the parsed value is the default, so an explicit
/// default in source re-serializes as absent. Both deserialize to
/// the same value, so for round-trip purposes they are equivalent.
///
/// What we do NOT drop: `0`, `false`, or any non-default scalar —
/// those carry information and their absence would be a real
/// regression.
fn canonicalize(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(m) => {
            let mut sorted: BTreeMap<String, serde_json::Value> = BTreeMap::new();
            for (k, val) in m {
                if is_default_like(val) {
                    continue;
                }
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

fn is_default_like(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Null => true,
        serde_json::Value::String(s) => s.is_empty(),
        serde_json::Value::Array(a) => a.is_empty(),
        serde_json::Value::Object(o) => o.is_empty(),
        _ => false,
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
                    truncate(&sa, 120),
                    truncate(&sb, 120)
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

fn assert_value_eq(idx: usize, label: &str, orig: &str, got: &str) {
    let a: serde_json::Value = serde_json::from_str(orig).expect("orig must be JSON");
    let b: serde_json::Value = serde_json::from_str(got).expect("got must be JSON");
    let a = canonicalize(&a);
    let b = canonicalize(&b);
    if a != b {
        let mut path = Vec::new();
        let diff = first_diff(&a, &b, &mut path).unwrap_or_else(|| "<unknown>".to_string());
        panic!(
            "line {} {} round-trip diverged: {}\n  original: {}\n  got:      {}",
            idx,
            label,
            diff,
            truncate(orig, 200),
            truncate(got, 200)
        );
    }
}

fn assert_line_roundtrip(idx: usize, original: &str) {
    // (1) Outer wrapper: JSON → RolloutLine → JSON.
    let line: RolloutLine = serde_json::from_str(original)
        .unwrap_or_else(|e| panic!("line {} does not parse as RolloutLine: {}", idx, e));
    let outer = serde_json::to_string(&line).expect("must serialize");
    assert_value_eq(idx, "outer", original, &outer);

    // (2) Inner payload: parse the payload into its typed variant by
    // `kind`, re-serialize, and compare against the original raw
    // payload. The outer test alone can't catch a field drop inside
    // Message/FunctionCall/etc., because RolloutLine stores `payload`
    // as a `Value` and passes it through untouched during serde.
    let payload_json = serde_json::to_string(&line.payload).expect("payload value must serialize");
    match line.kind.as_str() {
        "session_meta" => {
            let typed: SessionMeta = serde_json::from_value(line.payload.clone())
                .unwrap_or_else(|e| panic!("line {} payload not SessionMeta: {}", idx, e));
            let back = serde_json::to_string(&typed).unwrap();
            assert_value_eq(idx, "session_meta payload", &payload_json, &back);
        }
        "turn_context" => {
            let typed: TurnContext = serde_json::from_value(line.payload.clone())
                .unwrap_or_else(|e| panic!("line {} payload not TurnContext: {}", idx, e));
            let back = serde_json::to_string(&typed).unwrap();
            assert_value_eq(idx, "turn_context payload", &payload_json, &back);
        }
        "response_item" => roundtrip_response_item(idx, &line.payload, &payload_json),
        "event_msg" => roundtrip_event_msg(idx, &line.payload, &payload_json),
        // session_state / compacted are wrapped as raw Value; no typed
        // round-trip to perform. Unknown kinds fall through to the
        // outer check, which already verified the payload is preserved.
        _ => {}
    }
}

// ── Typed-payload dispatch ─────────────────────────────────────────
//
// `ResponseItem` and `EventMsg` are Deserialize-only sum types. To
// verify their inner *structs* round-trip (Message, FunctionCall,
// ExecCommandEnd, …), we dispatch on `payload.type` ourselves and
// round-trip the concrete struct directly. Variants we can't
// exercise (Other catch-all, opaque Value-wrapped events) are
// covered by the outer RolloutLine comparison, which already
// preserves `payload` as raw JSON.

fn roundtrip_response_item(idx: usize, payload: &serde_json::Value, payload_json: &str) {
    let kind = payload
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or_default();
    let back = match kind {
        "message" => {
            let v: Message = serde_json::from_value(payload.clone())
                .unwrap_or_else(|e| panic!("line {} Message parse: {}", idx, e));
            serde_json::to_string(&v).unwrap()
        }
        "reasoning" => {
            let v: Reasoning = serde_json::from_value(payload.clone())
                .unwrap_or_else(|e| panic!("line {} Reasoning parse: {}", idx, e));
            serde_json::to_string(&v).unwrap()
        }
        "function_call" => {
            let v: FunctionCall = serde_json::from_value(payload.clone())
                .unwrap_or_else(|e| panic!("line {} FunctionCall parse: {}", idx, e));
            serde_json::to_string(&v).unwrap()
        }
        "function_call_output" => {
            let v: FunctionCallOutput = serde_json::from_value(payload.clone())
                .unwrap_or_else(|e| panic!("line {} FunctionCallOutput parse: {}", idx, e));
            serde_json::to_string(&v).unwrap()
        }
        "custom_tool_call" => {
            let v: CustomToolCall = serde_json::from_value(payload.clone())
                .unwrap_or_else(|e| panic!("line {} CustomToolCall parse: {}", idx, e));
            serde_json::to_string(&v).unwrap()
        }
        "custom_tool_call_output" => {
            let v: CustomToolCallOutput = serde_json::from_value(payload.clone())
                .unwrap_or_else(|e| panic!("line {} CustomToolCallOutput parse: {}", idx, e));
            serde_json::to_string(&v).unwrap()
        }
        // Unknown response_item.type — `ResponseItem::Other` keeps
        // payload verbatim, covered by outer check.
        _ => return,
    };
    // Sanity: confirm from_value classifies this line as a typed
    // variant, not Other — catches silent downgrades where a field
    // rename or new required field makes parsing fail and fall
    // through to the catch-all.
    assert!(
        !matches!(
            ResponseItem::from_value(payload),
            ResponseItem::Other { .. }
        ),
        "line {}: response_item payload fell through to Other — deserializer regressed",
        idx
    );
    assert_value_eq(idx, &format!("response_item.{}", kind), payload_json, &back);
}

fn roundtrip_event_msg(idx: usize, payload: &serde_json::Value, payload_json: &str) {
    let kind = payload
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or_default();
    let back = match kind {
        "token_count" => {
            let v: TokenCountEvent = serde_json::from_value(payload.clone())
                .unwrap_or_else(|e| panic!("line {} TokenCountEvent parse: {}", idx, e));
            serde_json::to_string(&v).unwrap()
        }
        "exec_command_end" => {
            let v: ExecCommandEnd = serde_json::from_value(payload.clone())
                .unwrap_or_else(|e| panic!("line {} ExecCommandEnd parse: {}", idx, e));
            serde_json::to_string(&v).unwrap()
        }
        "patch_apply_end" => {
            let v: PatchApplyEnd = serde_json::from_value(payload.clone())
                .unwrap_or_else(|e| panic!("line {} PatchApplyEnd parse: {}", idx, e));
            serde_json::to_string(&v).unwrap()
        }
        // task_started / task_complete / user_message / agent_message
        // are wrapped as raw `Value` by EventMsg, so the outer wrapper
        // already verifies their round-trip. Unknown kinds fall into
        // EventMsg::Other (also outer-verified).
        _ => return,
    };
    assert_value_eq(idx, &format!("event_msg.{}", kind), payload_json, &back);
}

// ── Whole-fixture round-trip ───────────────────────────────────────

const SAMPLE: &str = include_str!("fixtures/sample-codex-python.jsonl");

#[test]
fn fixture_roundtrips_losslessly() {
    // The 138-line real-session fixture must survive
    // `JSON → RolloutLine → JSON` unchanged, line by line.
    let mut count = 0;
    for (idx, line) in SAMPLE.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        assert_line_roundtrip(idx, line);
        count += 1;
    }
    assert_eq!(count, 138, "expected 138 non-empty lines in sample fixture");
}

// ── Unknown-variant preservation ───────────────────────────────────

#[test]
fn unknown_rollout_kind_survives() {
    // Future-proof: if Codex adds a new top-level `type`, it must
    // survive via RolloutItem::Other with the original payload intact.
    let src = r#"{"timestamp":"2026-04-20T16:44:37.773Z","type":"future_kind","payload":{"hello":"world","nested":{"a":1}}}"#;
    let line: RolloutLine = serde_json::from_str(src).unwrap();
    assert_eq!(line.kind, "future_kind");
    match line.item() {
        RolloutItem::Unknown { kind, payload } => {
            assert_eq!(kind, "future_kind");
            assert_eq!(payload.get("hello").and_then(|v| v.as_str()), Some("world"));
            assert_eq!(
                payload
                    .get("nested")
                    .and_then(|v| v.get("a"))
                    .and_then(|v| v.as_i64()),
                Some(1)
            );
        }
        other => panic!("expected RolloutItem::Unknown, got {:?}", other),
    }
    let out = serde_json::to_string(&line).unwrap();
    let orig: serde_json::Value = serde_json::from_str(src).unwrap();
    let back: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(canonicalize(&orig), canonicalize(&back), "got: {}", out);
}

#[test]
fn unknown_response_item_type_survives() {
    // Future-proof: if OpenAI adds a new `response_item.type`, it must
    // survive via ResponseItem::Other.
    let src = r#"{"timestamp":"ts","type":"response_item","payload":{"type":"future_response","custom":"payload","nested":{"a":1}}}"#;
    let line: RolloutLine = serde_json::from_str(src).unwrap();
    match line.item() {
        RolloutItem::ResponseItem(ResponseItem::Other { kind, payload }) => {
            assert_eq!(kind, "future_response");
            assert_eq!(
                payload.get("custom").and_then(|v| v.as_str()),
                Some("payload")
            );
        }
        other => panic!("expected ResponseItem::Other, got {:?}", other),
    }
    let out = serde_json::to_string(&line).unwrap();
    let orig: serde_json::Value = serde_json::from_str(src).unwrap();
    let back: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(canonicalize(&orig), canonicalize(&back), "got: {}", out);
}

#[test]
fn unknown_event_msg_type_survives() {
    // Same guarantee for event_msg.
    let src = r#"{"timestamp":"ts","type":"event_msg","payload":{"type":"future_event","detail":"whatever"}}"#;
    let line: RolloutLine = serde_json::from_str(src).unwrap();
    match line.item() {
        RolloutItem::EventMsg(EventMsg::Other { kind, payload }) => {
            assert_eq!(kind, "future_event");
            assert_eq!(
                payload.get("detail").and_then(|v| v.as_str()),
                Some("whatever")
            );
        }
        other => panic!("expected EventMsg::Other, got {:?}", other),
    }
    let out = serde_json::to_string(&line).unwrap();
    let orig: serde_json::Value = serde_json::from_str(src).unwrap();
    let back: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(canonicalize(&orig), canonicalize(&back), "got: {}", out);
}

// ── Unknown-field preservation (forward-compat via flatten extras) ─

#[test]
fn rollout_line_top_level_extras_survive() {
    let src = r#"{"timestamp":"ts","type":"session_meta","payload":{"id":"s","timestamp":"t","cwd":"/p","originator":"x","cli_version":"1","source":"cli"},"future_line_field":{"a":1}}"#;
    let line: RolloutLine = serde_json::from_str(src).unwrap();
    let out = serde_json::to_string(&line).unwrap();
    assert!(
        out.contains("future_line_field"),
        "unknown top-level field dropped: {}",
        out
    );
}

#[test]
fn message_extras_survive() {
    // Unknown fields on a Message (response_item.message) must survive.
    let src = r#"{"role":"assistant","content":[{"type":"output_text","text":"hello"}],"future_msg_field":42}"#;
    let msg: Message = serde_json::from_str(src).unwrap();
    let out = serde_json::to_string(&msg).unwrap();
    assert!(
        out.contains("future_msg_field"),
        "unknown message field dropped: {}",
        out
    );
}

#[test]
fn function_call_extras_survive() {
    let src = r#"{"name":"exec","arguments":"{}","call_id":"c1","future_fc_field":"preserved"}"#;
    let fc: FunctionCall = serde_json::from_str(src).unwrap();
    let out = serde_json::to_string(&fc).unwrap();
    assert!(
        out.contains("future_fc_field"),
        "unknown function_call field dropped: {}",
        out
    );
}

// ── Byte-preservation of model-emitted strings ─────────────────────

#[test]
fn function_call_arguments_preserved_byte_for_byte() {
    // The docstring on `FunctionCall::arguments` promises the raw
    // string is kept verbatim — some models emit malformed JSON, and
    // we want byte-equivalent round-trip regardless. Verify with a
    // deliberately malformed payload (trailing comma) and whitespace
    // the parser would strip if we ever switched to a Value.
    let malformed = r#"{"cmd": "pwd", "trailing":,}"#;
    let src = format!(
        r#"{{"name":"exec_command","arguments":{},"call_id":"c1"}}"#,
        serde_json::to_string(malformed).unwrap()
    );
    let fc: FunctionCall = serde_json::from_str(&src).unwrap();
    assert_eq!(fc.arguments, malformed);
    let out = serde_json::to_string(&fc).unwrap();
    let back: FunctionCall = serde_json::from_str(&out).unwrap();
    assert_eq!(back.arguments, malformed, "arguments not byte-preserved");
}

#[test]
fn function_call_empty_arguments_preserved() {
    // Some function calls emit `"arguments":""` — not `"{}"`. Must
    // survive, since dropping an empty string could change semantics
    // (missing-arg vs explicit-empty-arg).
    let src = r#"{"name":"noop","arguments":"","call_id":"c1"}"#;
    let fc: FunctionCall = serde_json::from_str(src).unwrap();
    assert_eq!(fc.arguments, "");
    let out = serde_json::to_string(&fc).unwrap();
    assert!(out.contains("\"arguments\":\"\""), "got: {}", out);
}

// ── Payload variant hygiene ────────────────────────────────────────

#[test]
fn session_meta_with_only_required_fields_roundtrips() {
    // Optional fields (git, base_instructions, instructions) absent.
    // Must stay absent in the output — not resurrected as null or {}.
    let src = r#"{"timestamp":"ts","type":"session_meta","payload":{"id":"s","timestamp":"t","cwd":"/p","originator":"x","cli_version":"1","source":"cli"}}"#;
    let line: RolloutLine = serde_json::from_str(src).unwrap();
    let out = serde_json::to_string(&line).unwrap();
    assert!(
        !out.contains("\"git\""),
        "absent git field resurrected: {}",
        out
    );
    assert!(
        !out.contains("\"base_instructions\""),
        "absent base_instructions resurrected: {}",
        out
    );
    assert!(
        !out.contains("\"instructions\""),
        "absent instructions resurrected: {}",
        out
    );
}

#[test]
fn reasoning_without_encrypted_content_roundtrips() {
    // Plaintext-only reasoning (no `encrypted_content`) must not grow
    // a null or empty string on re-serialization.
    let src = r#"{"timestamp":"ts","type":"response_item","payload":{"type":"reasoning","summary":[{"type":"summary_text","text":"thought"}]}}"#;
    let line: RolloutLine = serde_json::from_str(src).unwrap();
    let out = serde_json::to_string(&line).unwrap();
    assert!(
        !out.contains("encrypted_content"),
        "absent encrypted_content resurrected: {}",
        out
    );
}

#[test]
fn patch_apply_end_with_empty_changes_roundtrips() {
    // `changes: {}` means "no files touched" (rare but possible).
    // Must round-trip as `{}`, not be dropped or become null.
    let src = r#"{"timestamp":"ts","type":"event_msg","payload":{"type":"patch_apply_end","call_id":"c","success":true,"changes":{}}}"#;
    let line: RolloutLine = serde_json::from_str(src).unwrap();
    let out = serde_json::to_string(&line).unwrap();
    assert!(out.contains("\"changes\":{}"), "got: {}", out);
}

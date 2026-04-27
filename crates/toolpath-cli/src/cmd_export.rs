//! `path export <target>` — emit toolpath documents into external formats.
//!
//! - `export claude` projects a Path document into Claude Code's JSONL
//!   format (either into `~/.claude/projects/<sanitized>/<session>.jsonl`
//!   for a resumable session, or to a file / stdout).
//! - `export gemini` projects a Path document into Gemini CLI's chat-file
//!   layout under `~/.gemini/tmp/<slot>/chats/`, ready for
//!   `gemini --resume <uuid>`.
//! - `export pathbase` uploads the document to a Pathbase server.

use anyhow::{Context, Result};
use clap::Subcommand;
use std::path::PathBuf;

use crate::cmd_cache::cache_ref;

#[derive(Subcommand, Debug)]
pub enum ExportTarget {
    /// Project a toolpath document into a Claude Code session
    Claude {
        /// Input: cache id (e.g. `claude-abc`) or path to a toolpath JSON file
        #[arg(short, long)]
        input: String,

        /// Target project directory. With this flag, writes the JSONL into
        /// `~/.claude/projects/<sanitized>/<session>.jsonl` so `claude -r <id>`
        /// can resume it. Defaults to cwd when no `--output` is given.
        #[arg(short, long)]
        project: Option<PathBuf>,

        /// Output JSONL to this file. Mutually exclusive with --project.
        #[arg(short, long, conflicts_with = "project")]
        output: Option<PathBuf>,
    },
    /// Project a toolpath document into a Gemini CLI session
    Gemini {
        /// Input: cache id (e.g. `claude-abc`) or path to a toolpath JSON file
        #[arg(short, long)]
        input: String,

        /// Target project directory. The session is written into
        /// `~/.gemini/tmp/<slot>/chats/` where `<slot>` is the
        /// `projects.json` friendly name when present, otherwise the
        /// SHA-256 hash of this directory's absolute path. `gemini`
        /// invoked from the same directory will find it via `--resume`.
        /// Defaults to cwd.
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
    /// Upload a toolpath document to Pathbase
    Pathbase {
        /// Input: cache id (e.g. `claude-abc`) or path to a toolpath JSON file
        #[arg(short, long)]
        input: String,

        /// Pathbase server URL (defaults to the stored session's server)
        #[arg(long)]
        url: Option<String>,
    },
}

pub fn run(target: ExportTarget) -> Result<()> {
    match target {
        ExportTarget::Claude {
            input,
            project,
            output,
        } => run_claude(input, project, output),
        ExportTarget::Gemini { input, project } => run_gemini(input, project),
        ExportTarget::Pathbase { input, url } => run_pathbase(input, url),
    }
}

fn run_claude(input: String, project: Option<PathBuf>, output: Option<PathBuf>) -> Result<()> {
    #[cfg(target_os = "emscripten")]
    {
        let _ = (input, project, output);
        anyhow::bail!("'path export claude' requires a native environment");
    }

    #[cfg(not(target_os = "emscripten"))]
    {
        let path = load_path_doc(&input)?;
        let conversation = build_claude_conversation(&path)?;
        let jsonl = serialize_jsonl(&conversation)?;

        match (project, output) {
            (Some(project_dir), None) => {
                let out_path = write_into_claude_project(&conversation, &jsonl, &project_dir)?;
                let session_id = &conversation.session_id;
                eprintln!(
                    "Exported session {} ({} entries) → {}",
                    session_id,
                    conversation.preamble.len() + conversation.entries.len(),
                    out_path.display()
                );
                eprintln!();
                eprintln!("Resume with:");
                eprintln!("  cd {} && claude -r {}", project_dir.display(), session_id);
            }
            (None, Some(out_path)) => {
                std::fs::write(&out_path, &jsonl)
                    .with_context(|| format!("write {}", out_path.display()))?;
                eprintln!("Wrote {} bytes to {}", jsonl.len(), out_path.display());
            }
            (None, None) => {
                println!("{}", jsonl);
            }
            (Some(_), Some(_)) => unreachable!("clap enforces conflicts_with"),
        }

        Ok(())
    }
}

#[cfg(not(target_os = "emscripten"))]
fn load_path_doc(input: &str) -> Result<toolpath::v1::Path> {
    let file = cache_ref(input)?;
    let json = std::fs::read_to_string(&file)
        .with_context(|| format!("Failed to read {}", file.display()))?;
    let doc: toolpath::v1::Document = serde_json::from_str(&json)
        .map_err(|e| anyhow::anyhow!("Failed to parse toolpath document: {}", e))?;
    match doc {
        toolpath::v1::Document::Path(p) => Ok(p),
        toolpath::v1::Document::Step(_) => {
            anyhow::bail!("Expected a Path document, got a Step")
        }
        toolpath::v1::Document::Graph(_) => {
            anyhow::bail!("Expected a Path document, got a Graph")
        }
    }
}

#[cfg(not(target_os = "emscripten"))]
fn build_claude_conversation(path: &toolpath::v1::Path) -> Result<toolpath_claude::Conversation> {
    use toolpath_convo::ConversationProjector;
    let view = toolpath_convo::extract_conversation(path);
    let projector = toolpath_claude::ClaudeProjector;
    projector
        .project(&view)
        .map_err(|e| anyhow::anyhow!("Projection failed: {}", e))
}

#[cfg(not(target_os = "emscripten"))]
fn serialize_jsonl(conv: &toolpath_claude::Conversation) -> Result<String> {
    let mut lines = Vec::with_capacity(conv.preamble.len() + conv.entries.len());
    for raw in &conv.preamble {
        lines.push(serde_json::to_string(raw)?);
    }
    for entry in &conv.entries {
        lines.push(serde_json::to_string(entry)?);
    }
    Ok(lines.join("\n"))
}

#[cfg(not(target_os = "emscripten"))]
fn write_into_claude_project(
    conv: &toolpath_claude::Conversation,
    jsonl: &str,
    project_dir: &std::path::Path,
) -> Result<PathBuf> {
    let project_dir = std::fs::canonicalize(project_dir)
        .with_context(|| format!("resolve project path {}", project_dir.display()))?;
    let project_path = project_dir.to_string_lossy();

    let resolver = toolpath_claude::PathResolver::new();
    let claude_project_dir = resolver
        .project_dir(&project_path)
        .map_err(|e| anyhow::anyhow!("Cannot resolve Claude project dir: {}", e))?;

    std::fs::create_dir_all(&claude_project_dir)
        .with_context(|| format!("create {}", claude_project_dir.display()))?;

    let session_id = &conv.session_id;
    let out_path = claude_project_dir.join(format!("{}.jsonl", session_id));
    std::fs::write(&out_path, jsonl).with_context(|| format!("write {}", out_path.display()))?;
    Ok(out_path)
}

// ── Gemini ────────────────────────────────────────────────────────────

fn run_gemini(input: String, project: Option<PathBuf>) -> Result<()> {
    #[cfg(target_os = "emscripten")]
    {
        let _ = (input, project);
        anyhow::bail!("'path export gemini' requires a native environment");
    }

    #[cfg(not(target_os = "emscripten"))]
    {
        use toolpath_convo::ConversationProjector;

        let path = load_path_doc(&input)?;
        let view = toolpath_convo::extract_conversation(&path);

        let project_dir = match project {
            Some(p) => std::fs::canonicalize(&p)
                .with_context(|| format!("resolve project path {}", p.display()))?,
            None => std::env::current_dir()?,
        };
        let project_path = project_dir.to_string_lossy().to_string();

        // The projector bakes `projectHash` and `directories` into the
        // emitted ChatFile so they match what Gemini computes when
        // invoked from the same cwd.
        let project_hash = toolpath_gemini::paths::project_hash(&project_path);
        let projector = toolpath_gemini::project::GeminiProjector::new()
            .with_project_hash(project_hash.clone())
            .with_project_path(project_path.clone());
        let conversation = projector
            .project(&view)
            .map_err(|e| anyhow::anyhow!("Projection failed: {}", e))?;

        if conversation.session_uuid.is_empty() {
            anyhow::bail!("Projected conversation has no session UUID — cannot place it on disk");
        }

        let resolver = toolpath_gemini::PathResolver::new();
        let chats_dir = resolver
            .chats_dir(&project_path)
            .map_err(|e| anyhow::anyhow!("Cannot resolve Gemini chats dir: {}", e))?;
        std::fs::create_dir_all(&chats_dir)
            .with_context(|| format!("create {}", chats_dir.display()))?;

        // Drop a `.project_root` marker so `list_project_dirs` and any
        // tooling that walks `tmp/` can pick us up even without a
        // `projects.json` entry.
        if let Some(slot_dir) = chats_dir.parent() {
            let marker = slot_dir.join(".project_root");
            if !marker.exists() {
                let _ = std::fs::write(&marker, format!("{}\n", project_path));
            }
        }

        // Layout: chats/session-<ts>-<short>.json (flat main file, the
        // `session-` prefix is mandatory so `--list-sessions` finds it),
        // plus chats/<session-uuid>/<sub>.json for any sub-agents.
        let main_stem = gemini_main_stem(&conversation);
        let main_path = chats_dir.join(format!("{}.json", main_stem));
        std::fs::write(
            &main_path,
            serde_json::to_string_pretty(&conversation.main)?,
        )
        .with_context(|| format!("write {}", main_path.display()))?;
        let mut written: Vec<PathBuf> = vec![main_path];

        if !conversation.sub_agents.is_empty() {
            let sub_dir = chats_dir.join(&conversation.session_uuid);
            std::fs::create_dir_all(&sub_dir)
                .with_context(|| format!("create {}", sub_dir.display()))?;
            for (i, sub) in conversation.sub_agents.iter().enumerate() {
                let stem = if sub.session_id.is_empty() {
                    format!("subagent-{}", i)
                } else {
                    sub.session_id.clone()
                };
                let sub_path = sub_dir.join(format!("{}.json", stem));
                std::fs::write(&sub_path, serde_json::to_string_pretty(sub)?)
                    .with_context(|| format!("write {}", sub_path.display()))?;
                written.push(sub_path);
            }
        }

        let total_messages = conversation.main.messages.len()
            + conversation
                .sub_agents
                .iter()
                .map(|s| s.messages.len())
                .sum::<usize>();
        let sub_n = conversation.sub_agents.len();
        eprintln!(
            "Exported Gemini session {} ({} messages across main + {} sub-agent{}) → {}",
            conversation.session_uuid,
            total_messages,
            sub_n,
            if sub_n == 1 { "" } else { "s" },
            chats_dir.display()
        );
        for path in &written {
            eprintln!("  wrote {}", path.display());
        }
        eprintln!();
        eprintln!("Resume with:");
        eprintln!(
            "  cd {} && gemini --resume {}",
            project_path, conversation.session_uuid
        );
        Ok(())
    }
}

/// On-disk stem for a Gemini main chat file:
/// `session-<YYYY-MM-DDTHH-MM>-<first8-of-uuid>`.
///
/// The `session-` prefix is mandatory — Gemini CLI's `--list-sessions`
/// filters on it before opening any file, so a file without it is
/// invisible to `--resume`. The short suffix matches Gemini's own
/// naming convention. Falls back to `session-<uuid>` if no timestamp is
/// available on the projected conversation.
#[cfg(not(target_os = "emscripten"))]
fn gemini_main_stem(convo: &toolpath_gemini::types::Conversation) -> String {
    let short: String = convo.session_uuid.chars().take(8).collect();
    let ts = convo
        .started_at
        .or(convo.last_activity)
        .or(convo.main.start_time)
        .or(convo.main.last_updated);
    match ts {
        Some(t) => format!("session-{}-{}", t.format("%Y-%m-%dT%H-%M"), short),
        None => format!("session-{}", convo.session_uuid),
    }
}

// ── Pathbase ──────────────────────────────────────────────────────────

fn run_pathbase(input: String, url_flag: Option<String>) -> Result<()> {
    #[cfg(target_os = "emscripten")]
    {
        let _ = (input, url_flag);
        anyhow::bail!("'path export pathbase' requires a native environment with network access");
    }

    #[cfg(not(target_os = "emscripten"))]
    {
        use crate::cmd_pathbase::{require_session, resolve_url, traces_post};

        let file = cache_ref(&input)?;
        let body = std::fs::read_to_string(&file)
            .with_context(|| format!("Failed to read {}", file.display()))?;
        // Validate locally so we give a clean error rather than relying on
        // the server to reject malformed payloads.
        toolpath::v1::Document::from_json(&body)
            .map_err(|e| anyhow::anyhow!("Invalid toolpath document: {}", e))?;

        let session = require_session()?;
        let base_url = match url_flag {
            Some(u) => resolve_url(Some(u)),
            None => session.url.clone(),
        };

        if host_of(&base_url) != host_of(&session.url) {
            eprintln!(
                "warning: uploading to {} with a token issued by {}; expect 401 unless this is the same deployment",
                base_url, session.url
            );
        }

        let trace = traces_post(&base_url, &session.token, &body)?;
        println!("{}", trace.url);
        eprintln!(
            "Uploaded {} → {} ({} bytes)",
            file.display(),
            trace.id,
            body.len()
        );
        Ok(())
    }
}

/// Extract `scheme://host[:port]` from a URL, dropping any path/query.
/// Returns the input unchanged if it doesn't look like a URL.
#[cfg(not(target_os = "emscripten"))]
fn host_of(url: &str) -> &str {
    let after_scheme = match url.find("://") {
        Some(i) => i + 3,
        None => return url,
    };
    // Find the next `/` after the scheme://; everything before it is host[:port].
    match url[after_scheme..].find('/') {
        Some(off) => &url[..after_scheme + off],
        None => url,
    }
}

#[cfg(all(test, not(target_os = "emscripten")))]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use toolpath::v1::{ArtifactChange, PathIdentity, Step, StepIdentity, StructuralChange};

    fn make_path_doc() -> toolpath::v1::Document {
        let artifact_key = "agent://claude/test-session";

        let init_step = Step {
            step: StepIdentity {
                id: "step-001".to_string(),
                parents: vec![],
                actor: "tool:claude-code".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
            },
            change: {
                let mut m = HashMap::new();
                m.insert(
                    artifact_key.to_string(),
                    ArtifactChange {
                        raw: None,
                        structural: Some(StructuralChange {
                            change_type: "conversation.init".to_string(),
                            extra: HashMap::new(),
                        }),
                    },
                );
                m
            },
            meta: None,
        };

        let append_step = Step {
            step: StepIdentity {
                id: "step-002".to_string(),
                parents: vec!["step-001".to_string()],
                actor: "human:user".to_string(),
                timestamp: "2024-01-01T00:00:01Z".to_string(),
            },
            change: {
                let mut m = HashMap::new();
                let mut extra = HashMap::new();
                extra.insert("role".to_string(), serde_json::json!("user"));
                extra.insert("text".to_string(), serde_json::json!("Hello"));
                m.insert(
                    artifact_key.to_string(),
                    ArtifactChange {
                        raw: None,
                        structural: Some(StructuralChange {
                            change_type: "conversation.append".to_string(),
                            extra,
                        }),
                    },
                );
                m
            },
            meta: None,
        };

        let path = toolpath::v1::Path {
            path: PathIdentity {
                id: "test-path".to_string(),
                base: None,
                head: "step-002".to_string(),
                graph_ref: None,
            },
            steps: vec![init_step, append_step],
            meta: None,
        };

        toolpath::v1::Document::Path(path)
    }

    #[test]
    fn claude_output_to_file() {
        let temp = tempfile::tempdir().unwrap();
        let input_path = temp.path().join("input.json");
        let output_path = temp.path().join("out.jsonl");

        let doc = make_path_doc();
        std::fs::write(&input_path, serde_json::to_string(&doc).unwrap()).unwrap();

        run_claude(
            input_path.to_string_lossy().to_string(),
            None,
            Some(output_path.clone()),
        )
        .unwrap();

        let out = std::fs::read_to_string(&output_path).unwrap();
        assert!(!out.is_empty());
        for line in out.lines() {
            serde_json::from_str::<serde_json::Value>(line).unwrap();
        }
    }

    #[test]
    fn claude_rejects_non_path_doc() {
        let temp = tempfile::tempdir().unwrap();
        let input_path = temp.path().join("input.json");
        let step = Step {
            step: StepIdentity {
                id: "s1".into(),
                parents: vec![],
                actor: "human:x".into(),
                timestamp: "2024-01-01T00:00:00Z".into(),
            },
            change: HashMap::new(),
            meta: None,
        };
        let doc = toolpath::v1::Document::Step(step);
        std::fs::write(&input_path, serde_json::to_string(&doc).unwrap()).unwrap();

        let err = run_claude(input_path.to_string_lossy().to_string(), None, None).unwrap_err();
        assert!(err.to_string().contains("Step"));
    }

    #[test]
    fn claude_invalid_json_errors() {
        let temp = tempfile::tempdir().unwrap();
        let input_path = temp.path().join("input.json");
        std::fs::write(&input_path, "not json").unwrap();
        let err = run_claude(input_path.to_string_lossy().to_string(), None, None).unwrap_err();
        assert!(err.to_string().contains("parse") || err.to_string().contains("Failed"));
    }

    #[test]
    fn host_of_strips_path() {
        assert_eq!(host_of("https://pathbase.dev"), "https://pathbase.dev");
        assert_eq!(host_of("https://pathbase.dev/"), "https://pathbase.dev");
        assert_eq!(
            host_of("https://pathbase.dev/api/v1/traces"),
            "https://pathbase.dev"
        );
        assert_eq!(
            host_of("http://127.0.0.1:9000/foo"),
            "http://127.0.0.1:9000"
        );
        assert_eq!(host_of("not-a-url"), "not-a-url");
    }

    #[test]
    fn gemini_writes_resume_ready_layout() {
        // End-to-end: a path doc whose conversation.append carries a
        // single user turn projects to a flat `session-*.json` file
        // under <slot>/chats/, with `kind: "main"` and the inner
        // `sessionId` matching the source UUID. Layout matches what
        // Gemini CLI's `--resume <uuid>` needs to find the session.
        use toolpath_gemini::{GeminiConvo, PathResolver};

        let temp = tempfile::tempdir().unwrap();
        let fake_home = temp.path().join("home");
        std::fs::create_dir_all(&fake_home).unwrap();
        let project_dir = temp.path().join("myproj");
        std::fs::create_dir_all(&project_dir).unwrap();

        // Build a minimal Path doc whose artifact key encodes the
        // session UUID we want to land in `sessionId`. The shared-derive
        // wire shape is `<provider>://<session-id>`, which extract
        // parses to populate `view.id` and `view.provider_id`.
        let session_uuid = "11111111-2222-3333-4444-555555555555";
        let artifact = format!("gemini-cli://{}", session_uuid);
        let mut extra = HashMap::new();
        extra.insert("role".into(), serde_json::json!("user"));
        extra.insert("text".into(), serde_json::json!("Hello from export"));
        let step = Step {
            step: StepIdentity {
                id: "step-001".into(),
                parents: vec![],
                actor: "human:alex".into(),
                timestamp: "2026-04-17T15:00:00Z".into(),
            },
            change: {
                let mut m = HashMap::new();
                m.insert(
                    artifact,
                    ArtifactChange {
                        raw: None,
                        structural: Some(StructuralChange {
                            change_type: "conversation.append".into(),
                            extra,
                        }),
                    },
                );
                m
            },
            meta: None,
        };
        let doc = toolpath::v1::Document::Path(toolpath::v1::Path {
            path: PathIdentity {
                id: "test-path".into(),
                base: None,
                head: "step-001".into(),
                graph_ref: None,
            },
            steps: vec![step],
            meta: None,
        });
        let input_path = temp.path().join("doc.json");
        std::fs::write(&input_path, serde_json::to_string(&doc).unwrap()).unwrap();

        // Override HOME so `PathResolver::new()` lands in the temp dir.
        let _g = crate::config::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let prior_home = std::env::var_os("HOME");
        unsafe {
            std::env::set_var("HOME", &fake_home);
        }
        let result = run_gemini(
            input_path.to_string_lossy().to_string(),
            Some(project_dir.clone()),
        );
        unsafe {
            match prior_home {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
        }
        result.expect("export gemini");

        // The file landed at chats/session-*.json (flat, prefixed).
        let canon_project = std::fs::canonicalize(&project_dir).unwrap();
        let resolver = PathResolver::new().with_home(&fake_home);
        let chats_dir = resolver.chats_dir(canon_project.to_str().unwrap()).unwrap();

        let session_files: Vec<PathBuf> = std::fs::read_dir(&chats_dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.is_file()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|s| s.starts_with("session-") && s.ends_with(".json"))
            })
            .collect();
        assert_eq!(session_files.len(), 1, "expected one session-*.json");

        let raw = std::fs::read_to_string(&session_files[0]).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed["sessionId"].as_str(), Some(session_uuid));
        assert_eq!(parsed["kind"].as_str(), Some("main"));

        // `--resume <uuid>` reads via inner sessionId, so verify the
        // library's resolver (which mirrors that lookup) finds it.
        let convo = GeminiConvo::with_resolver(resolver);
        let loaded = convo
            .read_conversation(canon_project.to_str().unwrap(), session_uuid)
            .expect("read back via uuid");
        assert_eq!(loaded.main.messages.len(), 1);
        assert_eq!(loaded.main.messages[0].content.text(), "Hello from export");
    }

    #[test]
    fn gemini_rejects_non_path_doc() {
        let temp = tempfile::tempdir().unwrap();
        let input_path = temp.path().join("input.json");
        let step = Step {
            step: StepIdentity {
                id: "s1".into(),
                parents: vec![],
                actor: "human:x".into(),
                timestamp: "2024-01-01T00:00:00Z".into(),
            },
            change: HashMap::new(),
            meta: None,
        };
        let doc = toolpath::v1::Document::Step(step);
        std::fs::write(&input_path, serde_json::to_string(&doc).unwrap()).unwrap();

        let project = temp.path().join("proj");
        std::fs::create_dir_all(&project).unwrap();
        let err = run_gemini(input_path.to_string_lossy().to_string(), Some(project))
            .expect_err("should reject Step doc");
        assert!(err.to_string().contains("Step"));
    }

    #[test]
    fn pathbase_requires_login() {
        let temp = tempfile::tempdir().unwrap();
        let input_path = temp.path().join("input.json");
        std::fs::write(
            &input_path,
            serde_json::to_string(&make_path_doc()).unwrap(),
        )
        .unwrap();

        let _g = crate::config::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var(crate::config::CONFIG_DIR_ENV, temp.path());
        }
        let err = run_pathbase(input_path.to_string_lossy().to_string(), None).unwrap_err();
        unsafe {
            std::env::remove_var(crate::config::CONFIG_DIR_ENV);
        }
        assert!(err.to_string().contains("Not logged in"));
    }
}

//! `path export <target>` — emit toolpath documents into external formats.
//!
//! `export claude` projects a Path document into Claude Code's JSONL format
//! (either into `~/.claude/projects/<sanitized>/<session>.jsonl` for a
//! resumable session, or to a file / stdout). `export pathbase` uploads the
//! document to a Pathbase server.

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
                eprintln!(
                    "  cd {} && claude -r {}",
                    project_dir.display(),
                    session_id
                );
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
fn build_claude_conversation(
    path: &toolpath::v1::Path,
) -> Result<toolpath_claude::Conversation> {
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
    std::fs::write(&out_path, jsonl)
        .with_context(|| format!("write {}", out_path.display()))?;
    Ok(out_path)
}

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

        let trace = traces_post(&base_url, &session.token, &body)?;
        println!("{}", trace.url);
        eprintln!("Uploaded {} → {} ({} bytes)", file.display(), trace.id, body.len());
        Ok(())
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
    fn pathbase_requires_login() {
        let temp = tempfile::tempdir().unwrap();
        let input_path = temp.path().join("input.json");
        std::fs::write(&input_path, serde_json::to_string(&make_path_doc()).unwrap()).unwrap();

        let _g = crate::cmd_pathbase::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var(crate::cmd_pathbase::CONFIG_DIR_ENV, temp.path());
        }
        let err = run_pathbase(input_path.to_string_lossy().to_string(), None).unwrap_err();
        unsafe {
            std::env::remove_var(crate::cmd_pathbase::CONFIG_DIR_ENV);
        }
        assert!(err.to_string().contains("Not logged in"));
    }
}

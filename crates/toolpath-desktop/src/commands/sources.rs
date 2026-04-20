use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

use crate::error::{DesktopError, DesktopResult};

/// A Claude Code project as listed for the source browser.
#[derive(Debug, Clone, Serialize)]
pub struct ClaudeProjectSummary {
    /// Original project path (e.g. `/Users/alex/myproject`).
    pub project_path: String,
    /// Friendly display name (basename of the project path).
    pub display_name: String,
    /// Number of logical conversations (chain heads) found for this project.
    pub session_count: usize,
    /// Timestamp of the most recent activity across all sessions, if known.
    pub last_activity: Option<String>,
}

/// A single conversation shown in the session picker.
#[derive(Debug, Clone, Serialize)]
pub struct ClaudeSessionSummary {
    /// Chain-head session ID.
    pub session_id: String,
    /// Short title derived from the first user message (truncated).
    pub title: Option<String>,
    /// Total turn count across the chain.
    pub turn_count: usize,
    /// ISO 8601 timestamp of the first entry.
    pub started_at: Option<String>,
    /// ISO 8601 timestamp of the most recent entry.
    pub last_activity: Option<String>,
}

/// One branch entry shown in the git picker.
#[derive(Debug, Clone, Serialize)]
pub struct GitBranchSummary {
    pub name: String,
    pub head_short: String,
    pub subject: String,
    pub author: String,
    pub timestamp: String,
}

fn claude_manager() -> toolpath_claude::ClaudeConvo {
    toolpath_claude::ClaudeConvo::new()
}

#[tauri::command]
pub fn list_claude_projects() -> DesktopResult<Vec<ClaudeProjectSummary>> {
    let manager = claude_manager();
    if !manager.exists() {
        return Ok(Vec::new());
    }

    let project_paths = manager
        .list_projects()
        .map_err(|e| DesktopError::Source(format!("list projects: {e}")))?;

    let mut out = Vec::with_capacity(project_paths.len());
    for project_path in project_paths {
        let display_name = PathBuf::from(&project_path)
            .file_name()
            .and_then(|s| s.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| project_path.clone());

        // list_conversation_metadata is sorted most-recent-first; use
        // its length for session count and the first entry's timestamp
        // for last_activity. Skip projects we can't read rather than failing
        // the whole call (a single bad project shouldn't hide the rest).
        let (session_count, last_activity) = match manager.list_conversation_metadata(&project_path)
        {
            Ok(metas) => {
                let last = metas.first().and_then(|m| m.last_activity).map(|t| t.to_rfc3339());
                (metas.len(), last)
            }
            Err(_) => (0, None),
        };

        out.push(ClaudeProjectSummary {
            project_path,
            display_name,
            session_count,
            last_activity,
        });
    }

    // Most recently active projects first.
    out.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
    Ok(out)
}

#[tauri::command]
pub fn list_claude_sessions(project_path: String) -> DesktopResult<Vec<ClaudeSessionSummary>> {
    let manager = claude_manager();
    let metadata = manager
        .list_conversation_metadata(&project_path)
        .map_err(|e| DesktopError::Source(format!("list sessions: {e}")))?;

    let mut out = Vec::with_capacity(metadata.len());
    for meta in metadata {
        let title = manager
            .read_conversation(&project_path, &meta.session_id)
            .ok()
            .and_then(|c| c.title(80));

        out.push(ClaudeSessionSummary {
            session_id: meta.session_id,
            title,
            turn_count: meta.message_count,
            started_at: meta.started_at.map(|t| t.to_rfc3339()),
            last_activity: meta.last_activity.map(|t| t.to_rfc3339()),
        });
    }
    Ok(out)
}

/// Status of an AI-agent integration on this machine.
///
/// Serialised as `kebab-case` so the frontend branches on plain strings.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)] // ComingSoon is used for future agents that haven't landed yet.
pub enum AgentStatus {
    /// Detected on disk and usable right now.
    Available,
    /// Known integration, but no data / tooling detected.
    Unavailable,
    /// Support in development — not yet wired up.
    ComingSoon,
}

/// Top-level agent entry shown on the "Agents" picker.
#[derive(Debug, Clone, Serialize)]
pub struct AgentSummary {
    /// Stable identifier used in navigation and as a routing key.
    pub id: String,
    /// Display name.
    pub name: String,
    /// One-line blurb — what this agent is.
    pub tagline: String,
    pub status: AgentStatus,
    /// Human-readable note. For `Unavailable`: why not. For `ComingSoon`:
    /// what's in flight. `None` when `Available` with nothing to add.
    pub reason: Option<String>,
    /// Absolute path where this agent's data lives, when known.
    pub data_path: Option<String>,
}

/// Enumerate known agent integrations + auto-detect which are usable.
///
/// New entries are added here as agent support lands. The list is small
/// enough that a stream variant isn't worth it — the whole thing serialises
/// in under a millisecond.
#[tauri::command]
pub fn list_agents() -> DesktopResult<Vec<AgentSummary>> {
    let manager = claude_manager();
    let claude_available = manager.exists();
    let claude_path = manager
        .claude_dir_path()
        .ok()
        .map(|p| p.to_string_lossy().into_owned());

    Ok(vec![
        AgentSummary {
            id: "claude-code".into(),
            name: "Claude Code".into(),
            tagline: "Anthropic's terminal-native coding agent.".into(),
            status: if claude_available {
                AgentStatus::Available
            } else {
                AgentStatus::Unavailable
            },
            reason: if claude_available {
                None
            } else {
                Some("No ~/.claude directory found — use Claude Code at least once and try again.".into())
            },
            data_path: claude_path,
        },
        {
            let pi = toolpath_pi::PiConvo::new();
            let pi_available = pi.exists();
            let pi_path = std::path::PathBuf::from(
                std::env::var("HOME").unwrap_or_default(),
            )
            .join(".pi/agent/sessions");
            AgentSummary {
                id: "pi-dev".into(),
                name: "pi.dev".into(),
                tagline: "pi.dev agent session traces.".into(),
                status: if pi_available {
                    AgentStatus::Available
                } else {
                    AgentStatus::Unavailable
                },
                reason: if pi_available {
                    None
                } else {
                    Some(
                        "No ~/.pi/agent/sessions directory found — run a pi.dev session and try again."
                            .into(),
                    )
                },
                data_path: pi_available
                    .then(|| pi_path.to_string_lossy().into_owned()),
            }
        },
    ])
}

/// Minimal per-project payload emitted from [`list_claude_projects_stream`].
///
/// Only cheap-to-compute fields: display name and a file-count proxy for
/// session count. `last_activity` is deliberately omitted — fetching it would
/// re-introduce the same per-session metadata reads we want to avoid.
#[derive(Debug, Clone, Serialize)]
pub struct ClaudeProjectQuick {
    pub project_path: String,
    pub display_name: String,
    pub session_count: usize,
}

/// Streaming variant of [`list_claude_projects`].
///
/// Emits one `claude:project` event per project as its cheap data becomes
/// available, then a terminal `claude:projects-done` event. The work runs
/// on Tauri's command worker pool — it's synchronous from the command's
/// perspective, so a panic turns into a rejected invoke instead of vanishing.
///
/// Frontend must subscribe to the events before invoking.
#[tauri::command]
pub fn list_claude_projects_stream(app: AppHandle) -> DesktopResult<()> {
    let manager = claude_manager();
    if !manager.exists() {
        let _ = app.emit("claude:projects-done", ());
        return Ok(());
    }

    let paths = match manager.list_projects() {
        Ok(p) => p,
        Err(e) => {
            let _ = app.emit("claude:projects-error", format!("list projects: {e}"));
            let _ = app.emit("claude:projects-done", ());
            return Ok(());
        }
    };

    for path in paths {
        let display_name = PathBuf::from(&path)
            .file_name()
            .and_then(|s| s.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| path.clone());

        // Chain heads only — cheap directory walk + chain index lookup,
        // no per-session JSONL parsing.
        let session_count = manager.list_conversations(&path).map(|v| v.len()).unwrap_or(0);

        let _ = app.emit(
            "claude:project",
            ClaudeProjectQuick {
                project_path: path,
                display_name,
                session_count,
            },
        );
    }

    let _ = app.emit("claude:projects-done", ());
    Ok(())
}

/// Per-session payload emitted from [`list_claude_sessions_stream`].
///
/// Mirrors [`ClaudeSessionSummary`] but without the expensive `title` field.
/// Frontends can fetch titles lazily for sessions the user hovers or
/// explicitly selects.
#[derive(Debug, Clone, Serialize)]
pub struct ClaudeSessionQuick {
    pub project_path: String,
    pub session_id: String,
    pub turn_count: usize,
    pub started_at: Option<String>,
    pub last_activity: Option<String>,
}

/// Streaming variant of [`list_claude_sessions`].
///
/// Emits one `claude:session` per session (metadata only, no title), then a
/// terminal `claude:sessions-done` event with the project path as payload so
/// the frontend can match the completion to the right project.
#[tauri::command]
pub fn list_claude_sessions_stream(app: AppHandle, project_path: String) -> DesktopResult<()> {
    let manager = claude_manager();
    match manager.list_conversation_metadata(&project_path) {
        Ok(metadata) => {
            for meta in metadata {
                let payload = ClaudeSessionQuick {
                    project_path: project_path.clone(),
                    session_id: meta.session_id,
                    turn_count: meta.message_count,
                    started_at: meta.started_at.map(|t| t.to_rfc3339()),
                    last_activity: meta.last_activity.map(|t| t.to_rfc3339()),
                };
                let _ = app.emit("claude:session", payload);
            }
        }
        Err(e) => {
            let _ = app.emit(
                "claude:sessions-error",
                format!("list sessions for {project_path}: {e}"),
            );
        }
    }
    let _ = app.emit("claude:sessions-done", project_path);
    Ok(())
}

/// Fetch the first-user-message title for a single session.
///
/// Requires reading the full JSONL, so it's deliberately split out from the
/// session-list stream. Call lazily (e.g. on hover) or in a background pass
/// after the session list has rendered.
#[tauri::command]
pub fn claude_session_title(
    project_path: String,
    session_id: String,
) -> DesktopResult<Option<String>> {
    let manager = claude_manager();
    let convo = manager
        .read_conversation(&project_path, &session_id)
        .map_err(|e| DesktopError::Source(format!("read {session_id}: {e}")))?;
    Ok(convo.title(80))
}

// ─── pi.dev ──────────────────────────────────────────────────────────────

fn pi_manager() -> toolpath_pi::PiConvo {
    toolpath_pi::PiConvo::new()
}

#[derive(Debug, Clone, Serialize)]
pub struct PiProjectQuick {
    pub project_path: String,
    pub display_name: String,
    pub session_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PiSessionQuick {
    pub project_path: String,
    pub session_id: String,
    pub entry_count: usize,
    pub timestamp: String,
}

/// Streaming variant of pi.dev project listing.
#[tauri::command]
pub fn list_pi_projects_stream(app: AppHandle) -> DesktopResult<()> {
    let manager = pi_manager();
    if !manager.exists() {
        let _ = app.emit("pi:projects-done", ());
        return Ok(());
    }

    let projects = match manager.list_projects() {
        Ok(p) => p,
        Err(e) => {
            let _ = app.emit("pi:projects-error", format!("list projects: {e}"));
            let _ = app.emit("pi:projects-done", ());
            return Ok(());
        }
    };

    for project in projects {
        let display_name = project.clone();
        let session_count = manager.list_sessions(&project).map(|v| v.len()).unwrap_or(0);
        let _ = app.emit(
            "pi:project",
            PiProjectQuick {
                project_path: project,
                display_name,
                session_count,
            },
        );
    }

    let _ = app.emit("pi:projects-done", ());
    Ok(())
}

/// Streaming variant of pi.dev session listing.
#[tauri::command]
pub fn list_pi_sessions_stream(app: AppHandle, project_path: String) -> DesktopResult<()> {
    let manager = pi_manager();
    match manager.list_sessions(&project_path) {
        Ok(sessions) => {
            for s in sessions {
                let _ = app.emit(
                    "pi:session",
                    PiSessionQuick {
                        project_path: project_path.clone(),
                        session_id: s.id,
                        entry_count: s.entry_count,
                        timestamp: s.timestamp,
                    },
                );
            }
        }
        Err(e) => {
            let _ = app.emit(
                "pi:sessions-error",
                format!("list sessions for {project_path}: {e}"),
            );
        }
    }
    let _ = app.emit("pi:sessions-done", project_path);
    Ok(())
}

#[tauri::command]
pub fn list_git_branches(repo_path: String) -> DesktopResult<Vec<GitBranchSummary>> {
    let repo = git2::Repository::open(&repo_path)
        .map_err(|e| DesktopError::Source(format!("open repo {repo_path}: {e}")))?;
    let branches = toolpath_git::list_branches(&repo)
        .map_err(|e| DesktopError::Source(format!("list branches: {e:#}")))?;

    Ok(branches
        .into_iter()
        .map(|b| GitBranchSummary {
            name: b.name,
            head_short: b.head_short,
            subject: b.subject,
            author: b.author,
            timestamp: b.timestamp,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use toolpath_claude::{ClaudeConvo, PathResolver};

    fn setup_manager_with_project() -> (tempfile::TempDir, ClaudeConvo) {
        let temp = tempfile::tempdir().unwrap();
        let claude_dir = temp.path().join(".claude");
        let project_dir = claude_dir.join("projects/-test-project");
        std::fs::create_dir_all(&project_dir).unwrap();

        let entry1 = r#"{"type":"user","uuid":"uuid-1","timestamp":"2024-01-01T00:00:00Z","cwd":"/test/project","message":{"role":"user","content":"Hello world"}}"#;
        let entry2 = r#"{"type":"assistant","uuid":"uuid-2","timestamp":"2024-01-01T00:00:01Z","message":{"role":"assistant","content":"Hi there"}}"#;
        std::fs::write(
            project_dir.join("session-abc.jsonl"),
            format!("{entry1}\n{entry2}\n"),
        )
        .unwrap();

        let resolver = PathResolver::new().with_claude_dir(&claude_dir);
        let manager = ClaudeConvo::with_resolver(resolver);
        (temp, manager)
    }

    #[test]
    fn session_summary_includes_title_and_turns() {
        let (_temp, manager) = setup_manager_with_project();
        let metas = manager
            .list_conversation_metadata("/test/project")
            .expect("list");
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].message_count, 2);

        let convo = manager
            .read_conversation("/test/project", &metas[0].session_id)
            .expect("read");
        assert_eq!(convo.title(80).as_deref(), Some("Hello world"));
    }

    #[test]
    fn list_git_branches_opens_temp_repo() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        {
            let mut config = repo.config().unwrap();
            config.set_str("user.name", "Test").unwrap();
            config.set_str("user.email", "t@t.t").unwrap();
        }
        let mut index = repo.index().unwrap();
        let file = dir.path().join("f.txt");
        std::fs::write(&file, "hi").unwrap();
        index.add_path(std::path::Path::new("f.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();

        let branches =
            list_git_branches(dir.path().to_string_lossy().into_owned()).expect("branches");
        assert!(!branches.is_empty());
    }

    #[test]
    fn list_git_branches_rejects_non_repo() {
        let dir = tempfile::tempdir().unwrap();
        let err = list_git_branches(dir.path().to_string_lossy().into_owned()).unwrap_err();
        assert!(matches!(err, DesktopError::Source(_)));
    }
}

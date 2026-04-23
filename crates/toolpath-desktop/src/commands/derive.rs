use std::sync::Arc;

use serde_json::Value;
use tauri::State;

use crate::cache::{CacheEntry, CacheKey, TraceCache};
use crate::commands::keychain;
use crate::error::{DesktopError, DesktopResult};

/// Serialise a `Document` through its JSON form so the frontend receives it as
/// a plain value without us manually reimplementing every type.
fn document_to_value(doc: &toolpath::v1::Document) -> DesktopResult<Value> {
    let json = doc
        .to_json()
        .map_err(|e| DesktopError::Derive(format!("serialize document: {e}")))?;
    Ok(serde_json::from_str(&json)?)
}

/// Core claude-derive logic, cache-aware. Used by both the Tauri command
/// wrapper and the tray's popover-open / prewarm paths — having a single
/// implementation guarantees they all share the same cache behaviour.
///
/// Cache lookup/insert only happens for single-session, no-thinking derives
/// (the shape the tray poller prewarms). Multi-session / thinking-on calls
/// fall through to a fresh derive every time.
pub fn derive_claude_impl(
    cache: &TraceCache,
    project_path: String,
    session_ids: Vec<String>,
    include_thinking: bool,
) -> DesktopResult<Value> {
    if session_ids.is_empty() {
        return Err(DesktopError::InvalidInput(
            "at least one session ID is required".into(),
        ));
    }

    let cacheable = session_ids.len() == 1 && !include_thinking;
    if cacheable {
        let key = CacheKey {
            provider: "claude".into(),
            project: project_path.clone(),
            session_id: session_ids[0].clone(),
        };
        if let Some(hit) = cache.get(&key) {
            return Ok(hit.doc);
        }
    }

    let manager = toolpath_claude::ClaudeConvo::new();
    let config = toolpath_claude::derive::DeriveConfig {
        project_path: Some(project_path.clone()),
        include_thinking,
    };

    let mut paths = Vec::with_capacity(session_ids.len());
    for session_id in &session_ids {
        let convo = manager
            .read_conversation(&project_path, session_id)
            .map_err(|e| DesktopError::Derive(format!("read {session_id}: {e}")))?;
        paths.push(toolpath_claude::derive::derive_path(&convo, &config));
    }

    let document = if paths.len() == 1 {
        toolpath::v1::Document::Path(paths.pop().unwrap())
    } else {
        // Multiple sessions selected -> produce a Graph wrapping each Path.
        let graph = toolpath::v1::Graph {
            graph: toolpath::v1::GraphIdentity {
                id: format!("graph-claude-{}", uuid_suffix()),
            },
            paths: paths
                .into_iter()
                .map(|p| toolpath::v1::PathOrRef::Path(Box::new(p)))
                .collect(),
            meta: Some(toolpath::v1::GraphMeta {
                title: Some(format!("Claude sessions from {project_path}")),
                ..Default::default()
            }),
        };
        toolpath::v1::Document::Graph(graph)
    };

    let value = document_to_value(&document)?;

    if cacheable {
        // Backfill the cache so a subsequent click (e.g. popover opening the
        // same session) doesn't re-derive. `last_activity` is unknown here —
        // leave it empty and let the next warmer pass replace it with a
        // freshness-keyed entry.
        cache.insert(
            CacheKey {
                provider: "claude".into(),
                project: project_path,
                session_id: session_ids.into_iter().next().unwrap(),
            },
            CacheEntry {
                doc: value.clone(),
                last_activity: String::new(),
            },
        );
    }

    Ok(value)
}

#[tauri::command]
pub fn derive_claude(
    cache: State<'_, Arc<TraceCache>>,
    project_path: String,
    session_ids: Vec<String>,
    include_thinking: bool,
) -> DesktopResult<Value> {
    derive_claude_impl(cache.inner(), project_path, session_ids, include_thinking)
}

pub fn derive_pi_impl(
    cache: &TraceCache,
    project_path: String,
    session_ids: Vec<String>,
    include_thinking: bool,
) -> DesktopResult<Value> {
    if session_ids.is_empty() {
        return Err(DesktopError::InvalidInput(
            "at least one session ID is required".into(),
        ));
    }

    let cacheable = session_ids.len() == 1 && !include_thinking;
    if cacheable {
        let key = CacheKey {
            provider: "pi".into(),
            project: project_path.clone(),
            session_id: session_ids[0].clone(),
        };
        if let Some(hit) = cache.get(&key) {
            return Ok(hit.doc);
        }
    }

    let manager = toolpath_pi::PiConvo::new();
    let config = toolpath_pi::DeriveConfig {
        include_thinking,
        ..Default::default()
    };

    let mut paths = Vec::with_capacity(session_ids.len());
    for session_id in &session_ids {
        let session = manager
            .read_session(&project_path, session_id)
            .map_err(|e| DesktopError::Derive(format!("read {session_id}: {e}")))?;
        paths.push(toolpath_pi::derive_path(&session, &config));
    }

    let document = if paths.len() == 1 {
        toolpath::v1::Document::Path(paths.pop().unwrap())
    } else {
        let graph = toolpath::v1::Graph {
            graph: toolpath::v1::GraphIdentity {
                id: format!("graph-pi-{}", uuid_suffix()),
            },
            paths: paths
                .into_iter()
                .map(|p| toolpath::v1::PathOrRef::Path(Box::new(p)))
                .collect(),
            meta: Some(toolpath::v1::GraphMeta {
                title: Some(format!("pi.dev sessions from {project_path}")),
                ..Default::default()
            }),
        };
        toolpath::v1::Document::Graph(graph)
    };

    let value = document_to_value(&document)?;

    if cacheable {
        cache.insert(
            CacheKey {
                provider: "pi".into(),
                project: project_path,
                session_id: session_ids.into_iter().next().unwrap(),
            },
            CacheEntry {
                doc: value.clone(),
                last_activity: String::new(),
            },
        );
    }

    Ok(value)
}

#[tauri::command]
pub fn derive_pi(
    cache: State<'_, Arc<TraceCache>>,
    project_path: String,
    session_ids: Vec<String>,
    include_thinking: bool,
) -> DesktopResult<Value> {
    derive_pi_impl(cache.inner(), project_path, session_ids, include_thinking)
}

#[tauri::command]
pub fn derive_git(repo_path: String, branch: String, base: Option<String>) -> DesktopResult<Value> {
    if branch.is_empty() {
        return Err(DesktopError::InvalidInput("branch is required".into()));
    }
    let repo = git2::Repository::open(&repo_path)
        .map_err(|e| DesktopError::Derive(format!("open repo {repo_path}: {e}")))?;

    let config = toolpath_git::DeriveConfig {
        remote: "origin".to_string(),
        title: None,
        base,
    };

    let document = toolpath_git::derive(&repo, std::slice::from_ref(&branch), &config)
        .map_err(|e| DesktopError::Derive(format!("derive git: {e:#}")))?;

    document_to_value(&document)
}

#[tauri::command]
pub fn derive_github(
    pr_url: String,
    include_ci: bool,
    include_comments: bool,
) -> DesktopResult<Value> {
    let parsed = toolpath_github::parse_pr_url(&pr_url).ok_or_else(|| {
        DesktopError::InvalidInput(
            "invalid PR URL; expected https://github.com/owner/repo/pull/N".into(),
        )
    })?;

    let token = keychain::read_github_token()
        .map_err(|e| DesktopError::Auth(format!("no GitHub token configured: {e}")))?;

    let config = toolpath_github::DeriveConfig {
        token,
        include_ci,
        include_comments,
        ..Default::default()
    };

    let path =
        toolpath_github::derive_pull_request(&parsed.owner, &parsed.repo, parsed.number, &config)
            .map_err(|e| DesktopError::Derive(format!("derive github: {e:#}")))?;

    document_to_value(&toolpath::v1::Document::Path(path))
}

fn uuid_suffix() -> String {
    uuid::Uuid::new_v4().to_string().chars().take(8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn derive_claude_rejects_empty_selection() {
        let cache = TraceCache::new();
        let err = derive_claude_impl(&cache, "/nowhere".into(), vec![], false).unwrap_err();
        assert!(matches!(err, DesktopError::InvalidInput(_)));
    }

    #[test]
    fn derive_pi_rejects_empty_selection() {
        let cache = TraceCache::new();
        let err = derive_pi_impl(&cache, "/nowhere".into(), vec![], false).unwrap_err();
        assert!(matches!(err, DesktopError::InvalidInput(_)));
    }

    #[test]
    fn derive_claude_serves_cached_doc_without_running_derive() {
        // Pre-populate the cache with a fake doc. A real derive against
        // /nowhere would error; a cache hit must short-circuit before that.
        let cache = TraceCache::new();
        let expected = serde_json::json!({"marker": "cached"});
        cache.insert(
            CacheKey {
                provider: "claude".into(),
                project: "/nowhere".into(),
                session_id: "sess".into(),
            },
            CacheEntry {
                doc: expected.clone(),
                last_activity: "2026-04-23T10:00:00Z".into(),
            },
        );
        let got =
            derive_claude_impl(&cache, "/nowhere".into(), vec!["sess".into()], false).unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn derive_pi_serves_cached_doc_without_running_derive() {
        let cache = TraceCache::new();
        let expected = serde_json::json!({"marker": "pi-cached"});
        cache.insert(
            CacheKey {
                provider: "pi".into(),
                project: "/nowhere".into(),
                session_id: "sess".into(),
            },
            CacheEntry {
                doc: expected.clone(),
                last_activity: String::new(),
            },
        );
        let got =
            derive_pi_impl(&cache, "/nowhere".into(), vec!["sess".into()], false).unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn derive_git_rejects_missing_branch() {
        let err = derive_git("/tmp".into(), String::new(), None).unwrap_err();
        assert!(matches!(err, DesktopError::InvalidInput(_)));
    }

    #[test]
    fn derive_git_rejects_bad_repo() {
        let dir = tempdir().unwrap();
        let err = derive_git(
            dir.path().to_string_lossy().into_owned(),
            "main".into(),
            None,
        )
        .unwrap_err();
        assert!(matches!(err, DesktopError::Derive(_)));
    }

    #[test]
    fn derive_github_rejects_bad_url() {
        let err = derive_github("not a url".into(), true, true).unwrap_err();
        assert!(matches!(err, DesktopError::InvalidInput(_)));
    }
}

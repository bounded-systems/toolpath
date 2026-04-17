use serde_json::Value;

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

#[tauri::command]
pub fn derive_claude(
    project_path: String,
    session_ids: Vec<String>,
    include_thinking: bool,
) -> DesktopResult<Value> {
    if session_ids.is_empty() {
        return Err(DesktopError::InvalidInput(
            "at least one session ID is required".into(),
        ));
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

    document_to_value(&document)
}

#[tauri::command]
pub fn derive_pi(
    project_path: String,
    session_ids: Vec<String>,
    include_thinking: bool,
) -> DesktopResult<Value> {
    if session_ids.is_empty() {
        return Err(DesktopError::InvalidInput(
            "at least one session ID is required".into(),
        ));
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

    document_to_value(&document)
}

#[tauri::command]
pub fn derive_git(
    repo_path: String,
    branch: String,
    base: Option<String>,
) -> DesktopResult<Value> {
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

    let path = toolpath_github::derive_pull_request(
        &parsed.owner,
        &parsed.repo,
        parsed.number,
        &config,
    )
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
        let err = derive_claude("/nowhere".into(), vec![], false).unwrap_err();
        assert!(matches!(err, DesktopError::InvalidInput(_)));
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

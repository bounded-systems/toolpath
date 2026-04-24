//! `path import <source>` — ingest external formats into toolpath documents.
//!
//! Default behavior writes each derived document into the on-disk cache at
//! `$CONFIG_DIR/documents/` under `<source>-<inner-id>.json` and prints the
//! path to stdout. `--no-cache` sends the JSON to stdout instead, for shell
//! composition with `render | query | validate`.

#[cfg(not(target_os = "emscripten"))]
use anyhow::Context;
use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;
use toolpath::v1::Document;

use crate::cmd_cache::{make_id, write_cached};

#[derive(Subcommand, Debug)]
pub enum ImportSource {
    /// Import from git repository history
    Git {
        /// Path to the git repository
        #[arg(short, long, default_value = ".")]
        repo: PathBuf,

        /// Branch name(s). Format: `name` or `name:start`
        #[arg(short, long, required = true)]
        branch: Vec<String>,

        /// Global base commit (overrides per-branch starts)
        #[arg(long)]
        base: Option<String>,

        /// Remote name for URI generation
        #[arg(long, default_value = "origin")]
        remote: String,

        /// Graph title (for multi-branch output)
        #[arg(long)]
        title: Option<String>,
    },
    /// Import from a GitHub pull request
    Github {
        /// PR URL (e.g. <https://github.com/owner/repo/pull/42>)
        #[arg(index = 1)]
        url: Option<String>,

        /// Repository in owner/repo format (alternative to URL)
        #[arg(short, long)]
        repo: Option<String>,

        /// Pull request number (required with --repo)
        #[arg(long)]
        pr: Option<u64>,

        /// Exclude CI check runs
        #[arg(long)]
        no_ci: bool,

        /// Exclude reviews and comments
        #[arg(long)]
        no_comments: bool,
    },
    /// Import from Claude conversation logs
    Claude {
        /// Project path (e.g., /Users/alex/myproject)
        #[arg(short, long)]
        project: String,

        /// Specific session ID
        #[arg(short, long)]
        session: Option<String>,

        /// Process all sessions in the project
        #[arg(long)]
        all: bool,
    },
    /// Import from Gemini CLI conversation logs
    Gemini {
        /// Project path (e.g., /Users/alex/myproject)
        #[arg(short, long)]
        project: String,

        /// Specific session UUID (the directory name under chats/)
        #[arg(short, long)]
        session: Option<String>,

        /// Process all sessions in the project
        #[arg(long)]
        all: bool,

        /// Include thinking blocks in conversation.append text
        #[arg(long)]
        include_thinking: bool,
    },
    /// Import from Codex CLI rollout files
    Codex {
        /// Session id, UUID, or filename stem (default: most recent)
        #[arg(short, long)]
        session: Option<String>,

        /// Process all sessions (emits one Path per session)
        #[arg(long)]
        all: bool,
    },
    /// Import from opencode session databases
    Opencode {
        /// Session id (default: most recent)
        #[arg(short, long)]
        session: Option<String>,

        /// Process all sessions (emits one Path per session)
        #[arg(long)]
        all: bool,

        /// Filter by project id (SHA of repo's first root commit)
        #[arg(long)]
        project: Option<String>,

        /// Skip snapshot-based file diff extraction
        #[arg(long)]
        no_snapshot_diffs: bool,
    },
    /// Import from Pi (pi.dev) coding-agent session logs
    Pi {
        /// Project path (cwd the session ran in)
        #[arg(short, long)]
        project: String,

        /// Specific session ID (default: most recent)
        #[arg(short, long)]
        session: Option<String>,

        /// Process all sessions in the project (emits a Graph)
        #[arg(long)]
        all: bool,

        /// Override the Pi sessions base directory (default: ~/.pi/agent/sessions)
        #[arg(long)]
        base: Option<PathBuf>,
    },
    /// Import from Pathbase (download a previously uploaded trace)
    Pathbase {
        /// Trace id or full pathbase URL
        #[arg(index = 1)]
        target: String,

        /// Pathbase server URL (overrides $PATHBASE_URL; ignored if target is a URL)
        #[arg(long)]
        url: Option<String>,
    },
}

#[derive(clap::Args, Debug)]
pub struct ImportArgs {
    #[command(subcommand)]
    pub source: ImportSource,

    /// Overwrite the cache entry if it already exists
    #[arg(long, global = true)]
    pub force: bool,

    /// Print the toolpath JSON to stdout instead of writing the cache
    #[arg(long, global = true)]
    pub no_cache: bool,
}

pub fn run(args: ImportArgs, pretty: bool) -> Result<()> {
    let docs = derive(args.source)?;
    emit(&docs, args.force, args.no_cache, pretty)
}

struct DerivedDoc {
    cache_id: String,
    doc: Document,
}

fn emit(docs: &[DerivedDoc], force: bool, no_cache: bool, pretty: bool) -> Result<()> {
    if docs.is_empty() {
        anyhow::bail!("no documents produced");
    }
    for d in docs {
        if no_cache {
            let json = if pretty {
                d.doc.to_json_pretty()?
            } else {
                d.doc.to_json()?
            };
            println!("{}", json);
        } else {
            let path = write_cached(&d.cache_id, &d.doc, force)?;
            println!("{}", path.display());
            let summary = doc_summary(&d.doc);
            eprintln!("Imported {} → {}", summary, d.cache_id);
        }
    }
    Ok(())
}

fn doc_summary(doc: &Document) -> String {
    match doc {
        Document::Graph(g) => format!("graph {} ({} paths)", g.graph.id, g.paths.len()),
        Document::Path(p) => format!("path {} ({} steps)", p.path.id, p.steps.len()),
        Document::Step(s) => format!("step {}", s.step.id),
    }
}

fn derive(source: ImportSource) -> Result<Vec<DerivedDoc>> {
    match source {
        ImportSource::Git {
            repo,
            branch,
            base,
            remote,
            title,
        } => derive_git(repo, branch, base, remote, title),
        ImportSource::Github {
            url,
            repo,
            pr,
            no_ci,
            no_comments,
        } => derive_github(url, repo, pr, no_ci, no_comments),
        ImportSource::Claude {
            project,
            session,
            all,
        } => derive_claude(project, session, all),
        ImportSource::Gemini {
            project,
            session,
            all,
            include_thinking,
        } => derive_gemini(project, session, all, include_thinking),
        ImportSource::Codex { session, all } => derive_codex(session, all),
        ImportSource::Opencode {
            session,
            all,
            project,
            no_snapshot_diffs,
        } => derive_opencode(session, all, project, no_snapshot_diffs),
        ImportSource::Pi {
            project,
            session,
            all,
            base,
        } => derive_pi(project, session, all, base),
        ImportSource::Pathbase { target, url } => derive_pathbase(target, url),
    }
}

// ── per-source derivations ─────────────────────────────────────────────

fn derive_git(
    repo_path: PathBuf,
    branches: Vec<String>,
    base: Option<String>,
    remote: String,
    title: Option<String>,
) -> Result<Vec<DerivedDoc>> {
    #[cfg(target_os = "emscripten")]
    {
        let _ = (repo_path, branches, base, remote, title);
        anyhow::bail!(
            "'path import git' requires a native environment with access to a git repository"
        );
    }

    #[cfg(not(target_os = "emscripten"))]
    {
        let repo_path = if repo_path.is_absolute() {
            repo_path
        } else {
            std::env::current_dir()?.join(&repo_path)
        };

        let repo = git2::Repository::open(&repo_path)
            .with_context(|| format!("Failed to open repository at {:?}", repo_path))?;

        let config = toolpath_git::DeriveConfig {
            remote,
            title,
            base,
        };

        let doc = toolpath_git::derive(&repo, &branches, &config)?;
        let cache_id = cache_id_for_doc("git", &doc);
        Ok(vec![DerivedDoc { cache_id, doc }])
    }
}

fn derive_github(
    url: Option<String>,
    repo: Option<String>,
    pr: Option<u64>,
    no_ci: bool,
    no_comments: bool,
) -> Result<Vec<DerivedDoc>> {
    #[cfg(target_os = "emscripten")]
    {
        let _ = (url, repo, pr, no_ci, no_comments);
        anyhow::bail!("'path import github' requires a native environment with network access");
    }

    #[cfg(not(target_os = "emscripten"))]
    {
        let (owner, repo_name, pr_number) = if let Some(url_str) = &url {
            let parsed = toolpath_github::parse_pr_url(url_str).ok_or_else(|| {
                anyhow::anyhow!("Invalid PR URL. Expected: https://github.com/owner/repo/pull/N")
            })?;
            (parsed.owner, parsed.repo, parsed.number)
        } else if let (Some(repo_str), Some(pr_num)) = (&repo, pr) {
            let (o, r) = repo_str
                .split_once('/')
                .ok_or_else(|| anyhow::anyhow!("Repository must be in owner/repo format"))?;
            (o.to_string(), r.to_string(), pr_num)
        } else {
            anyhow::bail!(
                "Provide a PR URL or both --repo and --pr.\n\
                 Usage: path import github https://github.com/owner/repo/pull/42\n\
                 Usage: path import github --repo owner/repo --pr 42"
            );
        };

        let token = toolpath_github::resolve_token()?;
        let config = toolpath_github::DeriveConfig {
            token,
            include_ci: !no_ci,
            include_comments: !no_comments,
            ..Default::default()
        };

        let path = toolpath_github::derive_pull_request(&owner, &repo_name, pr_number, &config)?;
        let doc = Document::Path(path);
        let cache_id = make_id("github", &format!("{owner}_{repo_name}-{pr_number}"));
        Ok(vec![DerivedDoc { cache_id, doc }])
    }
}

fn derive_claude(project: String, session: Option<String>, all: bool) -> Result<Vec<DerivedDoc>> {
    let manager = toolpath_claude::ClaudeConvo::new();
    derive_claude_with_manager(&manager, project, session, all)
}

fn derive_claude_with_manager(
    manager: &toolpath_claude::ClaudeConvo,
    project: String,
    session: Option<String>,
    all: bool,
) -> Result<Vec<DerivedDoc>> {
    let config = toolpath_claude::derive::DeriveConfig {
        project_path: Some(project.clone()),
        include_thinking: false,
    };

    let paths: Vec<toolpath::v1::Path> = if let Some(session_id) = session {
        let convo = manager
            .read_conversation(&project, &session_id)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        vec![toolpath_claude::derive::derive_path(&convo, &config)]
    } else if all {
        let convos = manager
            .read_all_conversations(&project)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        toolpath_claude::derive::derive_project(&convos, &config)
    } else {
        let convo = manager
            .most_recent_conversation(&project)
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .ok_or_else(|| anyhow::anyhow!("No conversations found for project: {}", project))?;
        vec![toolpath_claude::derive::derive_path(&convo, &config)]
    };

    Ok(paths
        .into_iter()
        .map(|p| {
            let cache_id = make_id("claude", &p.path.id);
            DerivedDoc {
                cache_id,
                doc: Document::Path(p),
            }
        })
        .collect())
}

fn derive_gemini(
    project: String,
    session: Option<String>,
    all: bool,
    include_thinking: bool,
) -> Result<Vec<DerivedDoc>> {
    let manager = toolpath_gemini::GeminiConvo::new();
    derive_gemini_with_manager(&manager, project, session, all, include_thinking)
}

fn derive_gemini_with_manager(
    manager: &toolpath_gemini::GeminiConvo,
    project: String,
    session: Option<String>,
    all: bool,
    include_thinking: bool,
) -> Result<Vec<DerivedDoc>> {
    let config = toolpath_gemini::derive::DeriveConfig {
        project_path: Some(project.clone()),
        include_thinking,
    };

    let paths: Vec<toolpath::v1::Path> = if let Some(session_uuid) = session {
        let convo = manager
            .read_conversation(&project, &session_uuid)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        vec![toolpath_gemini::derive::derive_path(&convo, &config)]
    } else if all {
        let convos = manager
            .read_all_conversations(&project)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        toolpath_gemini::derive::derive_project(&convos, &config)
    } else {
        let convo = manager
            .most_recent_conversation(&project)
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .ok_or_else(|| anyhow::anyhow!("No conversations found for project: {}", project))?;
        vec![toolpath_gemini::derive::derive_path(&convo, &config)]
    };

    Ok(paths
        .into_iter()
        .map(|p| {
            let cache_id = make_id("gemini", &p.path.id);
            DerivedDoc {
                cache_id,
                doc: Document::Path(p),
            }
        })
        .collect())
}

fn derive_codex(session: Option<String>, all: bool) -> Result<Vec<DerivedDoc>> {
    let manager = toolpath_codex::CodexConvo::new();
    let config = toolpath_codex::derive::DeriveConfig { project_path: None };

    let paths: Vec<toolpath::v1::Path> = if all {
        let sessions = manager
            .read_all_sessions()
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        if sessions.is_empty() {
            anyhow::bail!("No Codex sessions found in ~/.codex/sessions");
        }
        toolpath_codex::derive::derive_project(&sessions, &config)
    } else if let Some(sid) = session {
        let s = manager
            .read_session(&sid)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        vec![toolpath_codex::derive::derive_path(&s, &config)]
    } else {
        let s = manager
            .most_recent_session()
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .ok_or_else(|| anyhow::anyhow!("No Codex sessions found in ~/.codex/sessions"))?;
        vec![toolpath_codex::derive::derive_path(&s, &config)]
    };

    Ok(paths
        .into_iter()
        .map(|p| {
            let cache_id = make_id("codex", &p.path.id);
            DerivedDoc {
                cache_id,
                doc: Document::Path(p),
            }
        })
        .collect())
}

fn derive_opencode(
    session: Option<String>,
    all: bool,
    project: Option<String>,
    no_snapshot_diffs: bool,
) -> Result<Vec<DerivedDoc>> {
    #[cfg(target_os = "emscripten")]
    {
        let _ = (session, all, project, no_snapshot_diffs);
        anyhow::bail!(
            "'path import opencode' requires a native environment (SQLite + git2 not available under wasm)"
        );
    }

    #[cfg(not(target_os = "emscripten"))]
    {
        let manager = toolpath_opencode::OpencodeConvo::new();
        let config = toolpath_opencode::derive::DeriveConfig {
            no_snapshot_diffs,
            ..Default::default()
        };

        let paths: Vec<toolpath::v1::Path> = if all {
            let metas = manager
                .io()
                .list_session_metadata(project.as_deref())
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            if metas.is_empty() {
                anyhow::bail!("No opencode sessions found");
            }
            let mut out = Vec::with_capacity(metas.len());
            for m in metas {
                let s = manager
                    .read_session(&m.id)
                    .map_err(|e| anyhow::anyhow!("{}: {}", m.id, e))?;
                out.push(toolpath_opencode::derive::derive_path_with_resolver(
                    &s,
                    &config,
                    manager.resolver(),
                ));
            }
            out
        } else if let Some(sid) = session {
            let s = manager
                .read_session(&sid)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            vec![toolpath_opencode::derive::derive_path_with_resolver(
                &s,
                &config,
                manager.resolver(),
            )]
        } else {
            let s = manager
                .most_recent_session()
                .map_err(|e| anyhow::anyhow!("{}", e))?
                .ok_or_else(|| anyhow::anyhow!("No opencode sessions found"))?;
            vec![toolpath_opencode::derive::derive_path_with_resolver(
                &s,
                &config,
                manager.resolver(),
            )]
        };

        Ok(paths
            .into_iter()
            .map(|p| {
                let cache_id = make_id("opencode", &p.path.id);
                DerivedDoc {
                    cache_id,
                    doc: Document::Path(p),
                }
            })
            .collect())
    }
}

fn derive_pi(
    project: String,
    session: Option<String>,
    all: bool,
    base: Option<PathBuf>,
) -> Result<Vec<DerivedDoc>> {
    let manager = if let Some(path) = base {
        let resolver = toolpath_pi::PathResolver::new().with_sessions_dir(&path);
        toolpath_pi::PiConvo::with_resolver(resolver)
    } else {
        toolpath_pi::PiConvo::new()
    };
    derive_pi_with_manager(&manager, project, session, all)
}

fn derive_pi_with_manager(
    manager: &toolpath_pi::PiConvo,
    project: String,
    session: Option<String>,
    all: bool,
) -> Result<Vec<DerivedDoc>> {
    let config = toolpath_pi::DeriveConfig::default();

    let doc: Document = if all {
        let sessions = manager
            .read_all_sessions(&project)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        if sessions.is_empty() {
            anyhow::bail!("No Pi sessions found for project: {}", project);
        }
        let graph = toolpath_pi::derive::derive_graph(&sessions, None, &config);
        Document::Graph(graph)
    } else if let Some(sid) = session {
        let session = manager
            .read_session(&project, &sid)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        Document::Path(toolpath_pi::derive::derive_path(&session, &config))
    } else {
        let session = manager
            .most_recent_session(&project)
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .ok_or_else(|| anyhow::anyhow!("No Pi sessions found for project: {}", project))?;
        Document::Path(toolpath_pi::derive::derive_path(&session, &config))
    };

    let cache_id = cache_id_for_doc("pi", &doc);
    Ok(vec![DerivedDoc { cache_id, doc }])
}

fn derive_pathbase(target: String, url_flag: Option<String>) -> Result<Vec<DerivedDoc>> {
    #[cfg(target_os = "emscripten")]
    {
        let _ = (target, url_flag);
        anyhow::bail!("'path import pathbase' requires a native environment with network access");
    }

    #[cfg(not(target_os = "emscripten"))]
    {
        use crate::cmd_pathbase::{require_session, traces_get};

        let (base, id) = parse_pathbase_ref(&target, url_flag.as_deref())?;
        let session = require_session()?;
        // If the ref gave us an explicit base URL (via a full URL or --url),
        // use that. Otherwise fall back to the stored session's server.
        let base_url = base.unwrap_or_else(|| session.url.clone());
        let body = traces_get(&base_url, &session.token, &id)?;
        let doc = Document::from_json(&body).map_err(|e| {
            anyhow::anyhow!("server returned a non-toolpath document: {e}")
        })?;
        let cache_id = make_id("pathbase", &id);
        Ok(vec![DerivedDoc { cache_id, doc }])
    }
}

/// Parse a positional ref for `path import pathbase`. Returns `(override_base, id)`.
///
/// If the ref is a full URL like `https://pathbase.dev/traces/trc_01H...`, the
/// host prefix replaces the server URL and the trailing segment is the id.
/// Otherwise the ref is a bare id; `--url` (via `url_flag`) or `$PATHBASE_URL`
/// / default apply via the caller's session.
#[cfg(not(target_os = "emscripten"))]
fn parse_pathbase_ref(target: &str, url_flag: Option<&str>) -> Result<(Option<String>, String)> {
    use crate::cmd_pathbase::resolve_url;

    let scheme = if target.starts_with("https://") {
        Some("https://")
    } else if target.starts_with("http://") {
        Some("http://")
    } else {
        None
    };

    if let Some(scheme) = scheme {
        let rest = &target[scheme.len()..];
        let (host, path) = match rest.split_once('/') {
            Some((h, p)) => (h, p),
            None => anyhow::bail!("URL has no trace id segment: {target}"),
        };
        if host.is_empty() {
            anyhow::bail!("URL is missing a host: {target}");
        }
        let path = path
            .split(['?', '#'])
            .next()
            .unwrap_or("")
            .trim_end_matches('/');
        let id = path
            .rsplit('/')
            .find(|s| !s.is_empty())
            .ok_or_else(|| anyhow::anyhow!("URL has no trace id segment: {target}"))?
            .to_string();
        let base = format!("{scheme}{host}");
        Ok((Some(base), id))
    } else {
        let base = url_flag.map(|u| resolve_url(Some(u.to_string())));
        Ok((base, target.to_string()))
    }
}

fn cache_id_for_doc(source: &str, doc: &Document) -> String {
    match doc {
        Document::Graph(g) => make_id(source, &g.graph.id),
        Document::Path(p) => make_id(source, &p.path.id),
        Document::Step(s) => make_id(source, &s.step.id),
    }
}

#[cfg(all(test, not(target_os = "emscripten")))]
mod tests {
    use super::*;

    #[test]
    fn parse_pathbase_ref_full_url() {
        let (base, id) =
            parse_pathbase_ref("https://pathbase.dev/traces/trc_01H", None).unwrap();
        assert_eq!(base.as_deref(), Some("https://pathbase.dev"));
        assert_eq!(id, "trc_01H");
    }

    #[test]
    fn parse_pathbase_ref_bare_id_with_url_flag() {
        let (base, id) = parse_pathbase_ref("trc_01H", Some("https://other.example/")).unwrap();
        assert_eq!(base.as_deref(), Some("https://other.example"));
        assert_eq!(id, "trc_01H");
    }

    #[test]
    fn parse_pathbase_ref_bare_id_no_flag() {
        let (base, id) = parse_pathbase_ref("trc_01H", None).unwrap();
        assert_eq!(base, None);
        assert_eq!(id, "trc_01H");
    }

    #[test]
    fn parse_pathbase_ref_url_with_trailing_slash() {
        let (base, id) =
            parse_pathbase_ref("https://pathbase.dev/traces/trc_01H/", None).unwrap();
        assert_eq!(base.as_deref(), Some("https://pathbase.dev"));
        assert_eq!(id, "trc_01H");
    }

    fn setup_claude_manager() -> (tempfile::TempDir, toolpath_claude::ClaudeConvo) {
        let temp = tempfile::tempdir().unwrap();
        let claude_dir = temp.path().join(".claude");
        let project_dir = claude_dir.join("projects/-test-project");
        std::fs::create_dir_all(&project_dir).unwrap();

        let entry1 = r#"{"type":"user","uuid":"uuid-1","timestamp":"2024-01-01T00:00:00Z","cwd":"/test/project","message":{"role":"user","content":"Hello"}}"#;
        let entry2 = r#"{"type":"assistant","uuid":"uuid-2","timestamp":"2024-01-01T00:00:01Z","message":{"role":"assistant","content":"Hi there"}}"#;
        std::fs::write(
            project_dir.join("session-abc.jsonl"),
            format!("{}\n{}\n", entry1, entry2),
        )
        .unwrap();

        let resolver = toolpath_claude::PathResolver::new().with_claude_dir(&claude_dir);
        let manager = toolpath_claude::ClaudeConvo::with_resolver(resolver);
        (temp, manager)
    }

    #[test]
    fn derive_claude_session_returns_one_doc() {
        let (_t, mgr) = setup_claude_manager();
        let out = derive_claude_with_manager(
            &mgr,
            "/test/project".to_string(),
            Some("session-abc".to_string()),
            false,
        )
        .unwrap();
        assert_eq!(out.len(), 1);
        assert!(out[0].cache_id.starts_with("claude-"));
    }
}

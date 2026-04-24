//! Shared Pathbase client helpers.
//!
//! Hosts the HTTP client, URL / config-dir resolution, and session
//! storage logic used by `cmd_auth`, `cmd_import`, `cmd_export`, and
//! `cmd_cache`. All functions are `pub(crate)` — the outside world
//! interacts with these through the relevant command modules.

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub(crate) const CONFIG_DIR_NAME: &str = ".toolpath";
pub(crate) const CREDENTIALS_FILE: &str = "credentials.json";
pub(crate) const CONFIG_DIR_ENV: &str = "TOOLPATH_CONFIG_DIR";
pub(crate) const DEFAULT_URL: &str = "https://pathbase.dev";
pub(crate) const PATHBASE_URL_ENV: &str = "PATHBASE_URL";

/// JSON blob persisted at `credentials.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredSession {
    pub url: String,
    pub token: String,
    pub user: User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct User {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

/// Reference returned after uploading a trace.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TraceRef {
    pub id: String,
    pub url: String,
}

// ── URL + prompt helpers ────────────────────────────────────────────────

pub(crate) fn resolve_url(cli_url: Option<String>) -> String {
    let raw = cli_url
        .or_else(|| std::env::var(PATHBASE_URL_ENV).ok())
        .unwrap_or_else(|| DEFAULT_URL.to_string());
    raw.trim_end_matches('/').to_string()
}

pub(crate) fn prompt_line(prompt: &str) -> Result<String> {
    use std::io::{BufRead, Write};
    let mut stdout = std::io::stdout();
    stdout.write_all(prompt.as_bytes())?;
    stdout.flush()?;
    let stdin = std::io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

// ── HTTP layer ──────────────────────────────────────────────────────────

pub(crate) fn http_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent(concat!("toolpath-cli/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("failed to build HTTP client")
}

#[derive(Deserialize)]
pub(crate) struct RedeemResponse {
    pub token: String,
    pub user: User,
}

pub(crate) fn api_redeem(base_url: &str, code: &str) -> Result<(String, User)> {
    let client = http_client()?;
    let resp = client
        .post(format!("{base_url}/api/v1/auth/cli/redeem"))
        .json(&serde_json::json!({ "code": code }))
        .send()
        .with_context(|| format!("connect to {base_url}"))?;

    let status = resp.status();
    let body = resp.text().unwrap_or_default();

    if !status.is_success() {
        if status == reqwest::StatusCode::UNAUTHORIZED {
            bail!("code is invalid, already used, or expired — generate a new one");
        }
        if status == reqwest::StatusCode::BAD_REQUEST {
            let msg = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(String::from))
                .unwrap_or_else(|| body.clone());
            bail!("{msg}");
        }
        bail!("redeem failed ({status}): {body}");
    }

    let parsed: RedeemResponse =
        serde_json::from_str(&body).with_context(|| format!("parsing redeem response: {body}"))?;
    Ok((parsed.token, parsed.user))
}

pub(crate) fn api_logout(base_url: &str, token: &str) -> Result<()> {
    let client = http_client()?;
    let resp = client
        .post(format!("{base_url}/api/v1/auth/logout"))
        .bearer_auth(token)
        .send()
        .with_context(|| format!("connect to {base_url}"))?;
    if !resp.status().is_success() && resp.status() != reqwest::StatusCode::NO_CONTENT {
        bail!("server returned {}", resp.status());
    }
    Ok(())
}

pub(crate) fn api_me(base_url: &str, token: &str) -> Result<User> {
    let client = http_client()?;
    let resp = client
        .get(format!("{base_url}/api/v1/auth/me"))
        .bearer_auth(token)
        .send()
        .with_context(|| format!("connect to {base_url}"))?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        bail!("stored session is no longer valid — run `path auth login` again");
    }
    if !resp.status().is_success() {
        bail!("server returned {}", resp.status());
    }
    let user: User = resp.json().context("parsing /auth/me response")?;
    Ok(user)
}

/// `POST /api/v1/traces` — upload a toolpath document.
pub(crate) fn traces_post(base_url: &str, token: &str, body: &str) -> Result<TraceRef> {
    let client = http_client()?;
    let resp = client
        .post(format!("{base_url}/api/v1/traces"))
        .bearer_auth(token)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .with_context(|| format!("connect to {base_url}"))?;

    let status = resp.status();
    let text = resp.text().unwrap_or_default();

    if status == reqwest::StatusCode::UNAUTHORIZED {
        bail!("stored session is no longer valid — run `path auth login` again");
    }
    if !status.is_success() {
        let msg = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(String::from))
            .unwrap_or_else(|| text.clone());
        bail!("upload failed ({status}): {msg}");
    }

    serde_json::from_str(&text).with_context(|| format!("parsing upload response: {text}"))
}

/// `GET /api/v1/traces/{id}` — download a toolpath document body (JSON).
pub(crate) fn traces_get(base_url: &str, token: &str, id: &str) -> Result<String> {
    let client = http_client()?;
    let resp = client
        .get(format!("{base_url}/api/v1/traces/{id}"))
        .bearer_auth(token)
        .send()
        .with_context(|| format!("connect to {base_url}"))?;

    let status = resp.status();
    let text = resp.text().unwrap_or_default();

    if status == reqwest::StatusCode::UNAUTHORIZED {
        bail!("stored session is no longer valid — run `path auth login` again");
    }
    if status == reqwest::StatusCode::NOT_FOUND {
        bail!("trace {id} not found on {base_url}");
    }
    if !status.is_success() {
        let msg = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(String::from))
            .unwrap_or_else(|| text.clone());
        bail!("download failed ({status}): {msg}");
    }

    Ok(text)
}

// ── File storage ────────────────────────────────────────────────────────

/// The configured toolpath config directory (default `~/.toolpath`,
/// overridable via `$TOOLPATH_CONFIG_DIR`).
pub(crate) fn config_dir() -> Result<PathBuf> {
    if let Some(override_) = std::env::var_os(CONFIG_DIR_ENV) {
        return Ok(PathBuf::from(override_));
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| anyhow!("$HOME is not set — cannot locate config directory"))?;
    Ok(PathBuf::from(home).join(CONFIG_DIR_NAME))
}

pub(crate) fn credentials_path() -> Result<PathBuf> {
    Ok(config_dir()?.join(CREDENTIALS_FILE))
}

pub(crate) fn store_session(path: &Path, s: &StoredSession) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("credentials path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("create {}", parent.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
    }

    let payload = serde_json::to_string_pretty(s)?;
    std::fs::write(path, payload).with_context(|| format!("write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("chmod 0600 {}", path.display()))?;
    }
    Ok(())
}

pub(crate) fn load_session(path: &Path) -> Result<Option<StoredSession>> {
    match std::fs::read_to_string(path) {
        Ok(s) if s.trim().is_empty() => Ok(None),
        Ok(s) => Ok(Some(serde_json::from_str(&s).with_context(|| {
            format!("decode credentials at {}", path.display())
        })?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow!("read {}: {e}", path.display())),
    }
}

pub(crate) fn clear_session(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(anyhow!("remove {}: {e}", path.display())),
    }
}

/// Load the stored session or bail with a helpful message.
pub(crate) fn require_session() -> Result<StoredSession> {
    let path = credentials_path()?;
    load_session(&path)?.ok_or_else(|| anyhow!("Not logged in. Run `path auth login`."))
}

/// Shared lock for tests that manipulate `$TOOLPATH_CONFIG_DIR`. Every
/// test module that calls `set_var`/`remove_var` on this env var should
/// grab this lock first, otherwise parallel tests race and clobber each
/// other's directories.
#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> StoredSession {
        StoredSession {
            url: "https://pathbase.dev".into(),
            token: "tok".into(),
            user: User {
                id: "u1".into(),
                username: "alice".into(),
                email: Some("alice@example.com".into()),
                display_name: None,
                avatar_url: None,
            },
        }
    }

    #[test]
    fn resolve_url_prefers_cli_flag() {
        let got = resolve_url(Some("https://example.com/".into()));
        assert_eq!(got, "https://example.com");
    }

    #[test]
    fn store_then_load_roundtrips_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.json");
        assert!(load_session(&path).unwrap().is_none());
        store_session(&path, &sample()).unwrap();
        let back = load_session(&path).unwrap().unwrap();
        assert_eq!(back.user.username, "alice");
        assert_eq!(back.token, "tok");
    }

    #[test]
    fn store_creates_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("dir").join("credentials.json");
        store_session(&path, &sample()).unwrap();
        assert!(path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn store_sets_restrictive_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.json");
        store_session(&path, &sample()).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600 on credentials file, got {mode:o}");
    }

    #[test]
    fn clear_on_missing_file_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.json");
        assert!(clear_session(&path).is_ok());
    }

    #[test]
    fn load_empty_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.json");
        std::fs::write(&path, "").unwrap();
        assert!(load_session(&path).unwrap().is_none());
    }

    #[test]
    fn config_dir_honors_override() {
        let _g = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var(CONFIG_DIR_ENV, "/tmp/test-toolpath");
        }
        let dir = config_dir().unwrap();
        unsafe {
            std::env::remove_var(CONFIG_DIR_ENV);
        }
        assert_eq!(dir, PathBuf::from("/tmp/test-toolpath"));
    }
}

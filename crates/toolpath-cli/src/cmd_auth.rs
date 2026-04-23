use anyhow::{Context, Result, anyhow, bail};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const CONFIG_DIR_NAME: &str = ".toolpath";
const CREDENTIALS_FILE: &str = "credentials.json";
const CONFIG_DIR_ENV: &str = "TOOLPATH_CONFIG_DIR";
const DEFAULT_URL: &str = "https://pathbase.dev";
const PATHBASE_URL_ENV: &str = "PATHBASE_URL";

#[derive(Subcommand, Debug)]
pub enum AuthOp {
    /// Log in by opening a browser to Pathbase and pasting the displayed code
    Login {
        /// Pathbase server URL (defaults to $PATHBASE_URL or https://pathbase.dev)
        #[arg(long)]
        url: Option<String>,

        /// Paste the code directly instead of prompting
        #[arg(long)]
        code: Option<String>,
    },
    /// Log out and clear the stored session
    Logout,
    /// Show the stored session's server URL and cached user
    Status,
    /// Verify the stored session against the server and print the current user
    Whoami,
}

/// JSON blob persisted in the OS keychain.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSession {
    url: String,
    token: String,
    user: User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id: String,
    username: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    avatar_url: Option<String>,
}

pub fn run(op: AuthOp) -> Result<()> {
    let path = credentials_path()?;
    match op {
        AuthOp::Login { url, code } => login(&path, url, code),
        AuthOp::Logout => logout(&path),
        AuthOp::Status => status(&path),
        AuthOp::Whoami => whoami(&path),
    }
}

fn login(path: &Path, url: Option<String>, code_arg: Option<String>) -> Result<()> {
    let base_url = resolve_url(url);
    let auth_url = format!("{base_url}/auth/cli");

    let code = match code_arg {
        Some(c) => c,
        None => {
            println!("To connect this CLI to Pathbase:");
            println!();
            println!("  1. Open {auth_url} in your browser");
            println!("  2. Sign in if prompted");
            println!("  3. Copy the 8-character code shown on that page");
            println!();
            prompt_line("Paste code: ")?
        }
    };

    let (token, user) = api_redeem(&base_url, &code)?;
    store_session(
        path,
        &StoredSession {
            url: base_url.clone(),
            token,
            user: user.clone(),
        },
    )?;

    println!(
        "Logged in to {} as {}{}",
        base_url,
        user.username,
        user.email
            .as_deref()
            .map(|e| format!(" ({e})"))
            .unwrap_or_default()
    );
    println!("Credentials saved to {}", path.display());
    Ok(())
}

fn logout(path: &Path) -> Result<()> {
    let stored = match load_session(path)? {
        Some(s) => s,
        None => {
            println!("Not logged in.");
            return Ok(());
        }
    };

    // Best effort: tell the server to invalidate the session.
    if let Err(e) = api_logout(&stored.url, &stored.token) {
        eprintln!("warning: server logout failed: {e}");
    }

    clear_session(path)?;
    println!("Logged out.");
    Ok(())
}

fn status(path: &Path) -> Result<()> {
    match load_session(path)? {
        Some(s) => {
            println!("Logged in to {} as {}", s.url, s.user.username);
            if let Some(email) = &s.user.email {
                println!("  email: {email}");
            }
            println!("  user id: {}", s.user.id);
            println!("  credentials: {}", path.display());
            Ok(())
        }
        None => {
            println!("Not logged in. Run `path auth login`.");
            Ok(())
        }
    }
}

fn whoami(path: &Path) -> Result<()> {
    let stored = load_session(path)?
        .ok_or_else(|| anyhow!("Not logged in. Run `path auth login`."))?;
    let user = api_me(&stored.url, &stored.token)?;
    println!("{} ({})", user.username, user.id);
    if let Some(email) = &user.email {
        println!("email: {email}");
    }
    println!("server: {}", stored.url);
    Ok(())
}

// ── URL + prompt helpers ────────────────────────────────────────────────

fn resolve_url(cli_url: Option<String>) -> String {
    let raw = cli_url
        .or_else(|| std::env::var(PATHBASE_URL_ENV).ok())
        .unwrap_or_else(|| DEFAULT_URL.to_string());
    raw.trim_end_matches('/').to_string()
}

fn prompt_line(prompt: &str) -> Result<String> {
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

fn http_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent(concat!("toolpath-cli/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("failed to build HTTP client")
}

#[derive(Deserialize)]
struct RedeemResponse {
    token: String,
    user: User,
}

fn api_redeem(base_url: &str, code: &str) -> Result<(String, User)> {
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
            // server returns a JSON {"error": "..."} we can surface
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

fn api_logout(base_url: &str, token: &str) -> Result<()> {
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

fn api_me(base_url: &str, token: &str) -> Result<User> {
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

// ── File storage ────────────────────────────────────────────────────────

fn credentials_path() -> Result<PathBuf> {
    if let Some(override_) = std::env::var_os(CONFIG_DIR_ENV) {
        return Ok(PathBuf::from(override_).join(CREDENTIALS_FILE));
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| anyhow!("$HOME is not set — cannot locate credentials"))?;
    Ok(PathBuf::from(home)
        .join(CONFIG_DIR_NAME)
        .join(CREDENTIALS_FILE))
}

fn store_session(path: &Path, s: &StoredSession) -> Result<()> {
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

fn load_session(path: &Path) -> Result<Option<StoredSession>> {
    match std::fs::read_to_string(path) {
        Ok(s) if s.trim().is_empty() => Ok(None),
        Ok(s) => Ok(Some(serde_json::from_str(&s).with_context(|| {
            format!("decode credentials at {}", path.display())
        })?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow!("read {}: {e}", path.display())),
    }
}

fn clear_session(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(anyhow!("remove {}: {e}", path.display())),
    }
}

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
}

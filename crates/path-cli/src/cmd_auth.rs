use anyhow::{Result, anyhow};
use clap::Subcommand;
use std::path::Path;

use crate::cmd_pathbase::{
    PATHBASE_TOKEN_ENV, SessionSource, StoredSession, api_logout, api_me, api_redeem,
    clear_session, credentials_path, load_session, prompt_line, resolve_session, resolve_url,
    store_session,
};

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
    /// Show the active session's server URL, credential source, and cached user
    Status,
    /// Verify the active session against the server and print the current user
    Whoami,
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

/// Logout is deliberately file-only: there is no clearing a variable this
/// process does not own. When a token is still exported, say so — otherwise
/// "Logged out." is a lie the very next `path share` disproves.
fn logout(path: &Path) -> Result<()> {
    let stored = match load_session(path)? {
        Some(s) => s,
        None => {
            println!("Not logged in.");
            warn_if_env_token_shadows();
            return Ok(());
        }
    };

    if let Err(e) = api_logout(&stored.url, &stored.token) {
        eprintln!("warning: server logout failed: {e}");
    }

    clear_session(path)?;
    println!("Logged out.");
    warn_if_env_token_shadows();
    Ok(())
}

/// Point at the env token whenever it is set, for commands whose effect it
/// silently overrides.
fn warn_if_env_token_shadows() {
    if std::env::var(PATHBASE_TOKEN_ENV)
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
    {
        eprintln!(
            "note: {PATHBASE_TOKEN_ENV} is still set in this environment — \
             Pathbase commands will keep authenticating with it. \
             Unset it to fall back to the stored credentials file."
        );
    }
}

fn status(path: &Path) -> Result<()> {
    let session = match resolve_session(path)? {
        Some(s) => s,
        None => {
            println!(
                "Not logged in. Run `path auth login`, or set {PATHBASE_TOKEN_ENV} for \
                 unattended use."
            );
            return Ok(());
        }
    };

    match &session.user {
        // Stored credentials carry the identity that was resolved at login.
        Some(user) => {
            println!("Logged in to {} as {}", session.url, user.username);
            if let Some(email) = &user.email {
                println!("  email: {email}");
            }
            println!("  user id: {}", user.id);
        }
        // An env token carries no identity — do not invent one. `whoami` asks
        // the server, which is the only place the answer actually lives.
        None => {
            println!("Authenticated to {} via {}", session.url, session.source.describe());
            println!("  user: not known locally — run `path auth whoami` to ask the server");
        }
    }

    println!("  credential: {}", describe_credential(&session, path));

    // The shadowing case is the one worth spelling out: a stale credentials.json
    // sitting under an exported token looks like the account you are using.
    if session.source == SessionSource::Env
        && let Ok(Some(file)) = load_session(path)
    {
        println!(
            "  note: {} (stored login as {}) is ignored while {PATHBASE_TOKEN_ENV} is set",
            path.display(),
            file.user.username
        );
    }
    Ok(())
}

fn describe_credential(session: &crate::cmd_pathbase::Session, path: &Path) -> String {
    match session.source {
        SessionSource::Env => format!("{PATHBASE_TOKEN_ENV} (environment)"),
        SessionSource::File => path.display().to_string(),
    }
}

fn whoami(path: &Path) -> Result<()> {
    let session = resolve_session(path)?.ok_or_else(|| {
        anyhow!("Not logged in. Run `path auth login` or set {PATHBASE_TOKEN_ENV}.")
    })?;
    let user = api_me(&session.url, &session.token)?;
    println!("{} ({})", user.username, user.id);
    if let Some(email) = &user.email {
        println!("email: {email}");
    }
    println!("server: {}", session.url);
    println!("credential: {}", describe_credential(&session, path));
    Ok(())
}

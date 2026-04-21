use crate::error::{DesktopError, DesktopResult};

const SERVICE: &str = "dev.pathbase.toolpath-desktop";
const ACCOUNT: &str = "github-pat";

fn entry() -> DesktopResult<keyring::Entry> {
    keyring::Entry::new(SERVICE, ACCOUNT)
        .map_err(|e| DesktopError::Keychain(format!("open entry: {e}")))
}

/// Backend-only helper used by `derive::derive_github`. Not exposed to the
/// frontend because we never want a raw token crossing the IPC boundary.
pub(crate) fn read_github_token() -> DesktopResult<String> {
    let entry = entry()?;
    match entry.get_password() {
        Ok(tok) if !tok.is_empty() => Ok(tok),
        Ok(_) => Err(DesktopError::Auth("GitHub token is empty".into())),
        Err(keyring::Error::NoEntry) => Err(DesktopError::Auth("GitHub token not set".into())),
        Err(e) => Err(DesktopError::Keychain(format!("read: {e}"))),
    }
}

#[tauri::command]
pub fn github_set_token(token: String) -> DesktopResult<()> {
    if token.trim().is_empty() {
        return Err(DesktopError::InvalidInput("token is empty".into()));
    }
    let entry = entry()?;
    entry
        .set_password(token.trim())
        .map_err(|e| DesktopError::Keychain(format!("write: {e}")))?;
    Ok(())
}

#[tauri::command]
pub fn github_has_token() -> DesktopResult<bool> {
    let entry = entry()?;
    match entry.get_password() {
        Ok(tok) => Ok(!tok.is_empty()),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(e) => Err(DesktopError::Keychain(format!("probe: {e}"))),
    }
}

#[tauri::command]
pub fn github_clear_token() -> DesktopResult<()> {
    let entry = entry()?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(DesktopError::Keychain(format!("delete: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_token_rejects_empty_string() {
        let err = github_set_token("   ".into()).unwrap_err();
        assert!(matches!(err, DesktopError::InvalidInput(_)));
    }
}

//! Error types for `toolpath-pi`.

use std::path::PathBuf;
use thiserror::Error;

/// Errors produced by the `toolpath-pi` crate.
#[derive(Debug, Error)]
pub enum PiError {
    /// Underlying I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON (de)serialization failure.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// A session file was expected but could not be located.
    #[error("session not found: {0}")]
    SessionNotFound(String),

    /// A project directory (encoded cwd) was expected but not found on disk.
    #[error("project not found: {0}")]
    ProjectNotFound(String),

    /// A session JSONL file exists but cannot be interpreted.
    ///
    /// Carries the offending path and a short human-readable reason.
    #[error("invalid session file {path}: {reason}")]
    InvalidSessionFile {
        /// Path to the offending file.
        path: PathBuf,
        /// Short human-readable reason.
        reason: String,
    },

    /// A session header line was present but malformed (missing required fields,
    /// unexpected shape, etc.).
    #[error("malformed session header: {0}")]
    MalformedHeader(String),

    /// Wrapped error from `toolpath-convo`.
    #[error("conversation error: {0}")]
    Convo(#[from] toolpath_convo::ConvoError),

    /// Catch-all for arbitrary `anyhow` errors bubbling up from dependencies.
    #[error("{0}")]
    Anyhow(#[from] anyhow::Error),

    /// Generic free-form error.
    #[error("{0}")]
    Other(String),
}

impl PiError {
    /// Construct a `SessionNotFound` error.
    pub fn session_not_found(id: impl Into<String>) -> Self {
        Self::SessionNotFound(id.into())
    }

    /// Construct a `ProjectNotFound` error.
    pub fn project_not_found(cwd: impl Into<String>) -> Self {
        Self::ProjectNotFound(cwd.into())
    }

    /// Construct an `InvalidSessionFile` error.
    pub fn invalid_session_file(path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        Self::InvalidSessionFile {
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// Construct a `MalformedHeader` error.
    pub fn malformed_header(reason: impl Into<String>) -> Self {
        Self::MalformedHeader(reason.into())
    }

    /// Construct an `Other` error.
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, PiError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "missing");
        let err: PiError = io_err.into();
        match err {
            PiError::Io(_) => {}
            _ => panic!("expected Io variant"),
        }
    }

    #[test]
    fn test_json_error_display() {
        let json_err = serde_json::from_str::<u32>("x").unwrap_err();
        let err: PiError = json_err.into();
        let msg = err.to_string();
        assert!(
            msg.to_lowercase().contains("json"),
            "expected 'json' in display: {msg}"
        );
    }

    #[test]
    fn test_session_not_found_display() {
        let err = PiError::SessionNotFound("abc".into());
        assert!(err.to_string().contains("abc"));
    }

    #[test]
    fn test_project_not_found_display() {
        let err = PiError::ProjectNotFound("/Users/alex/project".into());
        assert!(err.to_string().contains("/Users/alex/project"));
    }

    #[test]
    fn test_other_display() {
        let err = PiError::Other("something went wrong".into());
        assert!(err.to_string().contains("something went wrong"));
    }

    #[test]
    fn test_invalid_session_file_display() {
        let err = PiError::invalid_session_file(PathBuf::from("/tmp/a.jsonl"), "bad line 3");
        let msg = err.to_string();
        assert!(msg.contains("/tmp/a.jsonl"));
        assert!(msg.contains("bad line 3"));
    }

    #[test]
    fn test_malformed_header_display() {
        let err = PiError::malformed_header("missing session_id");
        assert!(err.to_string().contains("missing session_id"));
    }

    #[test]
    fn test_anyhow_conversion() {
        let a: anyhow::Error = anyhow::anyhow!("boom");
        let err: PiError = a.into();
        assert!(err.to_string().contains("boom"));
    }

    #[test]
    fn test_helper_constructors() {
        assert!(matches!(
            PiError::session_not_found("s"),
            PiError::SessionNotFound(_)
        ));
        assert!(matches!(
            PiError::project_not_found("p"),
            PiError::ProjectNotFound(_)
        ));
        assert!(matches!(PiError::other("o"), PiError::Other(_)));
    }
}

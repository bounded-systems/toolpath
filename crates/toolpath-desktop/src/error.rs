use serde::Serialize;
use thiserror::Error;

/// Error type returned across the Tauri IPC boundary.
///
/// All command results use `Result<T, DesktopError>`. Errors are serialised to
/// JSON with a stable `code` discriminator so the frontend can branch on it
/// without string-matching.
#[derive(Debug, Error)]
pub enum DesktopError {
    #[error("source error: {0}")]
    Source(String),

    #[error("derive error: {0}")]
    Derive(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("io error: {0}")]
    Io(String),

    #[error("auth error: {0}")]
    Auth(String),

    #[error("keychain error: {0}")]
    Keychain(String),

    #[error("upload error: {0}")]
    Upload(String),
}

impl DesktopError {
    fn code(&self) -> &'static str {
        match self {
            Self::Source(_) => "source",
            Self::Derive(_) => "derive",
            Self::InvalidInput(_) => "invalid_input",
            Self::Io(_) => "io",
            Self::Auth(_) => "auth",
            Self::Keychain(_) => "keychain",
            Self::Upload(_) => "upload",
        }
    }
}

impl Serialize for DesktopError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("DesktopError", 2)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

impl From<std::io::Error> for DesktopError {
    fn from(err: std::io::Error) -> Self {
        DesktopError::Io(err.to_string())
    }
}

impl From<serde_json::Error> for DesktopError {
    fn from(err: serde_json::Error) -> Self {
        DesktopError::Io(format!("json: {err}"))
    }
}

impl From<anyhow::Error> for DesktopError {
    fn from(err: anyhow::Error) -> Self {
        DesktopError::Derive(format!("{err:#}"))
    }
}

pub type DesktopResult<T> = std::result::Result<T, DesktopError>;

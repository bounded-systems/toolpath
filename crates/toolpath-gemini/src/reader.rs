//! Read and parse Gemini CLI chat files and project logs.

use crate::error::{ConvoError, Result};
use crate::types::{ChatFile, LogEntry};
use std::fs;
use std::path::Path;

pub struct ConversationReader;

impl ConversationReader {
    /// Parse a single chat JSON file.
    pub fn read_chat_file<P: AsRef<Path>>(path: P) -> Result<ChatFile> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(ConvoError::ConversationNotFound(path.display().to_string()));
        }
        let bytes = fs::read(path)?;
        let chat: ChatFile = serde_json::from_slice(&bytes)
            .map_err(|e| ConvoError::Other(anyhow::anyhow!("{}: {}", path.display(), e)))?;
        Ok(chat)
    }

    /// Return the byte-length of a chat file on disk.
    pub fn file_size<P: AsRef<Path>>(path: P) -> Result<u64> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(ConvoError::ConversationNotFound(path.display().to_string()));
        }
        Ok(fs::metadata(path)?.len())
    }

    /// Parse `logs.json` — a JSON array of lightweight log entries.
    /// Returns an empty vec when the file is absent or malformed (the log
    /// is auxiliary; callers should not fail the whole operation when it's
    /// missing).
    pub fn read_logs<P: AsRef<Path>>(path: P) -> Result<Vec<LogEntry>> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let bytes = fs::read(path)?;
        match serde_json::from_slice::<Vec<LogEntry>>(&bytes) {
            Ok(v) => Ok(v),
            Err(e) => {
                eprintln!(
                    "Warning: Failed to parse Gemini log file {}: {}",
                    path.display(),
                    e
                );
                Ok(Vec::new())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_chat(body: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(body.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn test_read_chat_file() {
        let f = write_chat(
            r#"{"sessionId":"s","projectHash":"h","messages":[{"id":"m","timestamp":"t","type":"user","content":"hi"}]}"#,
        );
        let chat = ConversationReader::read_chat_file(f.path()).unwrap();
        assert_eq!(chat.session_id, "s");
        assert_eq!(chat.messages.len(), 1);
    }

    #[test]
    fn test_read_chat_file_nonexistent() {
        let err = ConversationReader::read_chat_file("/nonexistent.json").unwrap_err();
        matches!(err, ConvoError::ConversationNotFound(_));
    }

    #[test]
    fn test_read_chat_file_invalid_json() {
        let f = write_chat("not json");
        let err = ConversationReader::read_chat_file(f.path()).unwrap_err();
        matches!(err, ConvoError::Other(_));
    }

    #[test]
    fn test_read_logs() {
        let f = write_chat(
            r#"[{"sessionId":"s","messageId":0,"type":"user","message":"hi","timestamp":"t"}]"#,
        );
        let logs = ConversationReader::read_logs(f.path()).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].message, "hi");
    }

    #[test]
    fn test_read_logs_absent() {
        let logs = ConversationReader::read_logs("/nonexistent.json").unwrap();
        assert!(logs.is_empty());
    }

    #[test]
    fn test_read_logs_malformed_returns_empty() {
        let f = write_chat("{");
        let logs = ConversationReader::read_logs(f.path()).unwrap();
        assert!(logs.is_empty());
    }

    #[test]
    fn test_file_size() {
        let f = write_chat(r#"{"messages":[]}"#);
        let size = ConversationReader::file_size(f.path()).unwrap();
        assert!(size > 0);
    }

    #[test]
    fn test_file_size_nonexistent() {
        let err = ConversationReader::file_size("/nonexistent.json").unwrap_err();
        matches!(err, ConvoError::ConversationNotFound(_));
    }
}

//! Parse Codex rollout JSONL files.
//!
//! The writer is append-only but backgrounded — a crashed Codex
//! process may leave the final line mid-write. We skip unparseable
//! lines by default and surface them as warnings rather than failing
//! the whole read.

use crate::error::{ConvoError, Result};
use crate::types::{RolloutLine, Session};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

pub struct RolloutReader;

impl RolloutReader {
    /// Read every line of a rollout file into a [`Session`].
    ///
    /// The session id is taken from the first line's `session_meta`
    /// payload if present; otherwise from the filename stem.
    pub fn read_session<P: AsRef<Path>>(path: P) -> Result<Session> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(ConvoError::SessionNotFound(path.display().to_string()));
        }

        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut lines: Vec<RolloutLine> = Vec::new();
        for (idx, raw) in reader.lines().enumerate() {
            let raw = match raw {
                Ok(s) => s,
                Err(e) => {
                    eprintln!(
                        "Warning: IO error reading {} line {}: {}",
                        path.display(),
                        idx + 1,
                        e
                    );
                    continue;
                }
            };
            if raw.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<RolloutLine>(&raw) {
                Ok(line) => lines.push(line),
                Err(e) => {
                    // Tolerate a single truncated last line (common after crashes);
                    // warn about anything else.
                    if std::env::var("CODEX_ROLLOUT_STRICT").is_ok() {
                        return Err(ConvoError::Json(e));
                    }
                    eprintln!(
                        "Warning: unparseable rollout line {} in {}: {}",
                        idx + 1,
                        path.file_name().and_then(|n| n.to_str()).unwrap_or("<?>"),
                        e
                    );
                }
            }
        }

        let id = Self::derive_session_id(&lines, path);
        Ok(Session {
            id,
            file_path: path.to_path_buf(),
            lines,
        })
    }

    /// Peek just the first `session_meta` payload without fully parsing
    /// the rest of the file. Returns the session id if found.
    pub fn peek_session_id<P: AsRef<Path>>(path: P) -> Option<String> {
        let file = File::open(path).ok()?;
        let mut reader = BufReader::new(file);
        let mut first = String::new();
        reader.read_line(&mut first).ok()?;
        let line: RolloutLine = serde_json::from_str(first.trim()).ok()?;
        if line.kind != "session_meta" {
            return None;
        }
        line.payload
            .get("id")
            .and_then(|v| v.as_str())
            .map(str::to_string)
    }

    /// Return the byte-length of a rollout file.
    pub fn file_size<P: AsRef<Path>>(path: P) -> Result<u64> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(ConvoError::SessionNotFound(path.display().to_string()));
        }
        Ok(std::fs::metadata(path)?.len())
    }

    fn derive_session_id(lines: &[RolloutLine], path: &Path) -> String {
        // Prefer the session_meta payload.
        if let Some(first) = lines.first()
            && first.kind == "session_meta"
            && let Some(id) = first.payload.get("id").and_then(|v| v.as_str())
        {
            return id.to_string();
        }
        // Fall back to the UUID suffix of the filename stem.
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            return crate::paths::session_id_from_stem(stem).to_string();
        }
        "unknown".to_string()
    }
}

/// Type alias exposed for consumers to avoid re-importing `PathBuf`.
pub type RolloutPath = PathBuf;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn sample_rollout() -> String {
        [
            r#"{"timestamp":"2026-04-20T16:44:37.772Z","type":"session_meta","payload":{"id":"019dabc6-8fef-7681-a054-b5bb75fcb97d","timestamp":"2026-04-20T16:43:30.171Z","cwd":"/tmp/proj","originator":"codex-tui","cli_version":"0.118.0","source":"cli"}}"#,
            r#"{"timestamp":"2026-04-20T16:44:37.773Z","type":"turn_context","payload":{"turn_id":"019dabc7","cwd":"/tmp/proj"}}"#,
            r#"{"timestamp":"2026-04-20T16:44:37.775Z","type":"event_msg","payload":{"type":"task_started","turn_id":"019dabc7"}}"#,
            r#"{"timestamp":"2026-04-20T16:44:38.000Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}}"#,
        ]
        .join("\n")
    }

    fn write_fixture(body: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(body.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn read_session_basic() {
        let f = write_fixture(&sample_rollout());
        let s = RolloutReader::read_session(f.path()).unwrap();
        assert_eq!(s.id, "019dabc6-8fef-7681-a054-b5bb75fcb97d");
        assert_eq!(s.lines.len(), 4);
        assert!(s.meta().is_some());
    }

    #[test]
    fn read_session_nonexistent_errors() {
        let err = RolloutReader::read_session("/nonexistent").unwrap_err();
        assert!(matches!(err, ConvoError::SessionNotFound(_)));
    }

    /// Serializes access to `CODEX_ROLLOUT_STRICT` across tests in this
    /// module. Two tests probe `read_session` with opposing strictness
    /// expectations; without serialization, cargo test's threaded
    /// runner can observe the env var set by one test during another.
    fn strict_env_lock() -> std::sync::MutexGuard<'static, ()> {
        use std::sync::{Mutex, OnceLock};
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    #[test]
    fn read_session_handles_truncated_last_line() {
        let _g = strict_env_lock();
        // Belt-and-braces: even under the lock, make sure the env var
        // is clear before we observe lenient behavior.
        unsafe { std::env::remove_var("CODEX_ROLLOUT_STRICT") };
        // Good first line, garbage second — reader skips and warns.
        let body = sample_rollout() + "\n{\"timestamp\":\"broken"; // truncated
        let f = write_fixture(&body);
        let s = RolloutReader::read_session(f.path()).unwrap();
        assert_eq!(s.lines.len(), 4, "truncated line dropped, others kept");
    }

    #[test]
    fn read_session_respects_strict_env() {
        let _g = strict_env_lock();
        let body = sample_rollout() + "\n{\"timestamp\":\"broken";
        let f = write_fixture(&body);
        unsafe { std::env::set_var("CODEX_ROLLOUT_STRICT", "1") };
        let err = RolloutReader::read_session(f.path()).unwrap_err();
        unsafe { std::env::remove_var("CODEX_ROLLOUT_STRICT") };
        assert!(matches!(err, ConvoError::Json(_)));
    }

    #[test]
    fn peek_session_id_reads_first_line_only() {
        let f = write_fixture(&sample_rollout());
        let id = RolloutReader::peek_session_id(f.path()).unwrap();
        assert_eq!(id, "019dabc6-8fef-7681-a054-b5bb75fcb97d");
    }

    #[test]
    fn peek_session_id_missing_when_first_line_not_meta() {
        let body = r#"{"timestamp":"t","type":"event_msg","payload":{"type":"x"}}"#;
        let f = write_fixture(body);
        assert!(RolloutReader::peek_session_id(f.path()).is_none());
    }

    #[test]
    fn session_started_at_and_last_activity() {
        let f = write_fixture(&sample_rollout());
        let s = RolloutReader::read_session(f.path()).unwrap();
        assert!(s.started_at().is_some());
        assert!(s.last_activity() >= s.started_at());
    }

    #[test]
    fn session_first_user_text() {
        let f = write_fixture(&sample_rollout());
        let s = RolloutReader::read_session(f.path()).unwrap();
        assert_eq!(s.first_user_text().as_deref(), Some("hello"));
    }

    #[test]
    fn file_size_works() {
        let f = write_fixture(&sample_rollout());
        let size = RolloutReader::file_size(f.path()).unwrap();
        assert!(size > 0);
    }

    #[test]
    fn derive_session_id_falls_back_to_stem_uuid() {
        let body = r#"{"timestamp":"t","type":"event_msg","payload":{"type":"x"}}"#;
        let f = NamedTempFile::new().unwrap();
        let path = f
            .path()
            .parent()
            .unwrap()
            .join("rollout-2026-04-20T10-00-00-019dabc6-8fef-7681-a054-b5bb75fcb97d.jsonl");
        std::fs::write(&path, body).unwrap();
        let s = RolloutReader::read_session(&path).unwrap();
        assert_eq!(s.id, "019dabc6-8fef-7681-a054-b5bb75fcb97d");
        // Clean up
        drop(f);
        let _ = std::fs::remove_file(path);
    }
}

//! Read-only SQLite access to an `opencode.db`.
//!
//! We always open the database with `SQLITE_OPEN_READ_ONLY` (and
//! `SQLITE_OPEN_NO_MUTEX`) so we never interfere with a live opencode
//! process writing to the same file. SQLite WAL mode allows
//! concurrent readers and a single writer — readers observe a
//! consistent snapshot as of the transaction start.

use crate::error::{ConvoError, Result};
use crate::types::{Message, MessageData, Part, PartData, Project, Session};
use rusqlite::{Connection, OpenFlags, params};
use std::path::{Path, PathBuf};

/// Thin wrapper around a rusqlite connection opened read-only
/// against an opencode database.
pub struct DbReader {
    conn: Connection,
    path: PathBuf,
}

impl DbReader {
    /// Open the opencode database at `path` read-only.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(ConvoError::DatabaseNotFound(path.to_path_buf()));
        }
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Ok(Self {
            conn,
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Enumerate every project. Newest first by `time_updated`.
    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, worktree, vcs, name, time_created, time_updated, time_initialized, sandboxes
             FROM project
             ORDER BY time_updated DESC",
        )?;
        let rows = stmt.query_map([], Self::map_project)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get_project(&self, id: &str) -> Result<Option<Project>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, worktree, vcs, name, time_created, time_updated, time_initialized, sandboxes
             FROM project WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], Self::map_project)?;
        rows.next().transpose().map_err(ConvoError::from)
    }

    fn map_project(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
        let sandboxes_json: String = row.get::<_, Option<String>>(7)?.unwrap_or("[]".to_string());
        let sandboxes: Vec<String> = serde_json::from_str(&sandboxes_json).unwrap_or_default();
        Ok(Project {
            id: row.get(0)?,
            worktree: PathBuf::from(row.get::<_, String>(1)?),
            vcs: row.get(2)?,
            name: row.get(3)?,
            time_created: row.get(4)?,
            time_updated: row.get(5)?,
            time_initialized: row.get(6)?,
            sandboxes,
        })
    }

    /// Enumerate sessions, optionally filtered by project id.
    /// Newest first by `time_updated`.
    pub fn list_sessions(&self, project_id: Option<&str>) -> Result<Vec<Session>> {
        let sql = "SELECT id, project_id, workspace_id, parent_id, slug, directory, title,
                          version, share_url, summary_additions, summary_deletions,
                          summary_files, time_created, time_updated, time_compacting, time_archived
                   FROM session";
        let rows: Vec<Session> = if let Some(pid) = project_id {
            let mut stmt = self.conn.prepare(&format!(
                "{sql} WHERE project_id = ?1 ORDER BY time_updated DESC"
            ))?;
            stmt.query_map(params![pid], Self::map_session)?
                .collect::<rusqlite::Result<Vec<_>>>()?
        } else {
            let mut stmt = self
                .conn
                .prepare(&format!("{sql} ORDER BY time_updated DESC"))?;
            stmt.query_map([], Self::map_session)?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        Ok(rows)
    }

    pub fn get_session(&self, id: &str) -> Result<Option<Session>> {
        let sql = "SELECT id, project_id, workspace_id, parent_id, slug, directory, title,
                          version, share_url, summary_additions, summary_deletions,
                          summary_files, time_created, time_updated, time_compacting, time_archived
                   FROM session WHERE id = ?1";
        let mut stmt = self.conn.prepare(sql)?;
        let mut rows = stmt.query_map(params![id], Self::map_session)?;
        rows.next().transpose().map_err(ConvoError::from)
    }

    fn map_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<Session> {
        Ok(Session {
            id: row.get(0)?,
            project_id: row.get(1)?,
            workspace_id: row.get(2)?,
            parent_id: row.get(3)?,
            slug: row.get(4)?,
            directory: PathBuf::from(row.get::<_, String>(5)?),
            title: row.get(6)?,
            version: row.get(7)?,
            share_url: row.get(8)?,
            summary_additions: row.get(9)?,
            summary_deletions: row.get(10)?,
            summary_files: row.get(11)?,
            time_created: row.get(12)?,
            time_updated: row.get(13)?,
            time_compacting: row.get(14)?,
            time_archived: row.get(15)?,
            messages: Vec::new(),
        })
    }

    /// List raw messages (without parts) for a session in
    /// `(time_created ASC, id ASC)` order.
    pub fn list_messages_raw(&self, session_id: &str) -> Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, time_created, time_updated, data
             FROM message
             WHERE session_id = ?1
             ORDER BY time_created ASC, id ASC",
        )?;
        let rows = stmt
            .query_map(params![session_id], Self::map_message)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    fn map_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
        let raw_data: String = row.get(4)?;
        let data = match serde_json::from_str::<MessageData>(&raw_data) {
            Ok(d) => d,
            Err(e) => {
                // Never fail a full read because of one malformed row;
                // surface it as Other and let the provider layer drop it
                // if it wants to be strict.
                eprintln!(
                    "Warning: message {} has malformed data: {}",
                    row.get::<_, String>(0)?,
                    e
                );
                MessageData::Other
            }
        };
        Ok(Message {
            id: row.get(0)?,
            session_id: row.get(1)?,
            time_created: row.get(2)?,
            time_updated: row.get(3)?,
            data,
            parts: Vec::new(),
        })
    }

    /// List all parts for a message in `(time_created ASC, id ASC)`
    /// order.
    pub fn list_parts_for_message(&self, message_id: &str) -> Result<Vec<Part>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, message_id, session_id, time_created, time_updated, data
             FROM part
             WHERE message_id = ?1
             ORDER BY time_created ASC, id ASC",
        )?;
        let rows = stmt
            .query_map(params![message_id], Self::map_part)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// List all parts for an entire session in
    /// `(message_id, time_created, id)` order. More efficient than
    /// N message queries when you want the whole session.
    pub fn list_parts_for_session(&self, session_id: &str) -> Result<Vec<Part>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, message_id, session_id, time_created, time_updated, data
             FROM part
             WHERE session_id = ?1
             ORDER BY message_id ASC, time_created ASC, id ASC",
        )?;
        let rows = stmt
            .query_map(params![session_id], Self::map_part)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    fn map_part(row: &rusqlite::Row<'_>) -> rusqlite::Result<Part> {
        let raw_data: String = row.get(5)?;
        let data = match serde_json::from_str::<PartData>(&raw_data) {
            Ok(d) => d,
            Err(e) => {
                eprintln!(
                    "Warning: part {} has malformed data: {}",
                    row.get::<_, String>(0)?,
                    e
                );
                PartData::Unknown
            }
        };
        Ok(Part {
            id: row.get(0)?,
            message_id: row.get(1)?,
            session_id: row.get(2)?,
            time_created: row.get(3)?,
            time_updated: row.get(4)?,
            data,
        })
    }

    /// Convenience: load a session fully — messages with their parts
    /// attached, in order.
    pub fn load_session(&self, session_id: &str) -> Result<Session> {
        let mut session = self
            .get_session(session_id)?
            .ok_or_else(|| ConvoError::SessionNotFound(session_id.to_string()))?;
        let mut messages = self.list_messages_raw(session_id)?;
        let parts = self.list_parts_for_session(session_id)?;
        // Group parts by message_id.
        let mut by_msg: std::collections::HashMap<String, Vec<Part>> =
            std::collections::HashMap::new();
        for p in parts {
            by_msg.entry(p.message_id.clone()).or_default().push(p);
        }
        for m in &mut messages {
            if let Some(ps) = by_msg.remove(&m.id) {
                m.parts = ps;
            }
        }
        session.messages = messages;
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::NamedTempFile;

    fn fixture_db() -> NamedTempFile {
        let f = NamedTempFile::new().unwrap();
        let conn = Connection::open(f.path()).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE project (
              id text PRIMARY KEY, worktree text NOT NULL, vcs text, name text,
              icon_url text, icon_color text,
              time_created integer NOT NULL, time_updated integer NOT NULL,
              time_initialized integer, sandboxes text NOT NULL, commands text
            );
            CREATE TABLE session (
              id text PRIMARY KEY, project_id text NOT NULL, parent_id text,
              slug text NOT NULL, directory text NOT NULL, title text NOT NULL,
              version text NOT NULL, share_url text,
              summary_additions integer, summary_deletions integer,
              summary_files integer, summary_diffs text, revert text, permission text,
              time_created integer NOT NULL, time_updated integer NOT NULL,
              time_compacting integer, time_archived integer, workspace_id text
            );
            CREATE TABLE message (
              id text PRIMARY KEY, session_id text NOT NULL,
              time_created integer NOT NULL, time_updated integer NOT NULL,
              data text NOT NULL
            );
            CREATE TABLE part (
              id text PRIMARY KEY, message_id text NOT NULL, session_id text NOT NULL,
              time_created integer NOT NULL, time_updated integer NOT NULL,
              data text NOT NULL
            );
            INSERT INTO project (id, worktree, time_created, time_updated, sandboxes)
              VALUES ('proj1', '/tmp/p', 1000, 2000, '[]');
            INSERT INTO session (id, project_id, slug, directory, title, version,
              time_created, time_updated)
              VALUES ('ses_1', 'proj1', 'slug', '/tmp/p', 'T', '1.0.0', 1000, 2000);
            INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES
              ('msg_1','ses_1',1001,1001,'{"role":"user","time":{"created":1001},"agent":"build","model":{"providerID":"p","modelID":"m"}}'),
              ('msg_2','ses_1',1002,1002,'{"parentID":"msg_1","role":"assistant","mode":"build","agent":"build","path":{"cwd":"/tmp/p","root":"/tmp/p"},"cost":0.01,"tokens":{"input":5,"output":3,"reasoning":0,"cache":{"read":0,"write":0}},"modelID":"m","providerID":"p","time":{"created":1002,"completed":1003},"finish":"stop"}');
            INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES
              ('prt_1','msg_1','ses_1',1001,1001,'{"type":"text","text":"hello"}'),
              ('prt_2','msg_2','ses_1',1002,1002,'{"type":"step-start","snapshot":"abc"}'),
              ('prt_3','msg_2','ses_1',1002,1002,'{"type":"text","text":"hi!"}'),
              ('prt_4','msg_2','ses_1',1003,1003,'{"type":"step-finish","reason":"stop","snapshot":"abc","tokens":{"input":5,"output":3,"reasoning":0,"cache":{"read":0,"write":0}},"cost":0.01}');
        "#,
        )
        .unwrap();
        f
    }

    #[test]
    fn open_reads_projects() {
        let f = fixture_db();
        let r = DbReader::open(f.path()).unwrap();
        let ps = r.list_projects().unwrap();
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].id, "proj1");
    }

    #[test]
    fn open_reads_sessions() {
        let f = fixture_db();
        let r = DbReader::open(f.path()).unwrap();
        let ss = r.list_sessions(Some("proj1")).unwrap();
        assert_eq!(ss.len(), 1);
        assert_eq!(ss[0].id, "ses_1");
    }

    #[test]
    fn load_session_attaches_parts() {
        let f = fixture_db();
        let r = DbReader::open(f.path()).unwrap();
        let s = r.load_session("ses_1").unwrap();
        assert_eq!(s.messages.len(), 2);
        assert_eq!(s.messages[0].parts.len(), 1);
        assert_eq!(s.messages[1].parts.len(), 3);
        assert_eq!(s.first_user_text().as_deref(), Some("hello"));
    }

    #[test]
    fn load_session_missing_errors() {
        let f = fixture_db();
        let r = DbReader::open(f.path()).unwrap();
        let err = r.load_session("nope").unwrap_err();
        assert!(matches!(err, ConvoError::SessionNotFound(_)));
    }

    #[test]
    fn malformed_message_rolls_over_to_other() {
        let f = NamedTempFile::new().unwrap();
        let conn = Connection::open(f.path()).unwrap();
        conn.execute_batch(
            "CREATE TABLE message (id text PRIMARY KEY, session_id text NOT NULL,
                time_created integer NOT NULL, time_updated integer NOT NULL,
                data text NOT NULL);
             CREATE TABLE session (id text PRIMARY KEY, project_id text NOT NULL, slug text NOT NULL,
                directory text NOT NULL, title text NOT NULL, version text NOT NULL,
                time_created integer NOT NULL, time_updated integer NOT NULL,
                parent_id text, share_url text, summary_additions integer,
                summary_deletions integer, summary_files text, time_compacting integer,
                time_archived integer, workspace_id text);
             CREATE TABLE part (id text PRIMARY KEY, message_id text NOT NULL,
                session_id text NOT NULL, time_created integer NOT NULL,
                time_updated integer NOT NULL, data text NOT NULL);
             INSERT INTO session (id, project_id, slug, directory, title, version, time_created, time_updated)
               VALUES ('s','p','slug','/p','t','1.0.0',1,1);
             INSERT INTO message (id, session_id, time_created, time_updated, data)
               VALUES ('m','s',1,1,'{{not json}}');",
        )
        .unwrap();
        let r = DbReader::open(f.path()).unwrap();
        let msgs = r.list_messages_raw("s").unwrap();
        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0].data, MessageData::Other));
    }
}

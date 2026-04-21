//! Filesystem layout for opencode state.
//!
//! opencode stores everything under `$XDG_DATA_HOME/opencode/`
//! (`~/.local/share/opencode/` on macOS/Linux). The primary
//! conversation store is the `opencode.db` SQLite database; per-step
//! filesystem snapshots live in sibling bare git repositories under
//! `snapshot/<project-id>/<sha1(worktree)>/`.
//!
//! `project.id` is itself the SHA of the repo's first root commit
//! (`git rev-list --max-parents=0 HEAD`), so a project survives
//! being moved on disk. The inner snapshot dirname is the SHA-1 of
//! the absolute worktree path, which means snapshots are keyed by
//! exact path — moving the worktree orphans old snapshots even
//! though the session IDs still resolve.

use crate::error::{ConvoError, Result};
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};

const SNAPSHOT_SUBDIR: &str = "snapshot";
const DB_FILE: &str = "opencode.db";
const LOG_SUBDIR: &str = "log";

/// Builder-style resolver over the opencode data directory.
#[derive(Debug, Clone)]
pub struct PathResolver {
    home_dir: Option<PathBuf>,
    data_dir: Option<PathBuf>,
}

impl Default for PathResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl PathResolver {
    pub fn new() -> Self {
        Self {
            home_dir: home_dir(),
            data_dir: None,
        }
    }

    pub fn with_home<P: Into<PathBuf>>(mut self, home: P) -> Self {
        self.home_dir = Some(home.into());
        self
    }

    /// Override the data directory directly (defaults to
    /// `$XDG_DATA_HOME/opencode` or `~/.local/share/opencode`).
    pub fn with_data_dir<P: Into<PathBuf>>(mut self, data_dir: P) -> Self {
        self.data_dir = Some(data_dir.into());
        self
    }

    pub fn home_dir(&self) -> Result<&Path> {
        self.home_dir.as_deref().ok_or(ConvoError::NoHomeDirectory)
    }

    pub fn data_dir(&self) -> Result<PathBuf> {
        if let Some(d) = &self.data_dir {
            return Ok(d.clone());
        }
        // XDG_DATA_HOME fallback logic mirroring the `xdg-basedir` crate.
        if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
            let p = PathBuf::from(xdg).join("opencode");
            if !p.as_os_str().is_empty() {
                return Ok(p);
            }
        }
        Ok(self.home_dir()?.join(".local/share/opencode"))
    }

    pub fn db_path(&self) -> Result<PathBuf> {
        Ok(self.data_dir()?.join(DB_FILE))
    }

    pub fn snapshot_root(&self) -> Result<PathBuf> {
        Ok(self.data_dir()?.join(SNAPSHOT_SUBDIR))
    }

    pub fn log_dir(&self) -> Result<PathBuf> {
        Ok(self.data_dir()?.join(LOG_SUBDIR))
    }

    /// The bare git repository that backs snapshots for a given
    /// `(project_id, worktree)` pair.
    ///
    /// opencode has used two layouts over its lifetime:
    /// - Current: `snapshot/<project-id>/<sha1(worktree)>/` — one
    ///   gitdir per `(project, worktree)` pair so forked worktrees
    ///   get isolated snapshot stores.
    /// - Older: `snapshot/<project-id>/` — a single gitdir per
    ///   project regardless of worktree.
    ///
    /// Returns the first candidate that exists. If neither exists,
    /// returns the current-layout path (so the caller's subsequent
    /// `git2::Repository::open` will produce a clean NotFound error).
    pub fn snapshot_gitdir(&self, project_id: &str, worktree: &Path) -> Result<PathBuf> {
        let root = self.snapshot_root()?;
        let worktree_hash = sha1_hex(worktree.to_string_lossy().as_bytes());
        let nested = root.join(project_id).join(&worktree_hash);
        if nested.exists() {
            return Ok(nested);
        }
        let flat = root.join(project_id);
        if flat.exists() && flat.join("config").exists() {
            return Ok(flat);
        }
        Ok(nested)
    }

    pub fn exists(&self) -> bool {
        self.data_dir().map(|p| p.exists()).unwrap_or(false)
    }

    pub fn db_exists(&self) -> bool {
        self.db_path().map(|p| p.exists()).unwrap_or(false)
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

pub(crate) fn sha1_hex(bytes: &[u8]) -> String {
    let mut h = Sha1::new();
    h.update(bytes);
    let digest = h.finalize();
    let mut out = String::with_capacity(40);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(out, "{:02x}", b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> (TempDir, PathResolver) {
        let temp = TempDir::new().unwrap();
        let data = temp.path().join(".local/share/opencode");
        fs::create_dir_all(&data).unwrap();
        let resolver = PathResolver::new()
            .with_home(temp.path())
            .with_data_dir(&data);
        (temp, resolver)
    }

    #[test]
    fn data_dir_defaults_to_home_when_no_xdg() {
        let temp = TempDir::new().unwrap();
        // SAFETY: tests that mutate env vars serialize via the lock in
        // src/reader.rs tests; the direct test below doesn't mutate.
        let r = PathResolver::new().with_home(temp.path());
        let d = r.data_dir().unwrap();
        assert!(d.ends_with(".local/share/opencode"), "got {:?}", d);
    }

    #[test]
    fn db_path_under_data_dir() {
        let (_t, r) = setup();
        assert!(r.db_path().unwrap().ends_with("opencode/opencode.db"));
    }

    #[test]
    fn snapshot_gitdir_uses_sha1_of_worktree() {
        let (_t, r) = setup();
        let pid = "4e82d608d080e9d92be51e24b592302df6a8cbf8";
        let wt = Path::new("/Users/ben/empathic/oss/toolpath");
        let gd = r.snapshot_gitdir(pid, wt).unwrap();
        // sha1("/Users/ben/empathic/oss/toolpath") = bb93f39a…
        assert!(gd.to_string_lossy().contains(pid));
        assert!(
            gd.to_string_lossy()
                .contains("bb93f39a69862ba18e7893cc96424f83876a9687")
        );
    }

    #[test]
    fn sha1_of_known_string() {
        assert_eq!(
            sha1_hex(b"/Users/ben/empathic/oss/toolpath"),
            "bb93f39a69862ba18e7893cc96424f83876a9687"
        );
    }

    #[test]
    fn exists_reflects_data_dir() {
        let (_t, r) = setup();
        assert!(r.exists());
        let missing = PathResolver::new().with_data_dir("/never/exists");
        assert!(!missing.exists());
    }
}

//! Two-tier cache of pre-derived traces keyed by (provider, project, session).
//!
//! The tray poller kicks off a background warm-up pass after every 30-second
//! poll: for each recent session on a supported provider it derives the trace
//! ahead of time and stores the result here. When the user clicks a row in the
//! popover the IPC command pulls from the cache instead of re-running derive,
//! so the main window opens essentially instantly.
//!
//! Tier 1 (memory) — a small `HashMap<CacheKey, CacheEntry>` capped at
//! [`MAX_MEMORY_ENTRIES`], hot path for repeat lookups within a session.
//!
//! Tier 2 (disk) — optional. When `TraceCache::with_disk_dir(...)` is used,
//! entries are also written to `<dir>/<hash>.json` so they survive app
//! restarts. The disk directory lives under the OS temp dir, which macOS /
//! Linux periodically clean up anyway. On memory miss we fall back to disk and
//! promote the hit into memory.
//!
//! Freshness is keyed on the source session's `last_activity` timestamp — when
//! a session gets new turns its cached entry is treated as stale and re-derived
//! on the next poll. The synchronous click path is willing to serve a mildly
//! stale entry (at worst ~30s behind the next poll); correctness is guaranteed
//! by the next warm-up pass replacing it.
//!
//! The cache is shared via `app.manage(Arc<TraceCache>)`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Upper bound on in-memory entries. The popover shows at most 20 recent
/// sessions so this gives one poll's worth of headroom without letting memory
/// grow unbounded as the user's activity churns over days.
const MAX_MEMORY_ENTRIES: usize = 32;

/// Default cap on on-disk entries. Larger than the memory cap because disk
/// survives across restarts and we want yesterday's sessions still warm.
pub const DEFAULT_MAX_DISK_ENTRIES: usize = 200;

/// Identifies a single trace. The `project` field is empty for providers that
/// don't key on project (codex/opencode) — currently unused for prewarm since
/// only claude and pi derive from the desktop backend.
#[derive(Debug, Clone, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct CacheKey {
    pub provider: String,
    pub project: String,
    pub session_id: String,
}

/// Cached derive output plus the `last_activity` it was derived against so the
/// warmer can detect staleness without re-running derive.
///
/// Only the derived document is cached — source-label / filename strings are
/// cheap to reconstruct at each call site and have slightly different formats
/// for the popover-open vs. main-window-"Select" paths, so they don't belong
/// in shared state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub doc: Value,
    /// The session's `last_activity` at derive time (RFC3339). A newer
    /// timestamp in the next poll means the cached entry is stale. Empty when
    /// the writer doesn't know it (e.g. a user-initiated derive from the main
    /// window) — in that case the next warmer pass will replace it.
    pub last_activity: String,
}

/// Shared, thread-safe trace cache.
#[derive(Debug, Default)]
pub struct TraceCache {
    entries: Mutex<HashMap<CacheKey, CacheEntry>>,
    /// Keys whose warm-up derive is in progress. Prevents the warmer from
    /// racing itself when two polls land before the first derive finishes.
    in_flight: Mutex<HashSet<CacheKey>>,
    /// Root for tier-2 disk caching. `None` disables disk persistence.
    disk_dir: Option<PathBuf>,
}

impl TraceCache {
    /// Memory-only cache. Useful for tests that don't want disk side effects.
    #[cfg(test)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Memory + disk cache. Best-effort — failing to create the directory just
    /// downgrades to memory-only without returning an error (startup
    /// shouldn't fail because /tmp is weird).
    pub fn with_disk_dir(dir: PathBuf) -> Self {
        let dir = match fs::create_dir_all(&dir) {
            Ok(_) => Some(dir),
            Err(_) => None,
        };
        Self {
            disk_dir: dir,
            ..Self::default()
        }
    }

    /// Fetch a cached entry, if any. Checks memory first, then disk; on a disk
    /// hit, promotes the entry into memory so subsequent lookups skip the
    /// filesystem. Returns a clone so the caller doesn't hold the lock across
    /// IPC serialization.
    pub fn get(&self, key: &CacheKey) -> Option<CacheEntry> {
        if let Some(hit) = self.memory_get(key) {
            return Some(hit);
        }
        let entry = self.disk_load(key)?;
        self.memory_insert(key.clone(), entry.clone());
        Some(entry)
    }

    /// Store (or overwrite) an entry in both memory and disk (if enabled).
    pub fn insert(&self, key: CacheKey, entry: CacheEntry) {
        self.memory_insert(key.clone(), entry.clone());
        self.disk_save(&key, &entry);
    }

    /// Returns true if the cache has a fresh entry for this key — i.e. the
    /// stored `last_activity` matches. Used by the warmer to skip keys that
    /// are already up to date.
    ///
    /// Consults disk when memory misses so a freshly-restarted app doesn't
    /// re-derive everything already on disk.
    pub fn is_fresh(&self, key: &CacheKey, last_activity: &str) -> bool {
        self.get(key)
            .map(|e| e.last_activity == last_activity)
            .unwrap_or(false)
    }

    /// Attempt to claim a derive slot for this key. Returns true if the caller
    /// now owns the slot and should proceed; false if another warmer already
    /// has it in flight.
    pub fn try_claim(&self, key: &CacheKey) -> bool {
        let Ok(mut s) = self.in_flight.lock() else {
            return false;
        };
        s.insert(key.clone())
    }

    /// Release a slot claimed via `try_claim`. Call this in every exit path
    /// (success or failure) so the next poll can retry on error.
    pub fn release(&self, key: &CacheKey) {
        if let Ok(mut s) = self.in_flight.lock() {
            s.remove(key);
        }
    }

    /// Number of in-flight warm-ups. Used to cap concurrent derive threads.
    pub fn in_flight_count(&self) -> usize {
        self.in_flight.lock().map(|s| s.len()).unwrap_or(0)
    }

    /// Delete the oldest-mtime cache files when the on-disk count exceeds
    /// `max_entries`. Intended to run once at startup.
    pub fn prune_disk(&self, max_entries: usize) {
        let Some(dir) = &self.disk_dir else {
            return;
        };
        let Ok(read) = fs::read_dir(dir) else {
            return;
        };
        let mut files: Vec<(PathBuf, std::time::SystemTime)> = read
            .filter_map(|e| e.ok())
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("json") {
                    return None;
                }
                let mtime = entry.metadata().ok()?.modified().ok()?;
                Some((path, mtime))
            })
            .collect();
        if files.len() <= max_entries {
            return;
        }
        files.sort_by(|a, b| a.1.cmp(&b.1));
        for (path, _) in files.iter().take(files.len() - max_entries) {
            let _ = fs::remove_file(path);
        }
    }

    // --- internal helpers -------------------------------------------------

    fn memory_get(&self, key: &CacheKey) -> Option<CacheEntry> {
        self.entries
            .lock()
            .ok()
            .and_then(|m| m.get(key).cloned())
    }

    fn memory_insert(&self, key: CacheKey, entry: CacheEntry) {
        let Ok(mut m) = self.entries.lock() else {
            return;
        };
        if m.len() >= MAX_MEMORY_ENTRIES
            && !m.contains_key(&key)
            && let Some(oldest) = m
                .iter()
                .min_by(|a, b| a.1.last_activity.cmp(&b.1.last_activity))
                .map(|(k, _)| k.clone())
        {
            m.remove(&oldest);
        }
        m.insert(key, entry);
    }

    fn disk_path(&self, key: &CacheKey) -> Option<PathBuf> {
        let dir = self.disk_dir.as_ref()?;
        let raw = format!("{}|{}|{}", key.provider, key.project, key.session_id);
        Some(dir.join(format!("{:016x}.json", fnv1a_64(&raw))))
    }

    fn disk_load(&self, key: &CacheKey) -> Option<CacheEntry> {
        let path = self.disk_path(key)?;
        let bytes = fs::read(&path).ok()?;
        match serde_json::from_slice::<CacheEntry>(&bytes) {
            Ok(entry) => Some(entry),
            Err(_) => {
                // Corrupt or schema-incompatible — drop it so the next derive
                // replaces it instead of replaying the error forever.
                let _ = fs::remove_file(&path);
                None
            }
        }
    }

    fn disk_save(&self, key: &CacheKey, entry: &CacheEntry) {
        let Some(path) = self.disk_path(key) else {
            return;
        };
        if write_atomic(&path, entry).is_err() {
            // Best-effort — disk cache is a latency optimisation, not a
            // correctness requirement. Clean up any half-written temp file.
            if let Some(tmp) = tmp_path(&path) {
                let _ = fs::remove_file(tmp);
            }
        }
    }
}

/// FNV-1a 64-bit. Stable across Rust versions (unlike `DefaultHasher`) so
/// disk entries cached by one build remain valid for the next.
fn fnv1a_64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn tmp_path(path: &Path) -> Option<PathBuf> {
    let mut name = path.file_name()?.to_os_string();
    name.push(".tmp");
    Some(path.with_file_name(name))
}

fn write_atomic(path: &Path, entry: &CacheEntry) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(entry)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let tmp = tmp_path(path).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "cache path has no file name")
    })?;
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn key(provider: &str, project: &str, id: &str) -> CacheKey {
        CacheKey {
            provider: provider.into(),
            project: project.into(),
            session_id: id.into(),
        }
    }

    fn entry(activity: &str) -> CacheEntry {
        CacheEntry {
            doc: Value::Null,
            last_activity: activity.into(),
        }
    }

    #[test]
    fn get_returns_none_when_missing() {
        let cache = TraceCache::new();
        assert!(cache.get(&key("claude", "p", "s")).is_none());
    }

    #[test]
    fn insert_then_get_roundtrip() {
        let cache = TraceCache::new();
        let k = key("claude", "/proj", "sess-1");
        cache.insert(k.clone(), entry("2026-04-23T10:00:00Z"));
        let got = cache.get(&k).expect("entry present");
        assert_eq!(got.last_activity, "2026-04-23T10:00:00Z");
    }

    #[test]
    fn is_fresh_matches_activity_timestamp() {
        let cache = TraceCache::new();
        let k = key("claude", "/proj", "sess-1");
        cache.insert(k.clone(), entry("2026-04-23T10:00:00Z"));
        assert!(cache.is_fresh(&k, "2026-04-23T10:00:00Z"));
        assert!(!cache.is_fresh(&k, "2026-04-23T10:05:00Z"));
        assert!(!cache.is_fresh(&key("claude", "/proj", "missing"), "2026-04-23T10:00:00Z"));
    }

    #[test]
    fn try_claim_is_exclusive_until_release() {
        let cache = TraceCache::new();
        let k = key("claude", "/proj", "sess-1");
        assert!(cache.try_claim(&k));
        assert!(!cache.try_claim(&k));
        assert_eq!(cache.in_flight_count(), 1);
        cache.release(&k);
        assert_eq!(cache.in_flight_count(), 0);
        assert!(cache.try_claim(&k));
    }

    #[test]
    fn insert_evicts_oldest_when_full() {
        let cache = TraceCache::new();
        for i in 0..MAX_MEMORY_ENTRIES {
            cache.insert(
                key("claude", "/p", &format!("s{i:04}")),
                // Lexicographic order matches numeric order here.
                entry(&format!("2026-04-23T10:{i:02}:00Z")),
            );
        }
        let hot = key("claude", "/p", "new");
        cache.insert(hot.clone(), entry("2026-04-23T11:00:00Z"));

        // Oldest ("s0000") should be gone; newly inserted should be present.
        assert!(cache.memory_get(&key("claude", "/p", "s0000")).is_none());
        assert!(cache.memory_get(&hot).is_some());
    }

    #[test]
    fn insert_overwrite_does_not_evict() {
        let cache = TraceCache::new();
        let k = key("claude", "/p", "sess");
        cache.insert(k.clone(), entry("2026-04-23T10:00:00Z"));
        cache.insert(k.clone(), entry("2026-04-23T10:05:00Z"));
        assert_eq!(cache.get(&k).unwrap().last_activity, "2026-04-23T10:05:00Z");
    }

    #[test]
    fn disk_persists_across_instances() {
        let dir = TempDir::new().unwrap();
        let k = key("claude", "/proj", "sess-1");
        {
            let a = TraceCache::with_disk_dir(dir.path().to_path_buf());
            a.insert(k.clone(), entry("2026-04-23T10:00:00Z"));
        }
        // Fresh instance — nothing in memory, must hit disk.
        let b = TraceCache::with_disk_dir(dir.path().to_path_buf());
        assert!(b.memory_get(&k).is_none());
        let hit = b.get(&k).expect("disk hit");
        assert_eq!(hit.last_activity, "2026-04-23T10:00:00Z");
        // And the hit should have been promoted into memory.
        assert!(b.memory_get(&k).is_some());
    }

    #[test]
    fn disk_is_fresh_matches_across_restart() {
        let dir = TempDir::new().unwrap();
        let k = key("pi", "/proj", "abc");
        {
            let a = TraceCache::with_disk_dir(dir.path().to_path_buf());
            a.insert(k.clone(), entry("2026-04-23T10:00:00Z"));
        }
        let b = TraceCache::with_disk_dir(dir.path().to_path_buf());
        assert!(b.is_fresh(&k, "2026-04-23T10:00:00Z"));
        assert!(!b.is_fresh(&k, "2026-04-23T10:05:00Z"));
    }

    #[test]
    fn disk_load_discards_corrupt_file() {
        let dir = TempDir::new().unwrap();
        let cache = TraceCache::with_disk_dir(dir.path().to_path_buf());
        let k = key("claude", "/proj", "sess");
        let path = cache.disk_path(&k).expect("disk enabled");
        fs::write(&path, b"{ not valid json").unwrap();
        assert!(cache.disk_load(&k).is_none());
        assert!(!path.exists(), "corrupt file should be removed");
    }

    #[test]
    fn prune_disk_evicts_oldest() {
        let dir = TempDir::new().unwrap();
        let cache = TraceCache::with_disk_dir(dir.path().to_path_buf());
        for i in 0..5 {
            cache.insert(
                key("claude", "/p", &format!("s{i}")),
                entry(&format!("2026-04-23T10:0{i}:00Z")),
            );
        }
        cache.prune_disk(2);
        let remaining = fs::read_dir(dir.path())
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .path()
                    .extension()
                    .and_then(|s| s.to_str())
                    == Some("json")
            })
            .count();
        assert_eq!(remaining, 2);
    }

    #[test]
    fn with_disk_dir_falls_back_to_memory_only_when_dir_unusable() {
        // Pointing at a non-directory path — create a file and hand its path.
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("not-a-dir");
        fs::write(&file_path, b"oops").unwrap();
        // `create_dir_all` on an existing file returns an error; cache should
        // fall back to memory-only without panicking.
        let cache = TraceCache::with_disk_dir(file_path.clone());
        let k = key("claude", "/p", "s");
        cache.insert(k.clone(), entry("2026-04-23T10:00:00Z"));
        assert!(cache.get(&k).is_some());
    }
}

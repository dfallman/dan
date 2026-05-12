//! Project-file indexing and recent-files persistence.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::SystemTime;
use serde::{Serialize, Deserialize};

/// Walk upward from `start` looking for a `.git` entry (dir or file).
/// Returns the directory containing it. Falls back to `start` if nothing found.
#[allow(dead_code)]
pub fn detect_project_root(start: &Path) -> PathBuf {
    let mut cur = start.to_path_buf();
    loop {
        if cur.join(".git").exists() {
            return cur;
        }
        match cur.parent() {
            Some(p) => cur = p.to_path_buf(),
            None => return start.to_path_buf(),
        }
    }
}

/// Extensions to skip in the index walk (binary noise).
const BINARY_EXT_BLOCKLIST: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "ico",
    "pdf", "zip", "tar", "gz", "bz2", "xz", "7z",
    "so", "dylib", "dll", "o", "a", "exe", "class", "jar",
    "ttf", "otf", "woff", "woff2",
    "mp3", "mp4", "mov", "avi", "mkv", "webm",
];

fn is_blocked_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| BINARY_EXT_BLOCKLIST.iter().any(|&b| b.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

/// Spawn a background thread that walks `root` (gitignore-aware) and pushes
/// every regular file to `tx`. Thread is detached; senders are dropped on
/// completion. Caller drains `rx` in `poll_async_tasks`.
#[allow(dead_code)]
pub fn spawn_index_walker(root: PathBuf, tx: mpsc::Sender<PathBuf>) {
    std::thread::spawn(move || {
        use ignore::WalkBuilder;
        let walker = WalkBuilder::new(&root)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .hidden(true)
            .parents(true)
            .build();
        for entry in walker.flatten() {
            let p = entry.path();
            if !p.is_file() { continue; }
            if is_blocked_extension(p) { continue; }
            if tx.send(p.to_path_buf()).is_err() { return; }
        }
    });
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentFile {
    pub path: PathBuf,
    pub last_opened_unix: u64,
}

#[allow(dead_code)]
impl RecentFile {
    pub fn last_opened(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(self.last_opened_unix)
    }
}

/// Path to the persisted recent-files JSON.
#[allow(dead_code)]
pub fn recent_files_path() -> Option<PathBuf> {
    dirs::data_dir()
        .map(|d| d.join("dan").join("recent.json"))
        .or_else(|| dirs::home_dir().map(|h| h.join(".dan").join("recent.json")))
}

/// Read the recent-files list. Corrupt or missing → empty Vec.
/// Stale entries (file no longer exists) are filtered.
#[allow(dead_code)]
pub fn load_recent_files() -> Vec<RecentFile> {
    let Some(path) = recent_files_path() else { return Vec::new() };
    let Ok(content) = std::fs::read_to_string(&path) else { return Vec::new() };
    let mut list: Vec<RecentFile> = serde_json::from_str(&content).unwrap_or_default();
    list.retain(|r| r.path.exists());
    list
}

/// Atomically write the recent-files list. Best-effort; errors swallowed.
#[allow(dead_code)]
pub fn save_recent_files(list: &[RecentFile]) {
    let Some(path) = recent_files_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(content) = serde_json::to_string_pretty(list) else { return };
    let _ = crate::atomic_io::write(&path, content.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_project_root_finds_git_dir() {
        let temp = tempdir_for_test();
        std::fs::create_dir_all(temp.join("a/b/c")).unwrap();
        std::fs::create_dir_all(temp.join(".git")).unwrap();
        let found = detect_project_root(&temp.join("a/b/c"));
        assert_eq!(found, temp);
    }

    #[test]
    fn detect_project_root_falls_back_to_start_when_no_git() {
        let temp = tempdir_for_test();
        let inner = temp.join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        let found = detect_project_root(&inner);
        // Should walk up to root then fall back to start.
        assert_eq!(found, inner);
    }

    #[test]
    fn blocked_extension_check() {
        assert!(is_blocked_extension(Path::new("foo.png")));
        assert!(is_blocked_extension(Path::new("a/b/foo.PNG")));
        assert!(!is_blocked_extension(Path::new("foo.rs")));
        assert!(!is_blocked_extension(Path::new("noext")));
    }

    #[test]
    fn recent_files_round_trip() {
        let entries = vec![
            RecentFile { path: PathBuf::from("/tmp/a"), last_opened_unix: 100 },
            RecentFile { path: PathBuf::from("/tmp/b"), last_opened_unix: 200 },
        ];
        let json = serde_json::to_string(&entries).unwrap();
        let back: Vec<RecentFile> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].path, PathBuf::from("/tmp/a"));
    }

    fn tempdir_for_test() -> PathBuf {
        let mut d = std::env::temp_dir();
        d.push(format!("dan_palette_test_{}", std::process::id()));
        // Ensure clean slate.
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}

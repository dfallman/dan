//! Crash-time buffer rescue (P0-2).
//!
//! The panic hook runs on the panicking thread and cannot borrow the editor,
//! so the editor publishes O(1) snapshots of its dirty buffers into this global
//! registry after handling input (`TextRope::clone` is a structural-sharing
//! clone, so this is cheap). When a panic fires, the hook flushes those
//! snapshots to their swap paths before the process dies — turning a panic from
//! "lose everything since the last 5 s autosave" into "lose ~nothing".

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::buffer::rope::TextRope;

/// One dirty buffer's rescue snapshot: where to write, and what to write.
pub struct CrashEntry {
	pub swap_path: PathBuf,
	pub text: TextRope,
}

static REGISTRY: OnceLock<Mutex<Vec<CrashEntry>>> = OnceLock::new();

fn registry() -> &'static Mutex<Vec<CrashEntry>> {
	REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

/// Replace the registry with snapshots of the currently-dirty buffers.
pub fn publish(entries: Vec<CrashEntry>) {
	if let Ok(mut g) = registry().lock() {
		*g = entries;
	}
}

/// Flush every registered buffer to its swap path. Returns the paths written.
///
/// Uses `try_lock`: if a panic fires while the editor happens to hold the
/// registry lock (a vanishingly small window), we skip the dump rather than
/// deadlock the dying process.
pub fn dump() -> Vec<PathBuf> {
	let mut written = Vec::new();
	if let Ok(g) = registry().try_lock() {
		for entry in g.iter() {
			let content = entry.text.to_string_full();
			crate::recovery::write_swap_atomic(&entry.swap_path, &content);
			written.push(entry.swap_path.clone());
		}
	}
	written
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn dump_writes_registered_buffers_to_their_swap_paths() {
		// P0-2: a published dirty buffer must be flushed to disk by dump().
		let mut swap = std::env::temp_dir();
		swap.push(format!("dan_crash_test_{}.swp", std::process::id()));
		let _ = std::fs::remove_file(&swap);

		publish(vec![CrashEntry {
			swap_path: swap.clone(),
			text: TextRope::from_str("unsaved work"),
		}]);
		let written = dump();

		assert!(written.contains(&swap), "dump must report the written path");
		assert_eq!(
			std::fs::read_to_string(&swap).unwrap(),
			"unsaved work",
			"dump must persist the buffer content"
		);
		std::fs::remove_file(&swap).ok();
	}
}

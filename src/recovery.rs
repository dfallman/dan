use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Path of the `.swp` crash-recovery file for `original_path`. Prefers a
/// hidden sibling (`.foo.txt.swp`); falls back to a name-flattened entry in
/// `$TMPDIR/dan_swaps/` when the original directory isn't writable.
pub fn get_swap_path(original_path: &Path) -> PathBuf {
	if let Some(file_name) = original_path.file_name() {
		let mut swp_name = std::ffi::OsString::from(".");
		swp_name.push(file_name);
		swp_name.push(".swp");

		let local_swp = original_path.with_file_name(&swp_name);

		if let Some(parent) = local_swp.parent() {
			if fs::metadata(parent).is_ok()
				&& !parent
					.metadata()
					.map(|m| m.permissions().readonly())
					.unwrap_or(true)
			{
				return local_swp;
			}
		}

		// Fall back to $TMPDIR/dan_swaps/.
		let mut temp_dir = env::temp_dir();
		temp_dir.push("dan_swaps");
		let _ = fs::create_dir_all(&temp_dir);

		let flat_name = original_path
			.to_string_lossy()
			.replace(['/', '\\'], "_");
		temp_dir.push(format!("{}.swp", flat_name));
		return temp_dir;
	}

	PathBuf::from(".dan.swp")
}

/// Write `content` to `swap_path` via temp + rename. Best-effort: errors
/// are silently dropped because swap-file failures must never interrupt the
/// user's editing session.
pub fn write_swap_atomic(swap_path: &Path, content: &str) {
	let mut tmp_path = swap_path.to_path_buf();
	let tmp_ext = tmp_path
		.extension()
		.unwrap_or_default()
		.to_string_lossy()
		.into_owned()
		+ ".tmp";
	tmp_path.set_extension(tmp_ext);

	if fs::write(&tmp_path, content).is_ok() {
		let _ = fs::rename(tmp_path, swap_path);
	}
}

/// Remove the swap file. Called after a successful save.
pub fn cleanup_swap(swap_path: &Path) {
	if swap_path.exists() {
		let _ = fs::remove_file(swap_path);
	}
}

/// Returns the swap-file content if a recovery candidate exists, else None.
pub fn check_recovery(swap_path: &Path) -> Option<String> {
	if swap_path.exists() {
		fs::read_to_string(swap_path).ok()
	} else {
		None
	}
}

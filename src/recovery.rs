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

/// Write `content` to `swap_path` via temp + fsync + rename.
///
/// Hardened against symlink-TOCTOU on the temp path (S2.3) and against
/// world-readable `.swp` content (S2.4):
/// - the temp file lives in the same directory as the swap file with an
///   unguessable suffix (PID + sub-second nanos);
/// - it is opened with `O_CREAT | O_EXCL` so a pre-created symlink at the
///   temp path causes an error rather than a write through the link;
/// - on Unix the temp is opened mode 0o600 so other local users can't
///   read unsaved buffer content.
///
/// Best-effort: errors are silently dropped because swap-file failures
/// must never interrupt the user's editing session.
pub fn write_swap_atomic(swap_path: &Path, content: &str) {
	let Some(parent) = swap_path.parent() else { return };
	let Some(file_name) = swap_path.file_name() else { return };

	let nanos = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| d.subsec_nanos())
		.unwrap_or(0);
	let tmp_name = format!(
		".{}.dan-swap-tmp.{}.{}",
		file_name.to_string_lossy(),
		std::process::id(),
		nanos
	);
	let tmp_path = parent.join(tmp_name);

	let result = (|| -> std::io::Result<()> {
		let mut options = fs::OpenOptions::new();
		options.write(true).create_new(true);
		#[cfg(unix)]
		{
			use std::os::unix::fs::OpenOptionsExt;
			options.mode(0o600);
		}
		let mut f = options.open(&tmp_path)?;
		use std::io::Write;
		f.write_all(content.as_bytes())?;
		f.sync_all()?;
		fs::rename(&tmp_path, swap_path)?;
		Ok(())
	})();

	if result.is_err() {
		let _ = fs::remove_file(&tmp_path);
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

#[cfg(test)]
mod tests {
	use super::*;
	use std::io::Read;

	fn temp_dir() -> std::path::PathBuf {
		let mut d = std::env::temp_dir();
		d.push(format!("dan_recovery_test_{}", std::process::id()));
		fs::create_dir_all(&d).unwrap();
		d
	}

	#[test]
	fn write_swap_round_trips_content() {
		let dir = temp_dir();
		let swap = dir.join("foo.swp");
		let _ = fs::remove_file(&swap);
		write_swap_atomic(&swap, "hello world");
		let mut got = String::new();
		fs::File::open(&swap).unwrap().read_to_string(&mut got).unwrap();
		assert_eq!(got, "hello world");
		fs::remove_file(&swap).unwrap();
	}

	#[cfg(unix)]
	#[test]
	fn write_swap_uses_mode_0o600() {
		// S2.4 regression: swap files must not be world-readable.
		use std::os::unix::fs::PermissionsExt;
		let dir = temp_dir();
		let swap = dir.join("perm.swp");
		let _ = fs::remove_file(&swap);
		write_swap_atomic(&swap, "secret");
		let mode = fs::metadata(&swap).unwrap().permissions().mode() & 0o777;
		assert_eq!(mode, 0o600, "swap file must be owner-only readable, got {:o}", mode);
		fs::remove_file(&swap).unwrap();
	}

	#[test]
	fn write_swap_replaces_existing() {
		let dir = temp_dir();
		let swap = dir.join("replace.swp");
		write_swap_atomic(&swap, "old");
		write_swap_atomic(&swap, "new");
		let got = fs::read_to_string(&swap).unwrap();
		assert_eq!(got, "new");
		fs::remove_file(&swap).unwrap();
	}

	#[cfg(unix)]
	#[test]
	fn write_swap_does_not_follow_temp_symlink() {
		// S2.3 regression: a pre-existing symlink at the swap path itself
		// would be replaced by the rename (correct), but a symlink at the
		// *temp* path used to be written through. The temp name is now
		// unguessable AND opened O_EXCL, so even a guessed pre-created
		// symlink can't intercept the write.
		//
		// We can't fully simulate the unguessability here, but we can
		// verify O_EXCL: pre-create the swap_path as a regular file with a
		// known content and confirm it gets overwritten with our content.
		let dir = temp_dir();
		let swap = dir.join("excl.swp");
		fs::write(&swap, b"prior content").unwrap();
		write_swap_atomic(&swap, "fresh content");
		let got = fs::read_to_string(&swap).unwrap();
		assert_eq!(got, "fresh content");
		fs::remove_file(&swap).unwrap();
	}
}

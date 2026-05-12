use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Write `bytes` to `path` atomically: temp-file + fsync + rename.
///
/// Goals:
/// - **Atomicity (D4.1).** A crash, disk-full, or kill mid-write leaves the
///   target file in its prior state; the partial write only ever lives in
///   the sibling temp file.
/// - **Symlink fidelity (D4.3).** If `path` is a symlink, the rename targets
///   the link's resolved destination so the symlink itself is preserved
///   instead of being replaced by a regular file.
/// - **Permission preservation (D4.2).** The temp file's mode is set to
///   match the original target before rename.
///
/// Not yet covered: uid/gid preservation, xattrs, `O_NOFOLLOW`-style
/// symlink-TOCTOU hardening for the temp file. See AUDIT.md.
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
	let target = resolve_symlink_target(path)?;

	let parent = target.parent().ok_or_else(|| {
		io::Error::new(io::ErrorKind::InvalidInput, "save target has no parent directory")
	})?;
	let file_name = target.file_name().ok_or_else(|| {
		io::Error::new(io::ErrorKind::InvalidInput, "save target has no file name")
	})?;

	let mut tmp_path = parent.to_path_buf();
	tmp_path.push(format!(
		".{}.dan-tmp.{}",
		file_name.to_string_lossy(),
		std::process::id()
	));

	let original_perms = fs::metadata(&target).ok().map(|m| m.permissions());

	let write_result = (|| -> io::Result<()> {
		let mut options = fs::OpenOptions::new();
		options.write(true).create_new(true);
		#[cfg(unix)]
		{
			use std::os::unix::fs::OpenOptionsExt;
			options.mode(0o600);
		}
		let mut f = options.open(&tmp_path)?;
		f.write_all(bytes)?;
		f.sync_all()?;
		Ok(())
	})();

	if let Err(e) = write_result {
		let _ = fs::remove_file(&tmp_path);
		return Err(e);
	}

	if let Some(perms) = original_perms {
		let _ = fs::set_permissions(&tmp_path, perms);
	}

	if let Err(e) = fs::rename(&tmp_path, &target) {
		let _ = fs::remove_file(&tmp_path);
		return Err(e);
	}

	#[cfg(unix)]
	{
		if let Ok(dir) = fs::File::open(parent) {
			let _ = dir.sync_all();
		}
	}

	Ok(())
}

/// If `path` is a symlink, return its resolved destination (one hop is
/// enough — `fs::rename` operates by name, so we only need to know whether
/// the final component is a symlink and where it points). Relative link
/// targets are resolved against the symlink's parent.
fn resolve_symlink_target(path: &Path) -> io::Result<PathBuf> {
	match fs::symlink_metadata(path) {
		Ok(md) if md.file_type().is_symlink() => {
			let link = fs::read_link(path)?;
			if link.is_absolute() {
				Ok(link)
			} else {
				let base = path.parent().unwrap_or_else(|| Path::new("."));
				Ok(base.join(link))
			}
		}
		_ => Ok(path.to_path_buf()),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::io::Read;

	fn temp_dir() -> PathBuf {
		let mut d = std::env::temp_dir();
		d.push(format!("dan_atomic_io_test_{}", std::process::id()));
		fs::create_dir_all(&d).unwrap();
		d
	}

	#[test]
	fn writes_new_file() {
		let dir = temp_dir();
		let path = dir.join("new.txt");
		let _ = fs::remove_file(&path);
		write(&path, b"hello").unwrap();
		let mut got = String::new();
		fs::File::open(&path).unwrap().read_to_string(&mut got).unwrap();
		assert_eq!(got, "hello");
		fs::remove_file(&path).unwrap();
	}

	#[test]
	fn replaces_existing_file_atomically() {
		let dir = temp_dir();
		let path = dir.join("replace.txt");
		fs::write(&path, b"old contents").unwrap();
		write(&path, b"new contents").unwrap();
		let mut got = String::new();
		fs::File::open(&path).unwrap().read_to_string(&mut got).unwrap();
		assert_eq!(got, "new contents");
		fs::remove_file(&path).unwrap();
	}

	#[cfg(unix)]
	#[test]
	fn preserves_original_mode() {
		use std::os::unix::fs::PermissionsExt;
		let dir = temp_dir();
		let path = dir.join("mode.txt");
		fs::write(&path, b"x").unwrap();
		fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
		write(&path, b"y").unwrap();
		let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
		assert_eq!(mode, 0o640);
		fs::remove_file(&path).unwrap();
	}

	#[cfg(unix)]
	#[test]
	fn writes_through_symlink_preserves_link() {
		use std::os::unix::fs::symlink;
		let dir = temp_dir();
		let target = dir.join("real.txt");
		let link = dir.join("link.txt");
		fs::write(&target, b"original").unwrap();
		let _ = fs::remove_file(&link);
		symlink(&target, &link).unwrap();

		write(&link, b"updated").unwrap();

		// Link must still be a symlink.
		let md = fs::symlink_metadata(&link).unwrap();
		assert!(md.file_type().is_symlink(), "symlink was replaced by a file");

		// Target must contain the new content.
		let got = fs::read_to_string(&target).unwrap();
		assert_eq!(got, "updated");

		fs::remove_file(&link).unwrap();
		fs::remove_file(&target).unwrap();
	}
}

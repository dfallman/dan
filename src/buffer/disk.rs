//! A buffer's file on disk: cheap change stamps and the one decoding path
//! shared by open and reload.

use std::io;
use std::path::Path;
use std::time::SystemTime;

use super::is_too_large;

/// Cheap fingerprint of a file's on-disk state, compared on every poll.
/// The file is only read when this differs from the last-known stamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskStamp {
	pub mtime: Option<SystemTime>,
	pub len: u64,
	/// Changes when a writer replaces the file (temp file + rename).
	pub inode: Option<u64>,
}

impl DiskStamp {
	pub fn from_metadata(meta: &std::fs::Metadata) -> Self {
		#[cfg(unix)]
		let inode = {
			use std::os::unix::fs::MetadataExt;
			Some(meta.ino())
		};
		#[cfg(not(unix))]
		let inode = None;
		Self {
			mtime: meta.modified().ok(),
			len: meta.len(),
			inode,
		}
	}

	pub fn of(path: &Path) -> io::Result<Self> {
		Ok(Self::from_metadata(&std::fs::metadata(path)?))
	}
}

/// Read and decode a file: refuse directories, oversized and binary files;
/// honour a BOM; else UTF-8, else the chardetng guess.
pub fn read_decoded(path: &Path) -> io::Result<(String, &'static encoding_rs::Encoding)> {
	if path.is_dir() {
		return Err(io::Error::new(
			io::ErrorKind::IsADirectory,
			"Is a directory",
		));
	}

	// Refuse pathologically large files before reading them into memory
	// (P3-H): a multi-GB read can OOM-abort and take every buffer with it.
	let meta = std::fs::metadata(path)?;
	if is_too_large(meta.len()) {
		return Err(io::Error::new(
			io::ErrorKind::InvalidData,
			format!(
				"File too large to open ({} bytes; limit {} bytes)",
				meta.len(),
				super::MAX_FILE_BYTES
			),
		));
	}

	let bytes = std::fs::read(path)?;

	// A UTF-16/UTF-32 byte-order mark up front means the file legitimately
	// contains NUL bytes (every ASCII char), so it must NOT be rejected by
	// the NUL binary heuristic below (P4-N). `decode` strips the BOM.
	let bom_encoding = encoding_rs::Encoding::for_bom(&bytes)
		.map(|(enc, _)| enc)
		.filter(|&enc| enc != encoding_rs::UTF_8);

	let (content, encoding) = if let Some(enc) = bom_encoding {
		let (decoded, _, _) = enc.decode(&bytes);
		(decoded.into_owned(), enc)
	} else {
		// Treat any file containing a NUL byte as binary; refuse to open.
		if bytes.contains(&0) {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"File appears to be binary",
			));
		}

		if let Ok(s) = std::str::from_utf8(&bytes) {
			(s.to_string(), encoding_rs::UTF_8)
		} else {
			let mut detector = chardetng::EncodingDetector::new();
			detector.feed(&bytes, true);
			let enc = detector.guess(None, true);
			let (dec, _, _) = enc.decode(&bytes);
			(dec.into_owned(), enc)
		}
	};
	Ok((content, encoding))
}

/// A disk change that overlaps unsaved edits, awaiting keep-mine /
/// take-theirs. Until answered, the merge base keeps the conflict sections
/// at their old text (`Merge::resolved_base`), so a newer disk version
/// re-merges correctly.
#[derive(Debug, Clone)]
pub struct PendingConflict {
	/// `Buffer::version` the regions refer to.
	pub version: u64,
	/// Conflicting regions in the buffer's current char coordinates.
	pub regions: Vec<super::merge::Change>,
	/// The new disk text; becomes the merge base once answered.
	pub theirs: super::rope::TextRope,
}

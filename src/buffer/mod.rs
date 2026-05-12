pub mod history;
pub mod rope;

use std::io;
use std::path::{Path, PathBuf};

use self::history::History;
use self::rope::TextRope;

use crate::sanitize::sanitize_paste;

/// A text buffer representing a file or scratch document.
pub struct Buffer {
	/// The text content.
	pub text: TextRope,
	/// Edit history for undo/redo.
	pub history: History,
	/// File path (None for scratch buffers).
	pub file_path: Option<PathBuf>,
	/// Whether the buffer has unsaved changes.
	pub dirty: bool,
	/// Monotonic counter incremented on every mutation. Lets async tasks
	/// (e.g. the formatter) detect that the buffer changed underneath them
	/// while they were running.
	pub version: u64,
	/// The detected byte stream character encoding of the document.
	pub encoding: &'static encoding_rs::Encoding,
	/// Path of the `.swp` crash-recovery file for this buffer, if any.
	pub swp_path: Option<PathBuf>,
}

impl Buffer {
	/// Create an empty buffer.
	pub fn new() -> Self {
		Self {
			text: TextRope::new(),
			history: History::new(),
			file_path: None,
			dirty: false,
			version: 0,
			encoding: encoding_rs::UTF_8,
			swp_path: None,
		}
	}

	/// Create a buffer from a file, returning the Buffer and its sniffed indentation metrics.
	pub fn from_file(path: &Path) -> io::Result<(Self, Option<bool>, Option<usize>)> {
		if path.is_dir() {
			return Err(io::Error::new(
				io::ErrorKind::IsADirectory,
				"Is a directory",
			));
		}

		let bytes = std::fs::read(path)?;

		// Treat any file containing a NUL byte as binary; refuse to open.
		if bytes.contains(&0) {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"File appears to be binary",
			));
		}

		let (content, encoding) = if let Ok(s) = std::str::from_utf8(&bytes) {
			(s.to_string(), encoding_rs::UTF_8)
		} else {
			let mut detector = chardetng::EncodingDetector::new();
			detector.feed(&bytes, true);
			let enc = detector.guess(None, true);
			let (dec, _, _) = enc.decode(&bytes);
			(dec.into_owned(), enc)
		};

		// --- Smart Indentation Detection ---
		let mut tabs_count = 0;
		let mut spaces_count = 0;
		let mut space_indents = std::collections::HashMap::new();

		for line in content.lines().take(1000) {
			if line.starts_with('\t') {
				tabs_count += 1;
			} else if line.starts_with(' ') {
				let leading_spaces = line.chars().take_while(|&c| c == ' ').count();
				if leading_spaces > 0 {
					spaces_count += 1;
					*space_indents.entry(leading_spaces).or_insert(0) += 1;
				}
			}
		}

		let mut expand_tab = None;
		let mut tab_width = None;

		if tabs_count > spaces_count {
			expand_tab = Some(false);
		} else if spaces_count > tabs_count {
			expand_tab = Some(true);
			// Find majority vote among valid structural sizes
			let mut best_size = None;
			let mut max_votes = 0;
			for step in [2, 3, 4, 8] {
				let votes = *space_indents.get(&step).unwrap_or(&0);
				if votes > max_votes {
					max_votes = votes;
					best_size = Some(step);
				}
			}
			if let Some(w) = best_size {
				tab_width = Some(w);
			}
		}

		let buffer = Self {
			text: TextRope::from_str(&content),
			history: History::new(),
			file_path: Some(path.to_path_buf()),
			dirty: false,
			version: 0,
			encoding,
			swp_path: None,
		};

		Ok((buffer, expand_tab, tab_width))
	}

	/// Materialise the buffer text and apply on-save transforms
	/// (trim trailing whitespace, line-ending conversion).
	pub fn prepare_save_text(&self, config: &crate::config::Config) -> String {
		let mut text = self.text.to_string_full();

		if config.trim_trailing_whitespace.unwrap_or(false) {
			let mut processed = String::with_capacity(text.len());
			for mut line in text.split_inclusive('\n') {
				let has_nl = line.ends_with('\n');
				let has_cr = line.ends_with("\r\n");
				if has_nl {
					line = if has_cr {
						&line[..line.len() - 2]
					} else {
						&line[..line.len() - 1]
					};
				}
				processed.push_str(line.trim_end_matches([' ', '\t']));
				if has_cr {
					processed.push_str("\r\n");
				} else if has_nl {
					processed.push('\n');
				}
			}
			text = processed;
		}

		if let Some(ref eol) = config.end_of_line {
			let is_crlf = eol.to_lowercase() == "crlf";
			text = text.replace("\r\n", "\n");
			if is_crlf {
				text = text.replace('\n', "\r\n");
			}
		}

		text
	}

	/// Encode `text` using this buffer's stored encoding. Refuses if the
	/// encoding cannot losslessly represent every character — silently
	/// substituting `?` (encoding_rs's default) would corrupt the user's
	/// file (D4.5).
	fn encode_for_save(&self, text: &str) -> io::Result<Vec<u8>> {
		let (encoded_bytes, _, had_unmappable) = self.encoding.encode(text);
		if had_unmappable {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				format!(
					"Save aborted: file contains characters that cannot be represented in {}. \
					 Use Save As to write a UTF-8 copy.",
					self.encoding.name()
				),
			));
		}
		Ok(encoded_bytes.into_owned())
	}

	/// Save the buffer to its current file path. Uses temp-file + rename so
	/// a partial write (disk-full, crash, kill mid-write) cannot corrupt the
	/// on-disk file.
	pub fn save(&mut self, config: &crate::config::Config) -> io::Result<()> {
		if let Some(ref path) = self.file_path {
			let text = self.prepare_save_text(config);
			let encoded_bytes = self.encode_for_save(&text)?;
			crate::atomic_io::write(path, &encoded_bytes)?;

			if let Some(ref swp) = self.swp_path {
				crate::recovery::cleanup_swap(swp);
			}

			self.dirty = false;
			Ok(())
		} else {
			Err(io::Error::other(
				"No file path set for this buffer",
			))
		}
	}

	/// Save the buffer to a specific path (Save As). Uses the same atomic
	/// temp+rename strategy as `save`.
	pub fn save_to(&mut self, path: &Path, config: &crate::config::Config) -> io::Result<()> {
		let text = self.prepare_save_text(config);
		let encoded_bytes = self.encode_for_save(&text)?;
		crate::atomic_io::write(path, &encoded_bytes)?;
		self.file_path = Some(path.to_path_buf());

		if let Some(ref swp) = self.swp_path {
			crate::recovery::cleanup_swap(swp);
		}

		self.dirty = false;
		Ok(())
	}

	/// Number of lines in the buffer.
	pub fn line_count(&self) -> usize {
		self.text.len_lines()
	}

	/// Get the display name for this buffer.
	pub fn display_name(&self) -> String {
		self.file_path
			.as_ref()
			.and_then(|p| p.file_name())
			.map(|n| n.to_string_lossy().to_string())
			.unwrap_or_else(|| "[Scratch]".to_string())
	}

	/// Get the full path representation for this buffer.
	pub fn full_path_display(&self) -> String {
		self.file_path
			.as_ref()
			.map(|p| p.display().to_string())
			.unwrap_or_else(|| "[Scratch]".to_string())
	}

	// -- Edit operations with history tracking --

	/// Insert a character at a char position.
	pub fn insert_char(&mut self, pos: usize, ch: char) {
		self.history.start_group(&self.text);
		self.text.insert_char(pos, ch);
		self.mark_mutated();
	}

	/// Insert a string at a char position. The bytes are stored verbatim;
	/// terminal-injection sanitization is a render-layer concern (see
	/// `crate::sanitize` and `ScreenBuffer::put_char`). Callers handling
	/// untrusted external content (paste, drop) must use `insert_paste` instead.
	pub fn insert_str(&mut self, pos: usize, s: &str) {
		self.history.start_group(&self.text);
		self.text.insert_str(pos, s);
		self.mark_mutated();
	}

	/// Insert externally-sourced text at a char position, sanitizing
	/// terminal-injection vectors before storage. Use this only at paste/drop
	/// entry points; never for buffer-internal text movement. Returns the
	/// number of chars actually inserted (after sanitization).
	pub fn insert_paste(&mut self, pos: usize, s: &str) -> usize {
		let clean = sanitize_paste(s);
		let char_count = clean.chars().count();
		self.history.start_group(&self.text);
		self.text.insert_str(pos, &clean);
		self.mark_mutated();
		char_count
	}

	/// Delete a single character at a char position.
	pub fn delete_char(&mut self, pos: usize) {
		if pos < self.text.len_chars() {
			self.history.start_group(&self.text);
			self.text.remove(pos..pos + 1);
			self.mark_mutated();
		}
	}

	/// Delete a range of characters.
	pub fn delete_range(&mut self, start: usize, end: usize) {
		if start < end && end <= self.text.len_chars() {
			self.history.start_group(&self.text);
			self.text.remove(start..end);
			self.mark_mutated();
		}
	}

	/// Commit pending edits as an undo group.
	pub fn commit_edits(&mut self) {
		self.history.commit();
	}

	/// Undo the last edit group.
	pub fn undo(&mut self) {
		if let Some(restored) = self.history.undo(self.text.clone()) {
			self.text = restored;
			self.mark_mutated();
		}
	}

	/// Redo the last undone edit group.
	pub fn redo(&mut self) {
		if let Some(restored) = self.history.redo(self.text.clone()) {
			self.text = restored;
			self.mark_mutated();
		}
	}

	/// Mark a content mutation: bumps the dirty flag and the version
	/// counter. Centralised so async tasks can detect "buffer changed
	/// underneath me" via `Buffer::version`. Crate-public for call sites
	/// that mutate `self.text` directly (e.g. line-swap in `editing.rs`).
	#[inline]
	pub(crate) fn mark_mutated(&mut self) {
		self.dirty = true;
		self.version = self.version.wrapping_add(1);
	}
}

impl Default for Buffer {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn insert_str_preserves_c0_bytes() {
		// D4.8 regression: storage layer must not silently sanitize.
		// `insert_str` is the literal-insert API and must round-trip every byte.
		let mut b = Buffer::new();
		b.insert_str(0, "before\x01after");
		assert_eq!(b.text.to_string_full(), "before\x01after");
	}

	#[test]
	fn insert_str_preserves_esc_byte() {
		let mut b = Buffer::new();
		b.insert_str(0, "x\x1by");
		assert_eq!(b.text.to_string_full(), "x\x1by");
	}

	#[test]
	fn insert_paste_sanitizes_esc() {
		// `insert_paste` is the only API that mutates external content.
		let mut b = Buffer::new();
		let n = b.insert_paste(0, "x\x1by");
		assert_eq!(n, 3);
		// ESC becomes '^' per sanitize.rs; the contract is "no ESC in storage".
		assert!(!b.text.to_string_full().contains('\x1b'));
	}

	#[test]
	fn version_increments_on_each_mutation() {
		// R3.3 regression: the formatter race detector relies on every
		// mutation bumping `Buffer::version`.
		let mut b = Buffer::new();
		assert_eq!(b.version, 0);

		b.insert_str(0, "hello");
		assert_eq!(b.version, 1);

		b.insert_char(5, '!');
		assert_eq!(b.version, 2);

		b.insert_paste(0, "x");
		assert_eq!(b.version, 3);

		b.delete_char(0);
		assert_eq!(b.version, 4);

		b.delete_range(0, 1);
		assert_eq!(b.version, 5);
	}

	#[test]
	fn save_refuses_when_encoding_cannot_represent_content() {
		// D4.5 regression: encoding_rs replaces unmappable chars with '?'
		// silently. We must refuse the save instead.
		let mut b = Buffer::new();
		b.encoding = encoding_rs::WINDOWS_1252;
		let mut tmp = std::env::temp_dir();
		tmp.push(format!("dan_d45_test_{}.txt", std::process::id()));
		b.file_path = Some(tmp.clone());

		// '🌍' is not representable in Windows-1252.
		b.insert_str(0, "hello 🌍");

		let cfg = crate::config::Config::default();
		let result = b.save(&cfg);
		assert!(result.is_err(), "save should refuse unmappable chars");
		assert!(!tmp.exists(), "no file should be written when save refuses");
	}

	#[test]
	fn save_succeeds_when_encoding_can_represent_content() {
		let mut b = Buffer::new();
		b.encoding = encoding_rs::WINDOWS_1252;
		let mut tmp = std::env::temp_dir();
		tmp.push(format!("dan_d45_ok_{}.txt", std::process::id()));
		b.file_path = Some(tmp.clone());
		b.insert_str(0, "hello world");

		let cfg = crate::config::Config::default();
		assert!(b.save(&cfg).is_ok());
		let written = std::fs::read(&tmp).unwrap();
		assert_eq!(written, b"hello world");
		std::fs::remove_file(&tmp).unwrap();
	}

	#[test]
	fn version_unchanged_by_non_mutating_calls() {
		let mut b = Buffer::new();
		b.insert_str(0, "abc");
		let v = b.version;

		// Reading text should not bump version.
		let _ = b.text.to_string_full();
		let _ = b.line_count();
		let _ = b.display_name();
		assert_eq!(b.version, v);

		// Out-of-range deletes are silent no-ops; version must not bump.
		b.delete_char(999);
		b.delete_range(50, 60);
		assert_eq!(b.version, v);
	}
}

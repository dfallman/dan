pub mod history;
pub mod rope;
pub mod config_loader;

use std::io;
use std::path::{Path, PathBuf};

use self::history::History;
use self::rope::TextRope;

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
	/// The detected byte stream character encoding of the document.
	pub encoding: &'static encoding_rs::Encoding,
	/// File-local override for using spaces instead of tabs.
	pub expand_tab: Option<bool>,
	/// File-local override for tab spacing width.
	pub tab_width: Option<usize>,
	/// Formats all excess spaces terminating lines globally during save commits.
	pub trim_on_save: Option<bool>,
	/// Line termination style requested statically (LF / CRLF).
	pub newline_style: Option<String>,
	/// Dynamic `.swp` crash-recovery tracking pipeline securely checking OS permissions.
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
			encoding: encoding_rs::UTF_8,
			expand_tab: None,
			tab_width: None,
			trim_on_save: None,
			newline_style: None,
			swp_path: None,
		}
	}

	/// Create a buffer from a file.
	pub fn from_file(path: &Path) -> io::Result<Self> {
		if path.is_dir() {
			return Err(io::Error::new(
				io::ErrorKind::IsADirectory,
				"Is a directory",
			));
		}

		let bytes = std::fs::read(path)?;
		
		// If the file explicitly contains null bytes globally, it is functionally a binary file.
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

		let mut buffer = Self {
			text: TextRope::from_str(&content),
			history: History::new(),
			file_path: Some(path.to_path_buf()),
			dirty: false,
			encoding,
			expand_tab,
			tab_width,
			trim_on_save: None,
			newline_style: None,
			swp_path: None,
		};

		config_loader::load_project_settings(path, &mut buffer);

		Ok(buffer)
	}

	/// Prepares the output buffer recursively enforcing `.editorconfig` bindings.
	pub fn prepare_save_text(&self) -> String {
		let mut text = self.text.to_string_full();
		
		if self.trim_on_save.unwrap_or(false) {
			let mut processed = String::with_capacity(text.len());
			for mut line in text.split_inclusive('\n') {
				let has_nl = line.ends_with('\n');
				let has_cr = line.ends_with("\r\n");
				if has_nl {
					line = if has_cr { &line[..line.len()-2] } else { &line[..line.len()-1] };
				}
				processed.push_str(line.trim_end_matches(|c| c == ' ' || c == '\t'));
				if has_cr { processed.push_str("\r\n"); } else if has_nl { processed.push('\n'); }
			}
			text = processed;
		}

		if let Some(ref eol) = self.newline_style {
			let is_crlf = eol.to_lowercase() == "crlf";
			text = text.replace("\r\n", "\n");
			if is_crlf {
				text = text.replace('\n', "\r\n");
			}
		}

		text
	}

	/// Save the buffer to its file.
	pub fn save(&mut self) -> io::Result<()> {
		if let Some(ref path) = self.file_path {
			let text = self.prepare_save_text();
			let (encoded_bytes, _, _) = self.encoding.encode(&text);
			std::fs::write(path, encoded_bytes.as_ref())?;
			
			if let Some(ref swp) = self.swp_path {
				crate::recovery::cleanup_swap(swp);
			}
			
			self.dirty = false;
			Ok(())
		} else {
			Err(io::Error::new(
				io::ErrorKind::Other,
				"No file path set for this buffer",
			))
		}
	}

	/// Save the buffer to a new path and adopt it as the buffer's file.
	pub fn save_to(&mut self, path: &Path) -> io::Result<()> {
		let text = self.prepare_save_text();
		let (encoded_bytes, _, _) = self.encoding.encode(&text);
		std::fs::write(path, encoded_bytes.as_ref())?;
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
			.unwrap_or_else(|| "[scratch]".to_string())
	}

	// -- Edit operations with history tracking --

	/// Insert a character at a char position.
	pub fn insert_char(&mut self, pos: usize, ch: char) {
		self.history.start_group(&self.text);
		self.text.insert_char(pos, ch);
		self.dirty = true;
	}

	/// Insert a string at a char position.
	pub fn insert_str(&mut self, pos: usize, s: &str) {
		self.history.start_group(&self.text);
		self.text.insert_str(pos, s);
		self.dirty = true;
	}

	/// Delete a single character at a char position.
	pub fn delete_char(&mut self, pos: usize) {
		if pos < self.text.len_chars() {
			self.history.start_group(&self.text);
			self.text.remove(pos..pos + 1);
			self.dirty = true;
		}
	}

	/// Delete a range of characters.
	pub fn delete_range(&mut self, start: usize, end: usize) {
		if start < end && end <= self.text.len_chars() {
			self.history.start_group(&self.text);
			self.text.remove(start..end);
			self.dirty = true;
		}
	}

	/// Commit pending edits as an undo group.
	pub fn commit_edits(&mut self) {
		self.history.commit();
	}

	/// Undo the last edit group natively locking variables securely copying bounds O(1).
	pub fn undo(&mut self) {
		if let Some(restored) = self.history.undo(self.text.clone()) {
			self.text = restored;
			self.dirty = true;
		}
	}

	/// Redo the last undone edit group wrapping snapshot bounds natively securely.
	pub fn redo(&mut self) {
		if let Some(restored) = self.history.redo(self.text.clone()) {
			self.text = restored;
			self.dirty = true;
		}
	}
}

impl Default for Buffer {
	fn default() -> Self {
		Self::new()
	}
}

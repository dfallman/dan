//! Prompt modes: go-to-line, save-as, overwrite confirmation.
use crate::editor::mode::Mode;
use crate::editor::Editor;

impl Editor {
	pub(crate) fn cmd_go_to_line_open(&mut self) {
		self.clear_selection();
		self.goto_line_input.clear();
		self.prompt_cursor = 0;
		self.mode = Mode::GoToLine;
	}

	pub(crate) fn cmd_go_to_line_insert_char(&mut self, ch: char) {
		if ch.is_ascii_digit() {
			prompt_insert_char(&mut self.goto_line_input, self.prompt_cursor, ch);
			self.prompt_cursor += 1;
		}
	}

	pub(crate) fn cmd_go_to_line_delete_char(&mut self) {
		if self.prompt_cursor > 0 {
			self.prompt_cursor -= 1;
			prompt_remove_char(&mut self.goto_line_input, self.prompt_cursor);
		}
	}

	pub(crate) fn cmd_go_to_line_confirm(&mut self) {
		if let Ok(n) = self.goto_line_input.parse::<usize>() {
			let target = if n == 0 { 0 } else { n - 1 }; // 1-indexed to 0-indexed
			let max_line = self.buffer().line_count().saturating_sub(1);
			let line = target.min(max_line);
			self.buffer_mut().cursors.set_cursor(line, 0);
			self.set_status(format!("Jumped to line {}", line + 1));
		}
		self.goto_line_input.clear();
		self.prompt_cursor = 0;
		self.mode = Mode::Editing;
	}

	pub(crate) fn cmd_go_to_line_cancel(&mut self) {
		self.goto_line_input.clear();
		self.prompt_cursor = 0;
		self.mode = Mode::Editing;
		self.clear_status();
	}

	pub(crate) fn cmd_save_as_open(&mut self) {
		// Pre-populate with current file path if one exists.
		self.save_as_input = self
			.buffer()
			.file_path
			.as_ref()
			.map(|p| p.to_string_lossy().to_string())
			.unwrap_or_default();
		self.prompt_cursor = self.save_as_input.chars().count();
		self.mode = Mode::SaveAs;
	}

	pub(crate) fn cmd_save_as_insert_char(&mut self, ch: char) {
		prompt_insert_char(&mut self.save_as_input, self.prompt_cursor, ch);
		self.prompt_cursor += 1;
	}

	pub(crate) fn cmd_save_as_delete_char(&mut self) {
		if self.prompt_cursor > 0 {
			self.prompt_cursor -= 1;
			prompt_remove_char(&mut self.save_as_input, self.prompt_cursor);
		}
	}

	pub(crate) fn cmd_prompt_cursor_left(&mut self) {
		if self.prompt_cursor > 0 {
			self.prompt_cursor -= 1;
		}
	}

	pub(crate) fn cmd_prompt_cursor_right(&mut self) {
		let max_len = match self.mode {
			Mode::Searching => self.search_query.chars().count(),
			Mode::ReplacingWith => self.replace_with.chars().count(),
			Mode::GoToLine => self.goto_line_input.chars().count(),
			Mode::SaveAs | Mode::ConfirmOverwrite => self.save_as_input.chars().count(),
			_ => 0,
		};
		if self.prompt_cursor < max_len {
			self.prompt_cursor += 1;
		}
	}

	pub(crate) fn cmd_save_as_confirm(&mut self) {
		let path_str = self.save_as_input.clone();
		if path_str.is_empty() {
			self.save_as_input.clear();
			self.prompt_cursor = 0;
			self.mode = Mode::Editing;
			self.set_status("Save as cancelled: no path given");
		} else {
			let path = std::path::Path::new(&path_str);
			// Check if parent directory exists.
			if let Some(parent) = path.parent() {
				if !parent.as_os_str().is_empty() && !parent.exists() {
					self.set_status(format!(
						"Directory does not exist: {}",
						parent.display()
					));
					return;
				}
			}
			// Check if file already exists — ask for overwrite confirmation.
			if path.exists() {
				self.save_as_pending_path = Some(path_str);
				self.mode = Mode::ConfirmOverwrite;
			} else {
				// New file — save directly.
				self.save_as_input.clear();
				self.prompt_cursor = 0;
				self.buffer_mut().commit_edits();
				let cfg = self.config.clone();
				match self.buffer_mut().save_to(path, &cfg) {
					Ok(()) => {
						self.set_status(format!("✓ Saved as {}", path.display()));
						if self.quit_cycle_idx.is_some() {
							self.advance_quit_cycle();
						} else {
							self.mode = Mode::Editing;
						}
					}
					Err(e) => {
						// Drop any in-flight quit cycle: the save failed,
						// so we are not advancing through dirty buffers.
						self.quit_cycle_idx = None;
						self.mode = Mode::Editing;
						self.set_status(format!("Save failed: {}", e));
					}
				}
			}
		}
	}

	pub(crate) fn cmd_save_as_cancel(&mut self) {
		self.save_as_input.clear();
		self.prompt_cursor = 0;
		if self.quit_cycle_idx.is_some() {
			self.quit_cycle_idx = None;
		}
		self.mode = Mode::Editing;
		self.clear_status();
	}

	pub(crate) fn cmd_confirm_overwrite(&mut self) {
		if let Some(path_str) = self.save_as_pending_path.take() {
			let path = std::path::Path::new(&path_str);
			self.save_as_input.clear();
			self.prompt_cursor = 0;
			self.buffer_mut().commit_edits();
			let cfg = self.config.clone();
			match self.buffer_mut().save_to(path, &cfg) {
				Ok(()) => {
					self.set_status(format!("✓ Saved as {}", path_str));
					if self.quit_cycle_idx.is_some() {
						self.advance_quit_cycle();
					} else {
						self.mode = Mode::Editing;
					}
				}
				Err(e) => {
					// Same as SaveAsConfirm error path — clear the cycle.
					self.quit_cycle_idx = None;
					self.mode = Mode::Editing;
					self.set_status(format!("Save failed: {}", e));
				}
			}
		}
	}

	pub(crate) fn cmd_cancel_overwrite(&mut self) {
		self.save_as_pending_path = None;
		self.mode = Mode::SaveAs;
	}
}

/// Insert `ch` into `s` at **char** index `char_idx` (0..=char_count). The
/// editor's `prompt_cursor` is a char position; `String::insert` takes a byte
/// index, and using the char index directly panics on a non-boundary
/// (`is_char_boundary`) the moment any multibyte char precedes the cursor
/// (P1-B / P1-C). This translates char→byte first.
pub(super) fn prompt_insert_char(s: &mut String, char_idx: usize, ch: char) {
	let byte_idx = s
		.char_indices()
		.nth(char_idx)
		.map(|(b, _)| b)
		.unwrap_or(s.len());
	s.insert(byte_idx, ch);
}

/// Remove the char at **char** index `char_idx` from `s`. No-op if out of
/// range. Char-index counterpart to `String::remove` (which takes bytes).
pub(super) fn prompt_remove_char(s: &mut String, char_idx: usize) {
	if let Some((b, _)) = s.char_indices().nth(char_idx) {
		s.remove(b);
	}
}


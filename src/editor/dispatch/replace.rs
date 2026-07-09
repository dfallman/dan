//! Find-and-replace command handlers.
use crate::editor::dispatch::prompts::{prompt_insert_char, prompt_remove_char};
use crate::editor::mode::Mode;
use crate::editor::Editor;

impl Editor {
	pub(crate) fn cmd_replace_insert_char(&mut self, ch: char) {
		if self.mode == Mode::ReplacingWith {
			prompt_insert_char(&mut self.replace_with, self.prompt_cursor, ch);
			self.prompt_cursor += 1;
		}
	}

	pub(crate) fn cmd_replace_delete_char(&mut self) {
		if self.mode == Mode::ReplacingWith
			&& self.prompt_cursor > 0 {
				self.prompt_cursor -= 1;
				prompt_remove_char(&mut self.replace_with, self.prompt_cursor);
			}
	}

	pub(crate) fn cmd_replace_with_confirm(&mut self) {
		if self.buffer().search_matches.is_empty() {
			self.mode = Mode::Editing;
			self.search_query.clear();
			self.clear_status();
		} else {
			self.mode = Mode::ReplacingStep;
			self.jump_to_search_match();
		}
	}

	pub(crate) fn cmd_replace_action_yes(&mut self) {
		let current = {
			let buf = self.buffer();
			buf.search_matches.get(buf.search_match_idx).copied()
		};
		if let Some((start, end)) = current {
			let replacement = self.replace_with.clone();
			self.buffer_mut().commit_edits(); // wrap
			self.buffer_mut().delete_range(start, end);
			self.buffer_mut().insert_str(start, &replacement);
			self.buffer_mut().commit_edits();

			// Char count, not byte length: `start` is a char offset.
			let new_pos = start + replacement.chars().count();
			let line = self.buffer().text.char_to_line(new_pos);
			let col = new_pos - self.buffer().text.line_to_char(line);
			self.buffer_mut().search_saved_cursor = Some((line, col));
			self.buffer_mut().cursors.set_cursor(line, col);
			self.refresh_search_matches();

			if self.buffer().search_matches.is_empty() {
				self.mode = Mode::Editing;
				self.search_query.clear();
				self.clear_status();
			} else {
				// match idx is implicitly resync'd via refresh geometry bounding to the nearest next item naturally
				self.jump_to_search_match();
			}
		} else {
			self.mode = Mode::Editing;
		}
	}

	pub(crate) fn cmd_replace_action_no(&mut self) {
		if !self.buffer().search_matches.is_empty() {
			let len = self.buffer().search_matches.len();
			self.buffer_mut().search_match_idx = (self.buffer().search_match_idx + 1) % len;
			self.jump_to_search_match();
		} else {
			self.mode = Mode::Editing;
		}
	}

	pub(crate) fn cmd_replace_action_all(&mut self) {
		self.buffer_mut().commit_edits(); // Explicit history block grouping
		let replacement = self.replace_with.clone();

		// Iterate end-to-start so each replace leaves earlier match
		// offsets intact. Start from search_match_idx so already-skipped
		// (`n`-answered) matches stay untouched.
		let pending_matches = {
			let buf = self.buffer();
			buf.search_matches[buf.search_match_idx..].to_vec()
		};
		for &(start, end) in pending_matches.iter().rev() {
			self.buffer_mut().delete_range(start, end);
			self.buffer_mut().insert_str(start, &replacement);
		}

		self.buffer_mut().commit_edits();
		self.clamp_cursors();
		self.mode = Mode::Editing;
		self.search_query.clear();
		self.buffer_mut().search_matches.clear();
		self.buffer_mut().search_match_idx = 0;
		self.clear_status();
	}

	pub(crate) fn cmd_replace_cancel(&mut self) {
		if !self.replace_query.is_empty() {
			let q = self.replace_query.clone();
			self.buffer_mut().last_search_query = q;
		}
		if let Some((line, col)) = self.buffer_mut().search_saved_cursor.take() {
			self.buffer_mut().cursors.set_cursor(line, col);
		}
		self.search_query.clear();
		self.replace_query.clear();
		self.replace_with.clear();
		self.buffer_mut().search_matches.clear();
		self.buffer_mut().search_match_idx = 0;
		self.mode = Mode::Editing;
		self.clear_status();
	}
}

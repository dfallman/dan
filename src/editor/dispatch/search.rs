//! Search command handlers.
use crate::editor::mode::Mode;
use crate::editor::Editor;

impl Editor {
	pub(crate) fn cmd_search_forward(&mut self) {
		// Enter search mode — save the current cursor position.
		self.clear_selection();
		let c = self.buffer().cursors.cursor();
		self.buffer_mut().search_saved_cursor = Some((c.line, c.col));
		// Pre-fill with the last search query so re-opening search
		// immediately shows previous results.
		self.search_query = self.buffer().last_search_query.clone();
		self.buffer_mut().search_matches.clear();
		self.buffer_mut().search_match_idx = 0;
		self.mode = Mode::Searching;
		self.prompt_cursor = self.search_query.chars().count();
		self.prompt_view_start.set(0);
		// If we have a previous query, run the search immediately.
		if !self.search_query.is_empty() {
			self.refresh_search_matches();
		}
	}

	pub(crate) fn cmd_search_insert_char(&mut self, ch: char) {
		// Insert by char index (not byte index) so multibyte input works.
		let mut chars: Vec<char> = self.search_query.chars().collect();
		chars.insert(self.prompt_cursor, ch);
		self.search_query = chars.into_iter().collect();
		self.prompt_cursor += 1;

		self.refresh_search_matches();
	}

	pub(crate) fn cmd_search_delete_char(&mut self) {
		if self.prompt_cursor > 0 {
			self.prompt_cursor -= 1;
			let mut chars: Vec<char> = self.search_query.chars().collect();
			chars.remove(self.prompt_cursor);
			self.search_query = chars.into_iter().collect();
			self.refresh_search_matches();
		}
	}

	pub(crate) fn cmd_search_confirm(&mut self) {
		// Accept the current match — exit search, select matched text.
		if !self.search_query.is_empty() {
			let q = self.search_query.clone();
			self.buffer_mut().last_search_query = q;
		}
		let current = {
			let buf = self.buffer();
			buf.search_matches.get(buf.search_match_idx).copied()
		};
		if let Some((start, end)) = current {
			// Set anchor at the end of the match, head at the start
			// so the matched text is selected.
			use crate::editor::cursor::Cursor;
			let line = self.buffer().text.char_to_line(start);
			let col = start - self.buffer().text.line_to_char(line);
			let end_line = self.buffer().text.char_to_line(end);
			let end_col = end - self.buffer().text.line_to_char(end_line);
			self.buffer_mut().cursors.primary_mut().anchor = Cursor::new(end_line, end_col);
			self.buffer_mut().cursors.primary_mut().head = Cursor::new(line, col);
		}
		self.mode = Mode::Editing;
		self.search_query.clear();
		self.clear_search_regex_state();
		self.buffer_mut().search_matches.clear();
		self.buffer_mut().search_match_idx = 0;
		self.buffer_mut().search_saved_cursor = None;
		self.clear_status();
	}

	pub(crate) fn cmd_search_cancel(&mut self) {
		// Restore cursor to its pre-search position.
		if !self.search_query.is_empty() {
			let q = self.search_query.clone();
			self.buffer_mut().last_search_query = q;
		}
		if let Some((line, col)) = self.buffer_mut().search_saved_cursor.take() {
			self.buffer_mut().cursors.set_cursor(line, col);
		}
		self.search_query.clear();
		self.clear_search_regex_state();
		self.buffer_mut().search_matches.clear();
		self.buffer_mut().search_match_idx = 0;
		self.mode = Mode::Editing;
		self.clear_status();
	}

	pub(crate) fn cmd_search_convert_to_replace(&mut self) {
		if !self.buffer().search_matches.is_empty() {
			self.replace_query = self.search_query.clone();
			let q = self.search_query.clone();
			self.buffer_mut().last_search_query = q;
			self.mode = Mode::ReplacingWith;
			self.prompt_cursor = self.replace_with.chars().count();
			self.prompt_view_start.set(0);
		}
	}

	pub(crate) fn cmd_search_next(&mut self) {
		if !self.buffer().search_matches.is_empty() {
			let len = self.buffer().search_matches.len();
			self.buffer_mut().search_match_idx = (self.buffer().search_match_idx + 1) % len;
			self.jump_to_search_match();
		}
	}

	pub(crate) fn cmd_search_prev(&mut self) {
		if !self.buffer().search_matches.is_empty() {
			let idx = self.buffer().search_match_idx;
			let len = self.buffer().search_matches.len();
			self.buffer_mut().search_match_idx = if idx == 0 { len - 1 } else { idx - 1 };
			self.jump_to_search_match();
		}
	}
}

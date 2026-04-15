use crate::editor::Editor;

impl Editor {
	/// Re-run the search against the buffer and jump to the nearest match.
	pub(crate) fn refresh_search_matches(&mut self) {
		self.search_matches = self.buffer().text.find_all(&self.search_query);
		if self.search_matches.is_empty() {
			self.clear_status();
			return;
		}
		// Find the match nearest (at or after) the saved cursor position.
		let anchor_pos = if let Some((line, col)) = self.search_saved_cursor {
			self.buffer().text.line_to_char(line) + col
		} else {
			0
		};
		self.search_match_idx = self
			.search_matches
			.iter()
			.position(|&(start, _)| start >= anchor_pos)
			.unwrap_or(0);
		self.jump_to_search_match();
	}

	/// Jump the cursor to the currently-highlighted search match.
	pub(crate) fn jump_to_search_match(&mut self) {
		if let Some(&(start, _end)) = self.search_matches.get(self.search_match_idx) {
			let line = self.buffer().text.char_to_line(start);
			let col = start - self.buffer().text.line_to_char(line);
			self.cursors.set_cursor(line, col);

		}
	}
}

use crate::editor::Editor;

impl Editor {
	/// Returns true if there is an active selection.
	pub fn has_selection(&self) -> bool {
		self.cursors.has_selection()
	}

	/// Begin tracking a selection from the current cursor position.
	pub(crate) fn begin_selection_if_needed(&mut self) {
		self.cursors.begin_selection();
	}

	/// Clear the active selection (collapses anchor to head).
	pub(crate) fn clear_selection(&mut self) {
		self.cursors.collapse_selection();
	}

	/// Get the selected text range as (start_pos, end_pos) char offsets.
	pub fn selection_range(&self) -> Option<(usize, usize)> {
		if !self.cursors.has_selection() {
			return None;
		}
		let sel = self.cursors.primary();
		let (start_c, end_c) = sel.ordered();
		let start = self.buffer().text.line_to_char(start_c.line) + start_c.col;
		let end = self.buffer().text.line_to_char(end_c.line) + end_c.col;
		Some((start, end))
	}

	/// Get the selected text string.
	pub(crate) fn get_selected_text(&self) -> Option<String> {
		let (start, end) = self.selection_range()?;
		if start == end {
			return None;
		}
		Some(self.buffer().text.slice_to_string(start..end))
	}

	/// Delete the selected text and clear the selection.
	pub(crate) fn delete_selection_if_active(&mut self) {
		if let Some((start, end)) = self.selection_range() {
			if start < end {
				self.buffer_mut().delete_range(start, end);
				let new_line = self.buffer().text.char_to_line(start);
				let new_col = start - self.buffer().text.line_to_char(new_line);
				self.cursors.set_cursor(new_line, new_col);
			}
		}
		self.clear_selection();
	}
}

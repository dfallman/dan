use crate::editor::Editor;

impl Editor {
	/// Get the char position in the buffer for the current cursor.
	pub(crate) fn cursor_char_pos(&self) -> usize {
		let c = self.buffer().cursors.cursor();
		let line_start = self.buffer().text.line_to_char(c.line);
		let line_len = self.buffer().text.line_len_chars(c.line);
		line_start + c.col.min(line_len)
	}

	/// Get the length of a line excluding the trailing newline.
	pub(crate) fn line_len_no_newline(&self, line: usize) -> usize {
		let len = self.buffer().text.line_len_chars(line);
		if len > 0 {
			let line_start = self.buffer().text.line_to_char(line);
			let last_char = self.buffer().text.char_at(line_start + len - 1);
			if last_char == '\n' {
				len - 1
			} else {
				len
			}
		} else {
			0
		}
	}

	/// Clamp anchor and head cursors so they do not exceed the document bounds.
	/// Required after operations that shrink the document (like Undo/Redo)
	/// where the cursors might otherwise point to non-existent lines or columns.
	pub(crate) fn clamp_cursors(&mut self) {
		let max_line = self.buffer().line_count().saturating_sub(1);

		// Clamp Head
		if self.buffer().cursors.primary().head.line > max_line {
			self.buffer_mut().cursors.primary_mut().head.line = max_line;
		}
		let head_line = self.buffer().cursors.primary().head.line;
		let head_len = self.line_len_no_newline(head_line);
		if self.buffer().cursors.primary().head.col > head_len {
			self.buffer_mut().cursors.primary_mut().head.set_col(head_len);
		}

		// Clamp Anchor
		if self.buffer().cursors.primary().anchor.line > max_line {
			self.buffer_mut().cursors.primary_mut().anchor.line = max_line;
		}
		let anchor_line = self.buffer().cursors.primary().anchor.line;
		let anchor_len = self.line_len_no_newline(anchor_line);
		if self.buffer().cursors.primary().anchor.col > anchor_len {
			self.buffer_mut().cursors.primary_mut().anchor.set_col(anchor_len);
		}
	}

	/// Swap the current line with the line above it. Cursor follows.
	pub(crate) fn swap_line_up(&mut self) {
		let line = self.buffer().cursors.cursor().line;
		if line == 0 {
			return;
		}
		let col = self.buffer().cursors.cursor().col;
		let total = self.buffer().line_count();

		let cur_start = self.buffer().text.line_to_char(line);
		let cur_end = if line + 1 < total {
			self.buffer().text.line_to_char(line + 1)
		} else {
			self.buffer().text.len_chars()
		};
		let above_start = self.buffer().text.line_to_char(line - 1);

		let cur_text = self.buffer().text.slice_to_string(cur_start..cur_end);
		let above_text = self.buffer().text.slice_to_string(above_start..cur_start);

		// Strip exactly one trailing newline from each (preserves the line
		// content including any empty-line identity).
		let cur_body = cur_text.strip_suffix('\n').unwrap_or(&cur_text);
		let above_body = above_text.strip_suffix('\n').unwrap_or(&above_text);

		// Rebuild: cur_body first (it moves up), then above_body.
		// Every line except possibly the very last gets a trailing \n.
		let is_last = line + 1 >= total;
		let mut new_text = String::with_capacity(cur_text.len() + above_text.len());
		new_text.push_str(cur_body);
		new_text.push('\n');
		new_text.push_str(above_body);
		if !is_last {
			new_text.push('\n');
		}

		self.buffer_mut().text.remove(above_start..cur_end);
		self.buffer_mut().text.insert_str(above_start, &new_text);
		self.buffer_mut().mark_mutated();

		self.buffer_mut().cursors.set_cursor(line - 1, col);
	}

	/// Swap the current line with the line below it. Cursor follows.
	pub(crate) fn swap_line_down(&mut self) {
		let line = self.buffer().cursors.cursor().line;
		let total = self.buffer().line_count();
		if line + 1 >= total {
			return;
		}
		let col = self.buffer().cursors.cursor().col;

		let cur_start = self.buffer().text.line_to_char(line);
		let cur_end = self.buffer().text.line_to_char(line + 1);
		let below_end = if line + 2 < total {
			self.buffer().text.line_to_char(line + 2)
		} else {
			self.buffer().text.len_chars()
		};

		let cur_text = self.buffer().text.slice_to_string(cur_start..cur_end);
		let below_text = self.buffer().text.slice_to_string(cur_end..below_end);

		// Strip exactly one trailing newline from each.
		let cur_body = cur_text.strip_suffix('\n').unwrap_or(&cur_text);
		let below_body = below_text.strip_suffix('\n').unwrap_or(&below_text);

		// Rebuild: below_body first (it moves up), then cur_body.
		let is_last = line + 2 >= total;
		let mut new_text = String::with_capacity(cur_text.len() + below_text.len());
		new_text.push_str(below_body);
		new_text.push('\n');
		new_text.push_str(cur_body);
		if !is_last {
			new_text.push('\n');
		}

		self.buffer_mut().text.remove(cur_start..below_end);
		self.buffer_mut().text.insert_str(cur_start, &new_text);
		self.buffer_mut().mark_mutated();

		self.buffer_mut().cursors.set_cursor(line + 1, col);
	}

	/// Move all lines covered by the current selection up by one line.
	/// The selection (anchor + head) follows the moved block.
	pub(crate) fn move_lines_up(&mut self) {
		let (start, end) = self.buffer().cursors.primary().ordered();
		let first_line = start.line;
		// Include the line the end cursor is on, but if end is at col 0
		// of the next line we don't drag that empty line along.
		let last_line = if end.col == 0 && end.line > first_line {
			end.line - 1
		} else {
			end.line
		};

		if first_line == 0 {
			return; // already at top
		}

		let total = self.buffer().line_count();

		// Char ranges for the block and the line above.
		let block_start = self.buffer().text.line_to_char(first_line);
		let block_end = if last_line + 1 < total {
			self.buffer().text.line_to_char(last_line + 1)
		} else {
			self.buffer().text.len_chars()
		};
		let above_start = self.buffer().text.line_to_char(first_line - 1);

		let block_text = self.buffer().text.slice_to_string(block_start..block_end);
		let above_text = self.buffer().text.slice_to_string(above_start..block_start);

		let block_body = block_text.strip_suffix('\n').unwrap_or(&block_text);
		let above_body = above_text.strip_suffix('\n').unwrap_or(&above_text);

		let is_last = last_line + 1 >= total;
		let mut new_text = String::with_capacity(block_text.len() + above_text.len());
		new_text.push_str(block_body);
		new_text.push('\n');
		new_text.push_str(above_body);
		if !is_last {
			new_text.push('\n');
		}

		self.buffer_mut().text.remove(above_start..block_end);
		self.buffer_mut().text.insert_str(above_start, &new_text);
		self.buffer_mut().mark_mutated();

		// Shift both anchor and head up by one line to follow the block.
		let sel = self.buffer_mut().cursors.primary_mut();
		sel.anchor.line = sel.anchor.line.saturating_sub(1);
		sel.head.line = sel.head.line.saturating_sub(1);
	}

	/// Move all lines covered by the current selection down by one line.
	/// The selection (anchor + head) follows the moved block.
	pub(crate) fn move_lines_down(&mut self) {
		let (start, end) = self.buffer().cursors.primary().ordered();
		let first_line = start.line;
		let last_line = if end.col == 0 && end.line > first_line {
			end.line - 1
		} else {
			end.line
		};

		let total = self.buffer().line_count();
		if last_line + 1 >= total {
			return; // already at bottom
		}

		// Char ranges for the block and the line below.
		let block_start = self.buffer().text.line_to_char(first_line);
		let block_end = self.buffer().text.line_to_char(last_line + 1);
		let below_end = if last_line + 2 < total {
			self.buffer().text.line_to_char(last_line + 2)
		} else {
			self.buffer().text.len_chars()
		};

		let block_text = self.buffer().text.slice_to_string(block_start..block_end);
		let below_text = self.buffer().text.slice_to_string(block_end..below_end);

		let block_body = block_text.strip_suffix('\n').unwrap_or(&block_text);
		let below_body = below_text.strip_suffix('\n').unwrap_or(&below_text);

		let is_last = last_line + 2 >= total;
		let mut new_text = String::with_capacity(block_text.len() + below_text.len());
		new_text.push_str(below_body);
		new_text.push('\n');
		new_text.push_str(block_body);
		if !is_last {
			new_text.push('\n');
		}

		self.buffer_mut().text.remove(block_start..below_end);
		self.buffer_mut().text.insert_str(block_start, &new_text);
		self.buffer_mut().mark_mutated();

		// Shift both anchor and head down by one line to follow the block.
		let sel = self.buffer_mut().cursors.primary_mut();
		sel.anchor.line += 1;
		sel.head.line += 1;
	}

}

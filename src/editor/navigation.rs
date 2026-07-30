use crate::editor::layout::{
	self, grapheme_next, grapheme_prev, logical_to_visual, visual_rows, visual_to_logical,
};
use crate::editor::visual_col::{char_idx_for_visual_col, visual_col_at};
use crate::editor::Editor;

impl Editor {
	fn line_text(&self, line: usize) -> String {
		self.buffer().text.line(line)
	}

	/// Move cursor horizontally by one grapheme (-1 = left, +1 = right).
	pub(crate) fn move_cursor_horizontal(&mut self, delta: i32) {
		let c = self.buffer().cursors.cursor();
		if delta < 0 {
			if c.col > 0 {
				let line = self.line_text(c.line);
				let new_col = grapheme_prev(&line, c.col);
				self.set_cursor_col_with_goal(c.line, new_col);
			} else if c.line > 0 {
				let prev_len = self.line_len_no_newline(c.line - 1);
				let prev_line = c.line - 1;
				self.buffer_mut().cursors.primary_mut().head.line = prev_line;
				self.set_cursor_col_with_goal(prev_line, prev_len);
			}
		} else {
			let line_len = self.line_len_no_newline(c.line);
			if c.col < line_len {
				let line = self.line_text(c.line);
				let new_col = grapheme_next(&line, c.col);
				self.set_cursor_col_with_goal(c.line, new_col);
			} else if c.line + 1 < self.buffer().line_count() {
				self.buffer_mut().cursors.primary_mut().head.line = c.line + 1;
				self.set_cursor_col_with_goal(c.line + 1, 0);
			}
		}
	}

	/// Update column and reset goal column from the current visual position.
	fn set_cursor_col_with_goal(&mut self, line: usize, col: usize) {
		let tab_w = self.tab_width();
		let text = self.line_text(line);
		let col = layout::grapheme_floor(&text, col);
		let vcol = if self.config.wrap_lines && self.text_area_width() > 0 {
			let opts = self.wrap_opts();
			logical_to_visual(&text, opts, col).1
		} else {
			visual_col_at(text.chars(), col, tab_w)
		};
		self.buffer_mut().cursors.primary_mut().head.set_col(col);
		self.buffer_mut().cursors.primary_mut().head.desired_vcol = vcol;
	}

	/// Move cursor vertically by `delta` visual rows (wrap) or buffer lines (no-wrap).
	pub(crate) fn move_cursor_vertical(&mut self, delta: i32) {
		if !self.config.wrap_lines {
			let c = self.buffer().cursors.cursor();
			let new_line = if delta < 0 {
				c.line.saturating_sub((-delta) as usize)
			} else {
				let max_line = self.buffer().line_count().saturating_sub(1);
				(c.line + delta as usize).min(max_line)
			};

			if new_line != c.line {
				let line_len = self.line_len_no_newline(new_line);
				let new_col = char_idx_for_visual_col(
					self.buffer().text.line_slice(new_line).chars(),
					line_len,
					0,
					line_len,
					c.desired_vcol,
					self.tab_width(),
					true,
				);
				self.buffer_mut().cursors.primary_mut().head.line = new_line;
				self.buffer_mut()
					.cursors
					.primary_mut()
					.head
					.set_col_keep_vcol(new_col);
			}
			return;
		}

		let opts = self.wrap_opts();
		if opts.width == 0 {
			return;
		}

		let steps = delta.unsigned_abs() as usize;
		for _ in 0..steps {
			self.move_cursor_visual_row(delta.signum());
		}
	}

	/// Move one visual row: `dir` is -1 (up) or +1 (down).
	fn move_cursor_visual_row(&mut self, dir: i32) {
		let opts = self.wrap_opts();
		let c = self.buffer().cursors.cursor();
		let line_count = self.buffer().line_count();
		let text = self.line_text(c.line);
		let rows = visual_rows(&text, opts);
		let (cur_vrow, _) = logical_to_visual(&text, opts, c.col);
		let goal = c.desired_vcol;

		if dir > 0 {
			if cur_vrow + 1 < rows.len() {
				let new_col = visual_to_logical(&text, opts, cur_vrow + 1, goal);
				self.buffer_mut()
					.cursors
					.primary_mut()
					.head
					.set_col_keep_vcol(new_col);
			} else {
				let next_line = c.line + 1;
				if next_line < line_count {
					let next_text = self.line_text(next_line);
					let new_col = visual_to_logical(&next_text, opts, 0, goal);
					self.buffer_mut().cursors.primary_mut().head.line = next_line;
					self.buffer_mut()
						.cursors
						.primary_mut()
						.head
						.set_col_keep_vcol(new_col);
				}
			}
		} else if cur_vrow > 0 {
			let new_col = visual_to_logical(&text, opts, cur_vrow - 1, goal);
			self.buffer_mut()
				.cursors
				.primary_mut()
				.head
				.set_col_keep_vcol(new_col);
		} else if c.line > 0 {
			let prev_line = c.line - 1;
			let prev_text = self.line_text(prev_line);
			let prev_rows = visual_rows(&prev_text, opts);
			let last = prev_rows.len() - 1;
			let new_col = visual_to_logical(&prev_text, opts, last, goal);
			self.buffer_mut().cursors.primary_mut().head.line = prev_line;
			self.buffer_mut()
				.cursors
				.primary_mut()
				.head
				.set_col_keep_vcol(new_col);
		}
	}

	/// Move Home to the start of the current visual row (wrap) or logical line.
	pub(crate) fn move_visual_or_line_home(&mut self) {
		let c = self.buffer().cursors.cursor();
		if !self.config.wrap_lines || self.text_area_width() == 0 {
			self.buffer_mut().cursors.primary_mut().head.set_col(0);
			return;
		}
		let text = self.line_text(c.line);
		let opts = self.wrap_opts();
		let (vrow, _) = logical_to_visual(&text, opts, c.col);
		let points = layout::wrap_points(&text, opts);
		let col = points[vrow];
		self.set_cursor_col_with_goal(c.line, col);
	}

	/// Move End to the end of the current visual row (wrap) or logical line.
	pub(crate) fn move_visual_or_line_end(&mut self) {
		let c = self.buffer().cursors.cursor();
		let line_len = self.line_len_no_newline(c.line);
		if !self.config.wrap_lines || self.text_area_width() == 0 {
			self.set_cursor_col_with_goal(c.line, line_len);
			return;
		}
		let text = self.line_text(c.line);
		let opts = self.wrap_opts();
		let rows = visual_rows(&text, opts);
		let (vrow, _) = logical_to_visual(&text, opts, c.col);
		let (start, end) = rows[vrow];
		let is_last = vrow + 1 == rows.len();
		// End of visual row: last row → line end; else last char of row (not
		// wrap boundary, which displays on the next row).
		let col = if is_last {
			line_len
		} else {
			end.saturating_sub(1).max(start)
		};
		self.set_cursor_col_with_goal(c.line, col);
	}

	/// Logical line start (col 0).
	pub(crate) fn move_logical_line_start(&mut self) {
		let line = self.buffer().cursors.cursor().line;
		self.set_cursor_col_with_goal(line, 0);
	}

	/// Logical line end.
	pub(crate) fn move_logical_line_end(&mut self) {
		let c = self.buffer().cursors.cursor();
		let len = self.line_len_no_newline(c.line);
		self.set_cursor_col_with_goal(c.line, len);
	}

	/// Move cursor forward one word using programming-language-aware boundaries.
	pub(crate) fn move_word_forward(&mut self) {
		let (line, col) = {
			let text = &self.buffer().text;
			let total_chars = text.len_chars();
			let c = self.buffer().cursors.cursor();
			let mut pos = text.line_to_char(c.line) + c.col;

			if pos >= total_chars {
				return;
			}

			fn char_class(ch: char) -> u8 {
				if ch.is_whitespace() {
					0
				} else if ch.is_alphanumeric() || ch == '_' {
					1
				} else {
					2
				}
			}

			let start_class = char_class(text.char_at(pos));

			if start_class != 0 {
				while pos < total_chars {
					let ch = text.char_at(pos);
					if char_class(ch) != start_class {
						break;
					}
					pos += 1;
				}
			}

			while pos < total_chars {
				let ch = text.char_at(pos);
				if char_class(ch) != 0 {
					break;
				}
				pos += 1;
			}

			let line = text.char_to_line(pos);
			let line_start = text.line_to_char(line);
			(line, pos - line_start)
		};
		self.buffer_mut().cursors.primary_mut().head.line = line;
		self.set_cursor_col_with_goal(line, col);
	}

	/// Move cursor backward one word.
	pub(crate) fn move_word_backward(&mut self) {
		let (line, col) = {
			let text = &self.buffer().text;
			let c = self.buffer().cursors.cursor();
			let mut pos = text.line_to_char(c.line) + c.col;

			if pos == 0 {
				return;
			}

			fn char_class(ch: char) -> u8 {
				if ch.is_whitespace() {
					0
				} else if ch.is_alphanumeric() || ch == '_' {
					1
				} else {
					2
				}
			}

			while pos > 0 {
				let ch = text.char_at(pos - 1);
				if char_class(ch) != 0 {
					break;
				}
				pos -= 1;
			}

			if pos > 0 {
				let target_class = char_class(text.char_at(pos - 1));
				while pos > 0 {
					let ch = text.char_at(pos - 1);
					if char_class(ch) != target_class {
						break;
					}
					pos -= 1;
				}
			}

			let line = text.char_to_line(pos);
			let line_start = text.line_to_char(line);
			(line, pos - line_start)
		};
		self.buffer_mut().cursors.primary_mut().head.line = line;
		self.set_cursor_col_with_goal(line, col);
	}
}

use crate::editor::viewport::visual_rows_for;
use crate::editor::visual_col::{char_idx_for_visual_col, visual_col_at};
use crate::editor::Editor;

impl Editor {
	/// Move cursor horizontally by `delta` chars (-1 = left, 1 = right).
	pub(crate) fn move_cursor_horizontal(&mut self, delta: i32) {
		let c = self.cursors.cursor();
		let tab_w = self.tab_width();
		if delta < 0 {
			if c.col > 0 {
				let new_col = c.col - 1;
				self.cursors.primary_mut().head.set_col(new_col);
				self.cursors.primary_mut().head.desired_vcol = visual_col_at(
					self.buffer().text.line_slice(c.line).chars(),
					new_col,
					tab_w,
				);
			} else if c.line > 0 {
				let prev_len = self.line_len_no_newline(c.line - 1);
				self.cursors.primary_mut().head.line = c.line - 1;
				self.cursors.primary_mut().head.set_col(prev_len);
				self.cursors.primary_mut().head.desired_vcol = visual_col_at(
					self.buffer().text.line_slice(c.line - 1).chars(),
					prev_len,
					tab_w,
				);
			}
		} else {
			let line_len = self.line_len_no_newline(c.line);
			if c.col < line_len {
				let new_col = c.col + 1;
				self.cursors.primary_mut().head.set_col(new_col);
				self.cursors.primary_mut().head.desired_vcol = visual_col_at(
					self.buffer().text.line_slice(c.line).chars(),
					new_col,
					tab_w,
				);
			} else if c.line + 1 < self.buffer().line_count() {
				self.cursors.primary_mut().head.line = c.line + 1;
				self.cursors.primary_mut().head.set_col(0);
				// vcol 0 is correct for col 0
			}
		}
	}

	/// Move cursor vertically by `delta` lines.
	pub(crate) fn move_cursor_vertical(&mut self, delta: i32) {
		if !self.config.wrap_lines {
			// No-wrap mode: move by buffer lines, using visual column for sticky behaviour.
			let c = self.cursors.cursor();
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
				self.cursors.primary_mut().head.line = new_line;
				self.cursors.primary_mut().head.set_col_keep_vcol(new_col);
			}
			return;
		}

		// -- Wrap mode: move by visual (screen) rows --
		let text_area_width = self.text_area_width();
		if text_area_width == 0 {
			return;
		}
		let tab_w = self.tab_width();
		let c = self.cursors.cursor();
		let line_count = self.buffer().line_count();

		let cur_line_len = self.line_len_no_newline(c.line);

		let rows = visual_rows_for(
			self.buffer().text.line_slice(c.line).chars(),
			tab_w,
			text_area_width,
		);

		// Which visual row is the cursor on within its buffer line?
		let mut cur_vrow: usize = 0;
		for (i, &(start, end)) in rows.iter().enumerate() {
			if c.col >= start && (c.col < end || i == rows.len() - 1) {
				cur_vrow = i;
				break;
			}
		}

		if delta > 0 {
			// Moving down
			if cur_vrow + 1 < rows.len() {
				// Stay on same buffer line, move to next visual row.
				let next_idx = cur_vrow + 1;
				let next_row = rows[next_idx];
				let new_col = char_idx_for_visual_col(
					self.buffer().text.line_slice(c.line).chars(),
					cur_line_len,
					next_row.0,
					next_row.1,
					c.desired_vcol,
					tab_w,
					next_idx == rows.len() - 1,
				);
				self.cursors.primary_mut().head.set_col_keep_vcol(new_col);
			} else {
				// Move to next buffer line (first visual row).
				let next_line = c.line + 1;
				if next_line < line_count {
					let next_len = self.line_len_no_newline(next_line);
					let next_rows = visual_rows_for(
						self.buffer().text.line_slice(next_line).chars(),
						tab_w,
						text_area_width,
					);
					let first = next_rows[0];
					let new_col = char_idx_for_visual_col(
						self.buffer().text.line_slice(next_line).chars(),
						next_len,
						first.0,
						first.1,
						c.desired_vcol,
						tab_w,
						next_rows.len() == 1,
					);
					self.cursors.primary_mut().head.line = next_line;
					self.cursors.primary_mut().head.set_col_keep_vcol(new_col);
				}
			}
		} else {
			// Moving up
			if cur_vrow > 0 {
				// Stay on same buffer line, move to previous visual row.
				let prev_idx = cur_vrow - 1;
				let prev_row = rows[prev_idx];
				let new_col = char_idx_for_visual_col(
					self.buffer().text.line_slice(c.line).chars(),
					cur_line_len,
					prev_row.0,
					prev_row.1,
					c.desired_vcol,
					tab_w,
					prev_idx == rows.len() - 1,
				);
				self.cursors.primary_mut().head.set_col_keep_vcol(new_col);
			} else {
				// Move to previous buffer line (last visual row).
				if c.line > 0 {
					let prev_line = c.line - 1;
					let prev_len = self.line_len_no_newline(prev_line);
					let prev_rows = visual_rows_for(
						self.buffer().text.line_slice(prev_line).chars(),
						tab_w,
						text_area_width,
					);
					let last = prev_rows[prev_rows.len() - 1];
					let new_col = char_idx_for_visual_col(
						self.buffer().text.line_slice(prev_line).chars(),
						prev_len,
						last.0,
						last.1,
						c.desired_vcol,
						tab_w,
						true,
					);
					self.cursors.primary_mut().head.line = prev_line;
					self.cursors.primary_mut().head.set_col_keep_vcol(new_col);
				}
			}
		}
	}

	/// Move cursor forward one word using programming-language-aware boundaries.
	pub(crate) fn move_word_forward(&mut self) {
		let (line, col) = {
			let text = &self.buffer().text;
			let total_chars = text.len_chars();
			let c = self.cursors.cursor();
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
				// Phase 1: skip characters of the same class (word -> word, or punct -> punct)
				while pos < total_chars {
					let ch = text.char_at(pos);
					if char_class(ch) != start_class {
						break;
					}
					pos += 1;
				}
			}

			// Phase 2: skip trailing whitespace to land cleanly on the next token
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
		let tab_w = self.tab_width();
		self.cursors.primary_mut().head.line = line;
		self.cursors.primary_mut().head.set_col(col);
		self.cursors.primary_mut().head.desired_vcol =
			visual_col_at(self.buffer().text.line_slice(line).chars(), col, tab_w);
	}

	/// Move cursor backward one word, skipping spaces to land on the start of the previous word.
	pub(crate) fn move_word_backward(&mut self) {
		let (line, col) = {
			let text = &self.buffer().text;
			let c = self.cursors.cursor();
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

			// Phase 1: skip preceding whitespace behind cursor
			while pos > 0 {
				let ch = text.char_at(pos - 1);
				if char_class(ch) != 0 {
					break;
				}
				pos -= 1;
			}

			if pos > 0 {
				let target_class = char_class(text.char_at(pos - 1));
				// Phase 2: skip characters of the same target class to find bounding start
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
		let tab_w = self.tab_width();
		self.cursors.primary_mut().head.line = line;
		self.cursors.primary_mut().head.set_col(col);
		self.cursors.primary_mut().head.desired_vcol =
			visual_col_at(self.buffer().text.line_slice(line).chars(), col, tab_w);
	}
}

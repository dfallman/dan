//! Motion and selection command handlers.
use crate::editor::Editor;

impl Editor {
	pub(crate) fn cmd_move_left(&mut self) {
		self.move_cursor_horizontal(-1);
		self.clear_selection();
	}

	pub(crate) fn cmd_move_right(&mut self) {
		self.move_cursor_horizontal(1);
		self.clear_selection();
	}

	pub(crate) fn cmd_move_up(&mut self) {
		self.move_cursor_vertical(-1);
		self.clear_selection();
	}

	pub(crate) fn cmd_move_down(&mut self) {
		self.move_cursor_vertical(1);
		self.clear_selection();
	}

	pub(crate) fn cmd_move_line_start(&mut self) {
		self.move_visual_or_line_home();
		self.clear_selection();
	}

	pub(crate) fn cmd_move_line_end(&mut self) {
		self.move_visual_or_line_end();
		self.clear_selection();
	}

	pub(crate) fn cmd_move_logical_line_start(&mut self) {
		self.move_logical_line_start();
		self.clear_selection();
	}

	pub(crate) fn cmd_move_logical_line_end(&mut self) {
		self.move_logical_line_end();
		self.clear_selection();
	}

	pub(crate) fn cmd_move_word_forward(&mut self) {
		self.move_word_forward();
		self.clear_selection();
	}

	pub(crate) fn cmd_move_word_backward(&mut self) {
		self.move_word_backward();
		self.clear_selection();
	}

	pub(crate) fn cmd_swap_line_up(&mut self) {
		if self.has_selection() {
			self.move_lines_up();
		} else {
			self.swap_line_up();
			self.clear_selection();
		}
	}

	pub(crate) fn cmd_swap_line_down(&mut self) {
		if self.has_selection() {
			self.move_lines_down();
		} else {
			self.swap_line_down();
			self.clear_selection();
		}
	}

	pub(crate) fn cmd_move_buffer_top(&mut self) {
		self.buffer_mut().cursors.primary_mut().head.line = 0;
		self.buffer_mut().cursors.primary_mut().head.set_col(0);
		self.clear_selection();
	}

	pub(crate) fn cmd_move_buffer_bottom(&mut self) {
		let last_line = self.buffer().line_count().saturating_sub(1);
		self.buffer_mut().cursors.primary_mut().head.line = last_line;
		self.buffer_mut().cursors.primary_mut().head.set_col(0);
		self.clear_selection();
	}

	pub(crate) fn cmd_page_up(&mut self) {
		// Scroll by visible text area height (terminal height minus status + command bars)
		let page = (self.terminal_height as usize).saturating_sub(2).max(1);
		for _ in 0..page {
			self.move_cursor_vertical(-1);
		}
		self.clear_selection();
	}

	pub(crate) fn cmd_page_down(&mut self) {
		let page = (self.terminal_height as usize).saturating_sub(2).max(1);
		for _ in 0..page {
			self.move_cursor_vertical(1);
		}
		self.clear_selection();
	}

	pub(crate) fn cmd_scroll_viewport_up(&mut self) {
		let keep_sel = self.has_selection();
		if self.config.wrap_lines && self.text_area_width() > 0 {
			self.scroll_visual_rows_up(1);
		} else {
			let new_scroll_y = self.buffer().scroll_y.saturating_sub(1);
			self.buffer_mut().scroll_y = new_scroll_y;
		}
		// While a selection is active, leave the cursor (selection head) alone so
		// wheel / Ctrl+↑ scroll can pan without collapsing or reshaping the range.
		if !keep_sel {
			let visible_height = self.terminal_height.saturating_sub(2) as usize;
			let cursor_line = self.buffer().cursors.cursor().line;
			if cursor_line
				>= self.buffer().scroll_y + visible_height.saturating_sub(self.config.scroll_off)
			{
				self.move_cursor_vertical(-1);
			}
			self.clear_selection();
		}
	}

	pub(crate) fn cmd_scroll_viewport_down(&mut self) {
		let keep_sel = self.has_selection();
		if self.config.wrap_lines && self.text_area_width() > 0 {
			self.scroll_visual_rows_down(1);
		} else {
			self.buffer_mut().scroll_y += 1;
		}
		if !keep_sel {
			let cursor_line = self.buffer().cursors.cursor().line;
			if cursor_line < self.buffer().scroll_y + self.config.scroll_off {
				self.move_cursor_vertical(1);
			}
			self.clear_selection();
		}
	}

	/// Scroll the viewport up by `n` visual rows (wrap mode).
	fn scroll_visual_rows_up(&mut self, n: usize) {
		for _ in 0..n {
			if self.buffer().scroll_vrow > 0 {
				self.buffer_mut().scroll_vrow -= 1;
			} else if self.buffer().scroll_y > 0 {
				self.buffer_mut().scroll_y -= 1;
				let y = self.buffer().scroll_y;
				let h = self.cached_visual_height(y);
				self.buffer_mut().scroll_vrow = h.saturating_sub(1);
			} else {
				break;
			}
		}
	}

	/// Scroll the viewport down by `n` visual rows (wrap mode).
	fn scroll_visual_rows_down(&mut self, n: usize) {
		let line_count = self.buffer().line_count();
		for _ in 0..n {
			let y = self.buffer().scroll_y;
			if y >= line_count {
				break;
			}
			let h = self.cached_visual_height(y);
			if self.buffer().scroll_vrow + 1 < h {
				self.buffer_mut().scroll_vrow += 1;
			} else if y + 1 < line_count {
				self.buffer_mut().scroll_y += 1;
				self.buffer_mut().scroll_vrow = 0;
			} else {
				break;
			}
		}
	}

	pub(crate) fn cmd_move_fast_up(&mut self) {
		for _ in 0..self.config.fast_scroll_steps {
			self.move_cursor_vertical(-1);
		}
		self.clear_selection();
	}

	pub(crate) fn cmd_move_fast_down(&mut self) {
		for _ in 0..self.config.fast_scroll_steps {
			self.move_cursor_vertical(1);
		}
		self.clear_selection();
	}

	pub(crate) fn cmd_select_left(&mut self) {
		self.begin_selection_if_needed();
		self.move_cursor_horizontal(-1);
	}

	pub(crate) fn cmd_select_right(&mut self) {
		self.begin_selection_if_needed();
		self.move_cursor_horizontal(1);
	}

	pub(crate) fn cmd_select_up(&mut self) {
		self.begin_selection_if_needed();
		self.move_cursor_vertical(-1);
	}

	pub(crate) fn cmd_select_down(&mut self) {
		self.begin_selection_if_needed();
		self.move_cursor_vertical(1);
	}

	pub(crate) fn cmd_select_word_forward(&mut self) {
		self.begin_selection_if_needed();
		self.move_word_forward();
	}

	pub(crate) fn cmd_select_word_backward(&mut self) {
		self.begin_selection_if_needed();
		self.move_word_backward();
	}

	pub(crate) fn cmd_select_line_start(&mut self) {
		self.begin_selection_if_needed();
		self.move_visual_or_line_home();
	}

	pub(crate) fn cmd_select_line_end(&mut self) {
		self.begin_selection_if_needed();
		self.move_visual_or_line_end();
	}

	pub(crate) fn cmd_select_logical_line_start(&mut self) {
		self.begin_selection_if_needed();
		self.move_logical_line_start();
	}

	pub(crate) fn cmd_select_logical_line_end(&mut self) {
		self.begin_selection_if_needed();
		self.move_logical_line_end();
	}

	pub(crate) fn cmd_select_all(&mut self) {
		let last_line = self.buffer().line_count().saturating_sub(1);
		let last_col = self.line_len_no_newline(last_line);
		// Set anchor at start of buffer, head at end.
		use crate::editor::cursor::Cursor;
		self.buffer_mut().cursors.primary_mut().anchor = Cursor::new(0, 0);
		self.buffer_mut().cursors.primary_mut().head = Cursor::new(last_line, last_col);
	}
}

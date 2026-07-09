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
		self.buffer_mut().cursors.primary_mut().head.set_col(0);
		self.clear_selection();
	}

	pub(crate) fn cmd_move_line_end(&mut self) {
		let c = self.buffer().cursors.cursor();
		let len = self.line_len_no_newline(c.line);
		self.buffer_mut().cursors.primary_mut().head.set_col(len);
		self.buffer_mut().cursors.primary_mut().head.desired_vcol =
			crate::editor::visual_col::visual_col_at(
				self.buffer().text.line_slice(c.line).chars(),
				len,
				self.tab_width(),
			);
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
		let new_scroll_y = self.buffer().scroll_y.saturating_sub(1);
		self.buffer_mut().scroll_y = new_scroll_y;
		let visible_height = self.terminal_height.saturating_sub(2) as usize;
		let cursor_line = self.buffer().cursors.cursor().line;
		// Maintain VSCode-style viewport tether: push cursor back up if it would fall out of the bottom bound
		if cursor_line
			>= self.buffer_mut().scroll_y + visible_height.saturating_sub(self.config.scroll_off)
		{
			self.move_cursor_vertical(-1);
		}
		self.clear_selection();
	}

	pub(crate) fn cmd_scroll_viewport_down(&mut self) {
		self.buffer_mut().scroll_y += 1;
		let cursor_line = self.buffer().cursors.cursor().line;
		// Maintain VSCode-style viewport tether: pull cursor down if it would fall out of the top bound
		if cursor_line < self.buffer_mut().scroll_y + self.config.scroll_off {
			self.move_cursor_vertical(1);
		}
		self.clear_selection();
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
		self.buffer_mut().cursors.primary_mut().head.set_col(0);
	}

	pub(crate) fn cmd_select_line_end(&mut self) {
		self.begin_selection_if_needed();
		let c = self.buffer().cursors.cursor();
		let len = self.line_len_no_newline(c.line);
		let tab_w = self.tab_width();
		let vcol = crate::editor::visual_col::visual_col_at(
			self.buffer().text.line_slice(c.line).chars(),
			len,
			tab_w,
		);
		self.buffer_mut().cursors.primary_mut().head.set_col(len);
		self.buffer_mut().cursors.primary_mut().head.desired_vcol = vcol;
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

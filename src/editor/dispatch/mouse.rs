use super::Editor;
use crate::editor::mouse::screen_to_buffer;

impl Editor {
	pub(crate) fn cmd_mouse_down(&mut self, col: u16, row: u16) {
		if !self.config.mouse {
			return;
		}
		let Some((line, c)) = screen_to_buffer(self, col, row) else {
			return;
		};
		self.buffer_mut().cursors.set_cursor(line, c);
		let tab_w = self.tab_width();
		let vcol = crate::editor::visual_col::visual_col_at(
			self.buffer().text.line_slice(line).chars(),
			c,
			tab_w,
		);
		self.buffer_mut().cursors.primary_mut().head.desired_vcol = vcol;
	}

	pub(crate) fn cmd_mouse_drag(&mut self, col: u16, row: u16) {
		if !self.config.mouse {
			return;
		}
		let Some((line, c)) = screen_to_buffer(self, col, row) else {
			return;
		};
		let tab_w = self.tab_width();
		let vcol = crate::editor::visual_col::visual_col_at(
			self.buffer().text.line_slice(line).chars(),
			c,
			tab_w,
		);
		let head = &mut self.buffer_mut().cursors.primary_mut().head;
		head.line = line;
		head.set_col(c);
		head.desired_vcol = vcol;
	}

	pub(crate) fn cmd_mouse_up(&mut self, _col: u16, _row: u16) {
		// Selection already updated during drag; collapsed if never moved.
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::editor::commands::Command;

	fn editor_with_lines(lines: &[&str], width: u16, height: u16) -> Editor {
		let mut e = Editor::new();
		e.terminal_width = width;
		e.terminal_height = height;
		e.show_help = false;
		e.config.wrap_lines = false;
		e.config.line_numbers = true;
		e.config.mouse = true;
		e.execute(Command::SelectAll);
		e.execute(Command::DeleteForward);
		for (i, line) in lines.iter().enumerate() {
			if i > 0 {
				e.execute(Command::InsertNewline);
			}
			for ch in line.chars() {
				e.execute(Command::InsertChar(ch));
			}
		}
		e.execute(Command::MoveBufferTop);
		e
	}

	#[test]
	fn mouse_down_places_cursor() {
		let mut e = editor_with_lines(&["abcdef"], 40, 10);
		let gw = (e.gutter_width() + 1) as u16;
		e.execute(Command::MouseDown {
			col: gw + 2,
			row: 0,
		});
		let c = e.buffer().cursors.cursor();
		assert_eq!((c.line, c.col), (0, 2));
		assert!(!e.buffer().cursors.has_selection());
	}

	#[test]
	fn mouse_drag_selects_range() {
		let mut e = editor_with_lines(&["abcdef"], 40, 10);
		let gw = (e.gutter_width() + 1) as u16;
		e.execute(Command::MouseDown { col: gw, row: 0 });
		e.execute(Command::MouseDrag {
			col: gw + 3,
			row: 0,
		});
		e.execute(Command::MouseUp {
			col: gw + 3,
			row: 0,
		});
		assert!(e.buffer().cursors.has_selection());
		let (a, b) = e.buffer().cursors.primary().ordered();
		assert_eq!((a.line, a.col), (0, 0));
		assert_eq!((b.line, b.col), (0, 3));
	}

	#[test]
	fn mouse_disabled_is_noop() {
		let mut e = editor_with_lines(&["abcdef"], 40, 10);
		e.config.mouse = false;
		let before = e.buffer().cursors.cursor();
		e.execute(Command::MouseDown { col: 5, row: 0 });
		assert_eq!(e.buffer().cursors.cursor(), before);
	}

	#[test]
	fn wheel_scrolls_without_moving_cursor() {
		let mut e = editor_with_lines(
			&["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"],
			40,
			10,
		);
		e.buffer_mut().scroll_y = 5;
		let cur = e.buffer().cursors.cursor();
		e.execute(Command::ScrollViewportUp);
		assert_eq!(e.buffer().scroll_y, 4);
		assert_eq!(e.buffer().cursors.cursor(), cur);
	}

	#[test]
	fn wheel_preserves_selection() {
		let mut e = editor_with_lines(
			&["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"],
			40,
			20,
		);
		e.config.scroll_off = 0;
		let gw = (e.gutter_width() + 1) as u16;
		e.execute(Command::MouseDown { col: gw, row: 0 });
		e.execute(Command::MouseDrag {
			col: gw + 1,
			row: 2,
		});
		assert!(e.buffer().cursors.has_selection());
		let before = *e.buffer().cursors.primary();
		e.buffer_mut().scroll_y = 0;
		e.execute(Command::ScrollViewportDown);
		e.execute(Command::ScrollViewportDown);
		assert!(e.buffer().cursors.has_selection());
		assert_eq!(*e.buffer().cursors.primary(), before);
		assert_eq!(e.buffer().scroll_y, 2);
	}
}

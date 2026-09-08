use super::Editor;
use crate::editor::cursor::Cursor;
use crate::editor::mouse::screen_to_buffer;

impl Editor {
	pub(crate) fn cmd_mouse_down(&mut self, col: u16, row: u16, extend: bool) {
		if !self.config.mouse {
			return;
		}
		if self.scrollbar_hit(col, row) {
			self.scrollbar_drag_start(row);
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
		let click = Cursor {
			line,
			col: c,
			desired_vcol: vcol,
		};

		if extend && self.has_selection() {
			// Keep the far end of the existing selection as the new anchor;
			// move the head to the click. Click above → select click..old_end;
			// click below → select old_start..click; click inside → contract.
			let (start, end) = self.buffer().cursors.primary().ordered();
			let click_before_start = line < start.line || (line == start.line && c < start.col);
			let anchor = if click_before_start { end } else { start };
			let sel = self.buffer_mut().cursors.primary_mut();
			sel.anchor = anchor;
			sel.head = click;
			return;
		}

		if extend {
			// No prior selection: pin anchor at current cursor, head at click.
			let anchor = self.buffer().cursors.cursor();
			let sel = self.buffer_mut().cursors.primary_mut();
			sel.anchor = anchor;
			sel.head = click;
			return;
		}

		self.buffer_mut().cursors.set_cursor(line, c);
		self.buffer_mut().cursors.primary_mut().head.desired_vcol = vcol;
	}

	pub(crate) fn cmd_mouse_drag(&mut self, col: u16, row: u16) {
		if !self.config.mouse {
			return;
		}
		if self.scrollbar_drag_to(row) {
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

	pub(crate) fn cmd_mouse_move(&mut self, col: u16, row: u16) {
		if !self.config.mouse {
			return;
		}
		self.scrollbar_hover_at(col, row, std::time::Instant::now());
	}

	pub(crate) fn cmd_mouse_up(&mut self, _col: u16, _row: u16) {
		// Selection already updated during drag; collapsed if never moved.
		self.scrollbar_drag_end();
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
			extend: false,
		});
		let c = e.buffer().cursors.cursor();
		assert_eq!((c.line, c.col), (0, 2));
		assert!(!e.buffer().cursors.has_selection());
	}

	#[test]
	fn mouse_drag_selects_range() {
		let mut e = editor_with_lines(&["abcdef"], 40, 10);
		let gw = (e.gutter_width() + 1) as u16;
		e.execute(Command::MouseDown {
			col: gw,
			row: 0,
			extend: false,
		});
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
		e.execute(Command::MouseDown {
			col: 5,
			row: 0,
			extend: false,
		});
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
		e.execute(Command::MouseDown {
			col: gw,
			row: 0,
			extend: false,
		});
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

	/// 100 lines, 10-row terminal, no help bar → 9 text rows, 1-row thumb.
	fn scrollbar_editor() -> Editor {
		let lines: Vec<String> = (0..100).map(|i| format!("l{i}")).collect();
		let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
		let mut e = editor_with_lines(&refs, 40, 10);
		e.config.scrollbar = "always".to_string();
		e
	}

	#[test]
	fn scrollbar_track_click_jumps_without_moving_cursor() {
		let mut e = scrollbar_editor();
		let cur = e.buffer().cursors.cursor();
		let x = e.terminal_width - 1;
		e.execute(Command::MouseDown { col: x, row: 8, extend: false });
		assert_eq!(e.buffer().scroll_y, 91, "bottom track row → last page");
		assert_eq!(e.buffer().cursors.cursor(), cur, "cursor must not follow the click");
		assert!(e.pin_viewport, "viewport must stay where the scrollbar put it");
		assert!(e.scrollbar_drag.is_some());
	}

	#[test]
	fn scrollbar_thumb_drag_scrolls_and_release_ends_drag() {
		let mut e = scrollbar_editor();
		let x = e.terminal_width - 1;
		e.execute(Command::MouseDown { col: x, row: 0, extend: false });
		assert_eq!(e.buffer().scroll_y, 0);
		// Dragging off the column still moves the thumb.
		e.execute(Command::MouseDrag { col: x - 5, row: 4 });
		assert!(e.buffer().scroll_y > 0 && e.buffer().scroll_y < 91, "{}", e.buffer().scroll_y);
		assert!(!e.buffer().cursors.has_selection());
		e.execute(Command::MouseUp { col: x - 5, row: 4 });
		assert!(e.scrollbar_drag.is_none());
	}

	#[test]
	fn mouse_move_keeps_viewport_pinned_and_cursor_put() {
		let mut e = scrollbar_editor();
		e.config.scrollbar = "scrolling".to_string();
		e.execute(Command::ScrollViewportDown);
		assert!(e.pin_viewport);
		let cur = e.buffer().cursors.cursor();
		let x = e.terminal_width - 1;
		e.execute(Command::MouseMove { col: x, row: 3 });
		assert!(e.pin_viewport, "hover must not unpin a wheel-panned viewport");
		assert_eq!(e.buffer().cursors.cursor(), cur);
		assert!(e.scrollbar_visible_at(std::time::Instant::now()));
	}

	#[test]
	fn scrollbar_click_ignored_when_mode_none() {
		let mut e = scrollbar_editor();
		e.config.scrollbar = "none".to_string();
		let x = e.terminal_width - 1;
		e.execute(Command::MouseDown { col: x, row: 8, extend: false });
		assert_eq!(e.buffer().scroll_y, 0);
		assert!(e.scrollbar_drag.is_none());
		// Falls through to text hit-testing: cursor moves to that row.
		assert_eq!(e.buffer().cursors.cursor().line, 8);
	}

	#[test]
	fn scrollbar_click_ignored_when_mouse_off() {
		let mut e = scrollbar_editor();
		e.config.mouse = false;
		let x = e.terminal_width - 1;
		e.execute(Command::MouseDown { col: x, row: 8, extend: false });
		assert_eq!(e.buffer().scroll_y, 0);
		assert!(e.scrollbar_drag.is_none());
	}

	#[test]
	fn shift_click_above_extends_to_old_end() {
		let mut e = editor_with_lines(&["aaaa", "bbbb", "cccc", "dddd"], 40, 20);
		let gw = (e.gutter_width() + 1) as u16;
		// Select line 1 col0 .. line 2 col2 via drag
		e.execute(Command::MouseDown {
			col: gw,
			row: 1,
			extend: false,
		});
		e.execute(Command::MouseDrag {
			col: gw + 2,
			row: 2,
		});
		let (start, end) = e.buffer().cursors.primary().ordered();
		assert_eq!((start.line, start.col), (1, 0));
		assert_eq!((end.line, end.col), (2, 2));

		// Shift-click above on line 0 → click .. old end
		e.execute(Command::MouseDown {
			col: gw + 1,
			row: 0,
			extend: true,
		});
		let (a, b) = e.buffer().cursors.primary().ordered();
		assert_eq!((a.line, a.col), (0, 1));
		assert_eq!((b.line, b.col), (2, 2));
	}

	#[test]
	fn shift_click_below_extends_from_old_start() {
		let mut e = editor_with_lines(&["aaaa", "bbbb", "cccc", "dddd"], 40, 20);
		let gw = (e.gutter_width() + 1) as u16;
		e.execute(Command::MouseDown {
			col: gw,
			row: 1,
			extend: false,
		});
		e.execute(Command::MouseDrag {
			col: gw + 2,
			row: 2,
		});

		// Shift-click below on line 3 → old start .. click
		e.execute(Command::MouseDown {
			col: gw + 3,
			row: 3,
			extend: true,
		});
		let (a, b) = e.buffer().cursors.primary().ordered();
		assert_eq!((a.line, a.col), (1, 0));
		assert_eq!((b.line, b.col), (3, 3));
	}

	#[test]
	fn shift_click_inside_contracts() {
		let mut e = editor_with_lines(&["aaaa", "bbbb", "cccc", "dddd"], 40, 20);
		let gw = (e.gutter_width() + 1) as u16;
		e.execute(Command::MouseDown {
			col: gw,
			row: 1,
			extend: false,
		});
		e.execute(Command::MouseDrag {
			col: gw + 2,
			row: 2,
		});

		// Shift-click inside on line 2 col0 → old start .. click
		e.execute(Command::MouseDown {
			col: gw,
			row: 2,
			extend: true,
		});
		let (a, b) = e.buffer().cursors.primary().ordered();
		assert_eq!((a.line, a.col), (1, 0));
		assert_eq!((b.line, b.col), (2, 0));
	}
}

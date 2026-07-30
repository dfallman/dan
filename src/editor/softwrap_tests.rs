//! Soft-wrap movement tests at the editor-command level.

#[cfg(test)]
mod softwrap_motion_tests {
	use crate::editor::commands::Command;
	use crate::editor::Editor;

	fn wrapped_editor(body: &str, text_width: u16, height: u16) -> Editor {
		let mut e = Editor::new();
		// text_area_width = terminal_width - (gutter + 1); gutter=0 → need +1
		e.terminal_width = text_width + 1;
		e.terminal_height = height;
		e.show_help = false;
		e.config.wrap_lines = true;
		e.config.breakindent = false;
		e.config.line_numbers = false;
		e.buffer_mut().insert_str(0, body);
		e.buffer_mut().cursors.set_cursor(0, 0);
		assert_eq!(e.text_area_width(), text_width as usize);
		e
	}

	#[test]
	fn down_up_through_wrapped_line_preserves_goal_column() {
		// width 10: "abcdefghij" = one row; "0123456789ABCDEF" = two rows (0-9, A-F)
		// short line "hi" in between.
		let mut e = wrapped_editor("abcdefghijklmnop\nhi\nABCDEFGHIJKLMNOP\n", 10, 24);
		// Start at col 5 on line 0 ("f")
		e.buffer_mut().cursors.set_cursor(0, 5);
		e.buffer_mut().cursors.primary_mut().head.desired_vcol = 5;

		// Down within wrapped line → next visual row, same goal
		e.execute(Command::MoveDown);
		let c = e.buffer().cursors.cursor();
		assert_eq!(c.line, 0);
		assert_eq!(c.col, 15); // second row starts at 10; goal 5 → col 15
		assert_eq!(c.desired_vcol, 5);

		// Down onto short line → clamps
		e.execute(Command::MoveDown);
		let c = e.buffer().cursors.cursor();
		assert_eq!(c.line, 1);
		assert_eq!(c.col, 2);
		assert_eq!(c.desired_vcol, 5);

		// Down onto long line → restores goal
		e.execute(Command::MoveDown);
		let c = e.buffer().cursors.cursor();
		assert_eq!(c.line, 2);
		assert_eq!(c.col, 5);
		assert_eq!(c.desired_vcol, 5);

		// Up back through short line restores again
		e.execute(Command::MoveUp);
		e.execute(Command::MoveUp);
		let c = e.buffer().cursors.cursor();
		assert_eq!(c.line, 0);
		assert_eq!(c.col, 15);
	}

	#[test]
	fn right_left_across_wrap_boundary() {
		let mut e = wrapped_editor("abcdefghijklmnop\n", 10, 24);
		e.buffer_mut().cursors.set_cursor(0, 9); // last char of first visual row
		e.execute(Command::MoveRight);
		let c = e.buffer().cursors.cursor();
		assert_eq!(c.col, 10); // start of next visual row
		let (vr, vc) = crate::editor::layout::logical_to_visual(
			&e.buffer().text.line(0),
			e.wrap_opts(),
			c.col,
		);
		assert_eq!(vr, 1);
		assert_eq!(vc, 0);

		e.execute(Command::MoveLeft);
		assert_eq!(e.buffer().cursors.cursor().col, 9);
	}

	#[test]
	fn home_end_within_wrapped_row() {
		let mut e = wrapped_editor("abcdefghijklmnop\n", 10, 24);
		e.buffer_mut().cursors.set_cursor(0, 12); // second visual row
		e.execute(Command::MoveLineStart);
		assert_eq!(e.buffer().cursors.cursor().col, 10);
		e.execute(Command::MoveLineEnd);
		// last visual row end = line len (16) since only two rows of 10+6
		assert_eq!(e.buffer().cursors.cursor().col, 16);

		e.buffer_mut().cursors.set_cursor(0, 3);
		e.execute(Command::MoveLineEnd);
		assert_eq!(e.buffer().cursors.cursor().col, 9); // end of first visual row
	}

	#[test]
	fn logical_home_end_bindings() {
		let mut e = wrapped_editor("abcdefghijklmnop\n", 10, 24);
		e.buffer_mut().cursors.set_cursor(0, 12);
		e.execute(Command::MoveLogicalLineStart);
		assert_eq!(e.buffer().cursors.cursor().col, 0);
		e.execute(Command::MoveLogicalLineEnd);
		assert_eq!(e.buffer().cursors.cursor().col, 16);
	}

	#[test]
	fn page_down_moves_by_visual_rows() {
		// Many short lines so page math is simple, plus one wrapped line.
		let mut body = String::new();
		for i in 0..40 {
			body.push_str(&format!("L{i}\n"));
		}
		body.push_str(&"x".repeat(50));
		body.push('\n');
		let mut e = wrapped_editor(&body, 20, 12);
		// page = height - 2 = 10
		e.buffer_mut().cursors.set_cursor(0, 0);
		e.execute(Command::PageDown);
		assert_eq!(e.buffer().cursors.cursor().line, 10);
	}

	#[test]
	fn resize_keeps_logical_cursor_and_visible() {
		let mut e = wrapped_editor(&"a".repeat(80), 40, 20);
		e.buffer_mut().cursors.set_cursor(0, 50);
		let before = e.buffer().cursors.cursor();
		e.handle_resize(20, 20);
		let after = e.buffer().cursors.cursor();
		assert_eq!(after.line, before.line);
		assert_eq!(after.col, before.col);
		// Force scroll adjust via a no-op render path: page doesn't change col
		assert!(e.text_area_width() < 40);
	}
}

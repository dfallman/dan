//! Mouse hit-testing: screen cells → buffer (line, col).

use crate::editor::layout::{self, visual_to_logical};
use crate::editor::visual_col::char_idx_for_visual_col;
use crate::editor::Editor;
use crate::render::overlay_rows_for;

/// Map a screen cell to a buffer position, or `None` if the click is on chrome.
pub(crate) fn screen_to_buffer(
	editor: &Editor,
	screen_col: u16,
	screen_row: u16,
) -> Option<(usize, usize)> {
	let width = editor.terminal_width;
	let height = editor.terminal_height;
	if width == 0 || height == 0 {
		return None;
	}

	let overlay = overlay_rows_for(editor, width, height);
	// Status (1) + overlays cover the bottom; text occupies rows above that.
	let visible = height.saturating_sub(1 + overlay);
	if screen_row >= visible {
		return None;
	}

	let gutter = editor.gutter_width();
	let text_start = gutter + 1; // gutter digits + separator space (matches render)
	let text_area_width = editor.text_area_width();
	if text_area_width == 0 {
		return None;
	}

	let line_count = editor.buffer().line_count();
	if line_count == 0 {
		return Some((0, 0));
	}

	let tab_w = editor.tab_width();
	let target_row = screen_row as usize;

	if editor.config.wrap_lines {
		screen_to_buffer_wrap(
			editor,
			screen_col,
			target_row,
			text_start,
			line_count,
		)
	} else {
		screen_to_buffer_nowrap(
			editor,
			screen_col,
			target_row,
			text_start,
			tab_w,
			line_count,
		)
	}
}

fn screen_to_buffer_nowrap(
	editor: &Editor,
	screen_col: u16,
	target_row: usize,
	text_start: usize,
	tab_w: usize,
	line_count: usize,
) -> Option<(usize, usize)> {
	let line = editor.buffer().scroll_y + target_row;
	if line >= line_count {
		let last = line_count - 1;
		let len = editor.line_len_no_newline(last);
		return Some((last, len));
	}

	if (screen_col as usize) < text_start {
		return Some((line, 0));
	}

	let target_vcol = (screen_col as usize - text_start) + editor.scroll_x;
	let len = editor.line_len_no_newline(line);
	let col = char_idx_for_visual_col(
		editor.buffer().text.line_slice(line).chars(),
		len,
		0,
		len,
		target_vcol,
		tab_w,
		true,
	);
	Some((line, col))
}

fn screen_to_buffer_wrap(
	editor: &Editor,
	screen_col: u16,
	target_row: usize,
	text_start: usize,
	line_count: usize,
) -> Option<(usize, usize)> {
	let opts = editor.wrap_opts();
	let mut remaining = target_row;
	let mut buf_line = editor.buffer().scroll_y;
	let mut start_vrow = editor.buffer().scroll_vrow;

	while buf_line < line_count {
		let text = editor.buffer().text.line(buf_line);
		let rows = layout::visual_rows(&text, opts);
		let available = rows.len().saturating_sub(start_vrow);
		if remaining < available {
			let vrow_idx = start_vrow + remaining;
			if (screen_col as usize) < text_start {
				return Some((buf_line, rows[vrow_idx].0));
			}
			let target_vcol = screen_col as usize - text_start;
			let col = visual_to_logical(&text, opts, vrow_idx, target_vcol);
			return Some((buf_line, col));
		}
		remaining -= available;
		buf_line += 1;
		start_vrow = 0;
	}

	let last = line_count - 1;
	let len = editor.line_len_no_newline(last);
	Some((last, len))
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
		let body = lines.join("\n");
		e.buffer_mut().insert_str(0, &body);
		e
	}

	#[test]
	fn nowrap_click_maps_to_char() {
		let mut e = editor_with_lines(&["abcdef"], 40, 20);
		e.config.wrap_lines = false;
		// gutter for 1 line: 1 digit + sep → text starts at col 2
		let (line, col) = screen_to_buffer(&e, 2, 0).unwrap();
		assert_eq!((line, col), (0, 0));
		let (line, col) = screen_to_buffer(&e, 5, 0).unwrap();
		assert_eq!((line, col), (0, 3));
	}

	#[test]
	fn wrap_second_visual_row() {
		// width 8: gutter for 1 line is 1 + sep → text_area = 6 → "abcdefghij" wraps.
		let mut e = editor_with_lines(&["abcdefghij"], 8, 20);
		e.config.wrap_lines = true;
		e.config.breakindent = false;
		let text_w = e.text_area_width();
		assert!(text_w > 0 && text_w < 10, "precondition: wraps, text_w={text_w}");
		// First visual row starts at screen row 0; second at row 1.
		let (line, col) = screen_to_buffer(&e, 2, 1).unwrap();
		assert_eq!(line, 0);
		assert!(col > 0, "second visual row should map past col 0, got {col}");
	}

	#[test]
	fn scroll_commands_exist() {
		let mut e = editor_with_lines(&["a"; 30], 40, 10);
		e.execute(Command::ScrollViewportDown);
	}
}

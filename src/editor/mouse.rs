//! Mouse hit-testing: screen cells → buffer (line, col).

use crate::editor::visual_col::char_idx_for_visual_col;
use crate::editor::visual_rows_for;
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
			text_area_width,
			tab_w,
			line_count,
			gutter,
		)
	} else {
		screen_to_buffer_nowrap(
			editor,
			screen_col,
			target_row,
			text_start,
			tab_w,
			line_count,
			gutter,
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
	_gutter: usize,
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
	text_area_width: usize,
	tab_w: usize,
	line_count: usize,
	_gutter: usize,
) -> Option<(usize, usize)> {
	let mut remaining = target_row;
	let mut buf_line = editor.buffer().scroll_y;
	let mut start_vrow = editor.buffer().scroll_vrow;

	while buf_line < line_count {
		let vrows = visual_rows_for(
			editor.buffer().text.line_slice(buf_line).chars(),
			tab_w,
			text_area_width,
		);
		let available = vrows.len().saturating_sub(start_vrow);
		if remaining < available {
			let vrow_idx = start_vrow + remaining;
			let (row_start, row_end) = vrows[vrow_idx];
			let is_last = vrow_idx + 1 == vrows.len();
			let len = editor.line_len_no_newline(buf_line);

			if (screen_col as usize) < text_start {
				return Some((buf_line, 0));
			}

			let target_vcol = screen_col as usize - text_start;
			let col = char_idx_for_visual_col(
				editor.buffer().text.line_slice(buf_line).chars(),
				len,
				row_start,
				row_end,
				target_vcol,
				tab_w,
				is_last,
			);
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
	fn chrome_click_is_none() {
		let e = editor_with_lines(&["hello"], 40, 10);
		assert_eq!(screen_to_buffer(&e, 5, 9), None);
	}

	#[test]
	fn nowrap_click_maps_to_char() {
		let e = editor_with_lines(&["abcdef"], 40, 10);
		let gw = e.gutter_width() + 1;
		assert_eq!(screen_to_buffer(&e, gw as u16, 0), Some((0, 0)));
		assert_eq!(screen_to_buffer(&e, (gw + 2) as u16, 0), Some((0, 2)));
	}

	#[test]
	fn gutter_click_goes_col_zero() {
		let e = editor_with_lines(&["abcdef"], 40, 10);
		assert_eq!(screen_to_buffer(&e, 0, 0), Some((0, 0)));
	}

	#[test]
	fn past_eol_clamps() {
		let e = editor_with_lines(&["ab"], 40, 10);
		let gw = e.gutter_width() + 1;
		assert_eq!(screen_to_buffer(&e, (gw + 50) as u16, 0), Some((0, 2)));
	}

	#[test]
	fn wrap_second_visual_row() {
		// width 8: gutter for 1 line is 1 + sep → text_area = 6 → "abcdefghij" wraps.
		let mut e = editor_with_lines(&["abcdefghij"], 8, 10);
		e.config.wrap_lines = true;
		e.terminal_width = 8;
		let gw = e.gutter_width() + 1;
		let text_w = e.text_area_width();
		assert!(text_w > 0 && text_w < 10, "precondition: wraps, text_w={text_w}");
		assert_eq!(screen_to_buffer(&e, gw as u16, 1), Some((0, text_w)));
	}
}

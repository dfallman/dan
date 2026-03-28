use crossterm::{
	cursor,
	style::{self, Color, SetBackgroundColor, SetForegroundColor},
	QueueableCommand,
};
use std::io::{self, Write};

use crate::editor::Editor;
use crate::editor::mode::Mode;
use super::{write_spaces, Viewport};

/// Render the status bar.
pub fn render_status_bar<W: Write>(
	editor: &Editor,
	w: &mut W,
	vp: &Viewport,
) -> io::Result<()> {
	let status_y = vp.height.saturating_sub(vp.chrome_rows);
	w.queue(cursor::MoveTo(0, status_y))?;

	let width = vp.width as usize;
	let mut used: usize = 0;

	// Mode indicator (derive visual mode from selection state)
	let (mode_bg, mode_text) = if editor.has_selection() {
		(Color::DarkYellow, "SELECT")
	} else {
		(editor.mode.color(), editor.mode.label())
	};
	let mode_label = format!(" {} ", mode_text);
	w.queue(SetBackgroundColor(mode_bg))?;
	w.queue(SetForegroundColor(Color::Black))?;
	w.queue(style::Print(&mode_label))?;
	used += mode_label.len();

	w.queue(SetBackgroundColor(Color::White))?;
	w.queue(SetForegroundColor(Color::Black))?;

	// File name
	let name = editor.buffer().display_name();
	let dirty = if editor.buffer().dirty { " [+]" } else { "" };
	let file_part = format!(" {}{} ", name, dirty);
	w.queue(style::Print(&file_part))?;
	used += file_part.len();

	// Status message (if any)
	if let Some(ref msg) = editor.status_msg {
		let msg_part = format!(" {} ", msg);
		w.queue(SetBackgroundColor(Color::DarkBlue))?;
		w.queue(SetForegroundColor(Color::Black))?;
		w.queue(style::Print(&msg_part))?;
		w.queue(SetBackgroundColor(Color::White))?;
		w.queue(SetForegroundColor(Color::Black))?;
		used += msg_part.len();
	}

	// Right side: Language + Help toggle + cursor position
	let c = editor.cursors.cursor();
	let mut right_parts = Vec::new();
	
	if editor.config.show_help {
		right_parts.push("^H Help".to_string());
	}
	if editor.config.show_lang {
		let syntax = editor.highlighter.detect_syntax(editor.buffer().file_path.as_deref());
		right_parts.push(syntax.name.clone());
	}
	if editor.config.show_encoding {
		right_parts.push(editor.buffer().encoding.name().to_string());
	}
	right_parts.push(format!("Ln {:2}, Col {:2}", c.line + 1, c.col + 1));

	let right = format!(" {} ", right_parts.join("  "));
	let available = width.saturating_sub(used);
	if available >= right.len() {
		let padding = available - right.len();
		write_spaces(w, padding)?;
		w.queue(style::Print(&right))?;
	} else if available > 0 {
		let truncated: String = right.chars().take(available).collect();
		w.queue(style::Print(&truncated))?;
	}

	w.queue(SetBackgroundColor(Color::Reset))?;
	w.queue(SetForegroundColor(Color::Reset))?;

	Ok(())
}

/// Shortcut definitions for the help bar.
fn help_shortcuts() -> Vec<(&'static str, &'static str)> {
	vec![
		("^S", "Save"),
		("^A", "Save as"),
		("^Q", "Quit"),
		("^Z", "Undo"),
		("^Y", "Redo"),
		("^C", "Copy"),
		("^X", "Cut"),
		("^V", "Paste"),
		("^F", "Find"),
		("^G", "Goto"),
		("^K", "Del ln"),
		("^D", "Dupl"),
		("^W", "Wrap"),
		("^L", "Highl"),
		("^H", "Help"),
	]
}

/// Width of a single shortcut entry: key + " label ".
fn shortcut_width(key: &str, label: &str) -> usize {
	// key displayed, then " label " (space-label-space)
	key.len() + 1 + label.len() + 1
}

/// The width of the " HELP " label prefix.
const HELP_LABEL_WIDTH: usize = 6; // " HELP "

/// Calculate how many rows the help bar needs given a terminal width.
pub fn help_row_count(term_width: u16) -> u16 {
	let shortcuts = help_shortcuts();
	let width = term_width as usize;
	if width == 0 { return 1; }

	// First row starts after the " HELP " label.
	let mut rows: u16 = 1;
	let mut x = HELP_LABEL_WIDTH;

	for (key, label) in &shortcuts {
		let sw = shortcut_width(key, label);
		if x + sw > width && x > HELP_LABEL_WIDTH {
			// Doesn't fit — start a new row.
			rows += 1;
			x = sw;
		} else {
			x += sw;
		}
	}
	rows
}

/// Render the pico-style help bar at the bottom of the screen.
/// Builds upward from the row above the status bar when it needs
/// multiple lines.
pub fn render_help_bar<W: Write>(
	_editor: &Editor,
	w: &mut W,
	vp: &Viewport,
) -> io::Result<()> {
	let shortcuts = help_shortcuts();
	let width = vp.width as usize;
	let num_rows = help_row_count(vp.width) as usize;

	// Layout shortcuts into rows.
	// Each row is a Vec of (key, label) pairs.
	let mut rows: Vec<Vec<(&str, &str)>> = vec![Vec::new()];
	let mut x = HELP_LABEL_WIDTH; // first row starts after " HELP "

	for (key, label) in &shortcuts {
		let sw = shortcut_width(key, label);
		if x + sw > width && x > HELP_LABEL_WIDTH && !rows.last().unwrap().is_empty() {
			rows.push(Vec::new());
			x = sw;
		} else {
			x += sw;
		}
		rows.last_mut().unwrap().push((key, label));
	}

	// The bottom-most help row sits directly above the status bar.
	// Status bar is at vp.height - 1, so the bottom help row is at
	// vp.height - 2, and rows build upward from there.
	let base_y = vp.height.saturating_sub(2);

	for (row_idx, row_items) in rows.iter().enumerate() {
		// Rows render top-to-bottom, row 0 is the topmost.
		let y = base_y.saturating_sub((num_rows - 1 - row_idx) as u16);
		w.queue(cursor::MoveTo(0, y))?;

		let mut used: usize = 0;

		if row_idx == 0 {
			// First row: " HELP " label in yellow (like mode labels)
			w.queue(SetBackgroundColor(Color::DarkYellow))?;
			w.queue(SetForegroundColor(Color::Black))?;
			w.queue(style::Print(" HELP "))?;
			used += HELP_LABEL_WIDTH;
		}

		// Render shortcut items
		for (key, label) in row_items {
			w.queue(SetBackgroundColor(Color::Blue))?;
			w.queue(SetForegroundColor(Color::Black))?;
			w.queue(style::Print(key))?;
			w.queue(SetBackgroundColor(Color::White))?;
			w.queue(SetForegroundColor(Color::Black))?;
			let lbl = format!(" {} ", label);
			w.queue(style::Print(&lbl))?;
			used += key.len() + lbl.len();
		}

		// On the last row, right-align the version string
		if row_idx == rows.len() - 1 {
			let version_str = format!(
				"Dan v{} ({}) ",
				crate::VERSION.trim(),
				crate::GIT_HASH,
			);
			let available = width.saturating_sub(used);
			if available >= version_str.len() {
				let remaining = available - version_str.len();
				if remaining > 0 {
					w.queue(SetBackgroundColor(Color::White))?;
					write_spaces(w, remaining)?;
				}
				w.queue(SetBackgroundColor(Color::White))?;
				w.queue(SetForegroundColor(Color::DarkGrey))?;
				w.queue(style::Print(&version_str))?;
			} else {
				if available > 0 {
					w.queue(SetBackgroundColor(Color::White))?;
					write_spaces(w, available)?;
				}
			}
		} else {
			// Pad remaining width with white background
			let remaining = width.saturating_sub(used);
			if remaining > 0 {
				w.queue(SetBackgroundColor(Color::White))?;
				write_spaces(w, remaining)?;
			}
		}
	}

	w.queue(SetForegroundColor(Color::Reset))?;
	w.queue(SetBackgroundColor(Color::Reset))?;

	Ok(())
}

/// Returns `(rows_needed, cursor_linear_offset)` for the active interactive prompt.
/// `rows_needed` specifies how many terminal rows the prompt occupies.
/// `cursor_linear_offset` is the 1D character position of the interactive cursor.
pub fn prompt_geometry(editor: &Editor, width: u16) -> (u16, u16) {
	if width == 0 {
		return (0, 0);
	}
	let w = width as usize;
	
	match editor.mode {
		Mode::Searching => {
			let label_len = 13; // " Search for: "
			let query_chars = editor.search_query.chars().count();
			let query_display_len = query_chars + 2; // " {} "
			let mut info_len = 0;
			if !editor.search_matches.is_empty() {
				let info = format!(" ({}/{}) ", editor.search_match_idx + 1, editor.search_matches.len());
				info_len = info.chars().count();
			} else if !editor.search_query.is_empty() {
				info_len = 5; // " (0) "
			}
			let total = label_len + query_display_len + info_len;
			let rows = ((total + w - 1) / w) as u16;
			
			// Cursor is at the end of the query (before the trailing space)
			let cursor_offset = (label_len + 1 + query_chars) as u16;
			(rows, cursor_offset)
		}
		Mode::GoToLine => {
			let label_len = 13; // " Go to line: "
			let input_chars = editor.goto_line_input.chars().count();
			let input_display_len = input_chars + 2;
			let hint_len = format!(" (1-{}) ", editor.buffer().line_count()).chars().count();
			let total = label_len + input_display_len + hint_len;
			let rows = ((total + w - 1) / w) as u16;
			
			let cursor_offset = (label_len + 1 + input_chars) as u16;
			(rows, cursor_offset)
		}
		Mode::SaveAs | Mode::ConfirmOverwrite => {
			let label_len = 10; // " Save As: "
			let input_chars = editor.save_as_input.chars().count();
			let input_display_len = input_chars + 2;
			let total = label_len + input_display_len;
			let rows = ((total + w - 1) / w) as u16;
			
			let cursor_offset = (label_len + 1 + editor.save_as_cursor) as u16;
			(rows, cursor_offset)
		}
		_ => (0, 0)
	}
}

/// Render the search prompt bar (appears below the status bar).
pub fn render_search_bar<W: Write>(
	editor: &Editor,
	w: &mut W,
	vp: &Viewport,
) -> io::Result<()> {
	let (rows, _) = prompt_geometry(editor, vp.width);
	if rows == 0 { return Ok(()); }
	let search_y = vp.height.saturating_sub(1 + rows);
	w.queue(cursor::MoveTo(0, search_y))?;

	let width = vp.width as usize;
	let mut used: usize = 0;

	// Label
	w.queue(SetBackgroundColor(Color::DarkYellow))?;
	w.queue(SetForegroundColor(Color::Black))?;
	let label = " Search for: ";
	w.queue(style::Print(label))?;
	used += label.chars().count();

	// Query text
	w.queue(SetBackgroundColor(Color::DarkGrey))?;
	w.queue(SetForegroundColor(Color::White))?;
	let query_display = format!(" {} ", editor.search_query);
	w.queue(style::Print(&query_display))?;
	used += query_display.chars().count();

	// Match count
	let info = if editor.search_matches.is_empty() {
		if editor.search_query.is_empty() {
			String::new()
		} else {
			" (0) ".to_string()
		}
	} else {
		format!(
			" ({}/{}) ",
			editor.search_match_idx + 1,
			editor.search_matches.len()
		)
	};
	if !info.is_empty() {
		w.queue(SetForegroundColor(Color::White))?;
		w.queue(style::Print(&info))?;
		used += info.chars().count();
	}

	// Pad the rest of the multi-line block
	let total_cells = (rows as usize) * width;
	let remaining = total_cells.saturating_sub(used);
	if remaining > 0 {
		w.queue(SetBackgroundColor(Color::DarkGrey))?;
		write_spaces(w, remaining)?;
	}

	w.queue(SetBackgroundColor(Color::Reset))?;
	w.queue(SetForegroundColor(Color::Reset))?;

	Ok(())
}

/// Render the go-to-line prompt bar (appears below the status bar).
pub fn render_goto_line_bar<W: Write>(
	editor: &Editor,
	w: &mut W,
	vp: &Viewport,
) -> io::Result<()> {
	let (rows, _) = prompt_geometry(editor, vp.width);
	if rows == 0 { return Ok(()); }
	let bar_y = vp.height.saturating_sub(1 + rows);
	w.queue(cursor::MoveTo(0, bar_y))?;

	let width = vp.width as usize;
	let mut used: usize = 0;

	// Label
	w.queue(SetBackgroundColor(Color::DarkCyan))?;
	w.queue(SetForegroundColor(Color::Black))?;
	let label = " Go to line: ";
	w.queue(style::Print(label))?;
	used += label.chars().count();

	// Line number input
	w.queue(SetBackgroundColor(Color::DarkGrey))?;
	w.queue(SetForegroundColor(Color::White))?;
	let input_display = format!(" {} ", editor.goto_line_input);
	w.queue(style::Print(&input_display))?;
	used += input_display.chars().count();

	// Hint
	let total_lines = editor.buffer().line_count();
	let hint = format!(" (1-{}) ", total_lines);
	w.queue(SetForegroundColor(Color::Grey))?;
	w.queue(style::Print(&hint))?;
	used += hint.chars().count();

	// Pad the rest of the multi-line block
	let total_cells = (rows as usize) * width;
	let remaining = total_cells.saturating_sub(used);
	if remaining > 0 {
		w.queue(SetBackgroundColor(Color::DarkGrey))?;
		write_spaces(w, remaining)?;
	}

	w.queue(SetBackgroundColor(Color::Reset))?;
	w.queue(SetForegroundColor(Color::Reset))?;

	Ok(())
}

/// Render the save-as prompt bar (appears below the status bar).
pub fn render_save_as_bar<W: Write>(
	editor: &Editor,
	w: &mut W,
	vp: &Viewport,
) -> io::Result<()> {
	let (rows, _) = prompt_geometry(editor, vp.width);
	if rows == 0 { return Ok(()); }
	let bar_y = vp.height.saturating_sub(1 + rows);
	w.queue(cursor::MoveTo(0, bar_y))?;

	let width = vp.width as usize;
	let mut used: usize = 0;

	// Label
	w.queue(SetBackgroundColor(Color::DarkGreen))?;
	w.queue(SetForegroundColor(Color::Black))?;
	let label = " Save as: ";
	w.queue(style::Print(label))?;
	used += label.len();

	// Path input
	w.queue(SetBackgroundColor(Color::DarkGrey))?;
	w.queue(SetForegroundColor(Color::White))?;
	let input_display = format!(" {} ", editor.save_as_input);
	w.queue(style::Print(&input_display))?;
	used += input_display.len();

	// Hint
	// w.queue(SetForegroundColor(Color::Grey))?;
	// let hint = " (Enter=save, Esc=cancel) ";
	// w.queue(style::Print(hint))?;
	// used += hint.len();

	// Pad the rest of the multi-line block
	let total_cells = (rows as usize) * width;
	let remaining = total_cells.saturating_sub(used);
	if remaining > 0 {
		w.queue(SetBackgroundColor(Color::DarkGrey))?;
		write_spaces(w, remaining)?;
	}

	w.queue(SetBackgroundColor(Color::Reset))?;
	w.queue(SetForegroundColor(Color::Reset))?;

	Ok(())
}

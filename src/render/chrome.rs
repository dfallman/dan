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
		w.queue(SetBackgroundColor(Color::Blue))?;
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
		("^R", "Replace"),
		("^G", "Go to"),
		("^D", "Duplicate"),
		("^K", "Delete"),
		("^W", "Wrap text"),
		("^L", "Lint"),
		("^E", "Comment"),
		("^T", "Syntax highl"),
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
				w.queue(SetForegroundColor(Color::Black))?;
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

pub struct PromptBlock {
	pub bg: Color,
	pub fg: Color,
	pub text: String,
}

pub struct PromptLayout {
	pub rows: u16,
	pub cursor_offset: u16,
	pub blocks: Vec<PromptBlock>,
}

/// Dynamically format and dimension the interactive overlay component natively supporting bounds.
pub fn build_prompt(editor: &Editor, width: u16) -> Option<PromptLayout> {
	if width == 0 { return None; }
	let w = width as usize;

	match editor.mode {
		Mode::ReplacingSearch => {
			let label = " Replace find: ".to_string();
			let query_display = format!(" {} ", editor.replace_query);
			let info = if editor.search_matches.is_empty() {
				if editor.replace_query.is_empty() { String::new() } else { " (0) ".to_string() }
			} else {
				format!(" ({}/{}) ", editor.search_match_idx + 1, editor.search_matches.len())
			};

			let total = label.chars().count() + query_display.chars().count() + info.chars().count();
			let rows = ((total + w - 1) / w) as u16;
			let cursor_offset = (label.chars().count() + 1 + editor.replace_query.chars().count()) as u16;

			let mut blocks = vec![
				PromptBlock { bg: Color::DarkMagenta, fg: Color::Black, text: label },
				PromptBlock { bg: Color::DarkGrey, fg: Color::White, text: query_display },
			];
			if !info.is_empty() {
				blocks.push(PromptBlock { bg: Color::DarkGrey, fg: Color::White, text: info });
			}
			Some(PromptLayout { rows, cursor_offset, blocks })
		}
		Mode::ReplacingWith => {
			let label = " Replace with: ".to_string();
			let query_display = format!(" {} ", editor.replace_with);

			let total = label.chars().count() + query_display.chars().count();
			let rows = ((total + w - 1) / w) as u16;
			let cursor_offset = (label.chars().count() + 1 + editor.replace_with.chars().count()) as u16;

			let blocks = vec![
				PromptBlock { bg: Color::DarkMagenta, fg: Color::Black, text: label },
				PromptBlock { bg: Color::DarkGrey, fg: Color::White, text: query_display },
			];
			Some(PromptLayout { rows, cursor_offset, blocks })
		}
		Mode::ReplacingStep => {
			let label = " Replace? (y)es, (n)o, (a)ll, (q)uit: ".to_string();
			let rows = ((label.chars().count() + w - 1) / w) as u16;

			let blocks = vec![
				PromptBlock { bg: Color::DarkMagenta, fg: Color::Black, text: label },
			];
			Some(PromptLayout { rows, cursor_offset: 0, blocks })
		}
		Mode::Searching => {
			let label = " Search for: ".to_string();
			let query_display = format!(" {} ", editor.search_query);
			let info = if editor.search_matches.is_empty() {
				if editor.search_query.is_empty() { String::new() } else { " (0) ".to_string() }
			} else {
				format!(" ({}/{}) ", editor.search_match_idx + 1, editor.search_matches.len())
			};

			let total = label.chars().count() + query_display.chars().count() + info.chars().count();
			let rows = ((total + w - 1) / w) as u16;
			let cursor_offset = (label.chars().count() + 1 + editor.search_query.chars().count()) as u16;

			let mut blocks = vec![
				PromptBlock { bg: Color::DarkYellow, fg: Color::Black, text: label },
				PromptBlock { bg: Color::DarkGrey, fg: Color::White, text: query_display },
			];
			if !info.is_empty() {
				blocks.push(PromptBlock { bg: Color::DarkGrey, fg: Color::White, text: info });
			}
			Some(PromptLayout { rows, cursor_offset, blocks })
		}
		Mode::GoToLine => {
			let label = " Go to line: ".to_string();
			let input_display = format!(" {} ", editor.goto_line_input);
			let total_lines = editor.buffer().line_count();
			let hint = format!(" (1-{}) ", total_lines);

			let total = label.chars().count() + input_display.chars().count() + hint.chars().count();
			let rows = ((total + w - 1) / w) as u16;
			let cursor_offset = (label.chars().count() + 1 + editor.goto_line_input.chars().count()) as u16;

			let blocks = vec![
				PromptBlock { bg: Color::DarkCyan, fg: Color::Black, text: label },
				PromptBlock { bg: Color::DarkGrey, fg: Color::White, text: input_display },
				PromptBlock { bg: Color::DarkGrey, fg: Color::Grey, text: hint },
			];
			Some(PromptLayout { rows, cursor_offset, blocks })
		}
		Mode::RecoverSwap => {
			let label = " RECOVERY ".to_string();
			let msg = " Swap file detected! Restore unsaved changes? (y)es, (n)o ".to_string();
			let total = label.chars().count() + msg.chars().count();
			let rows = ((total + w - 1) / w) as u16;
			let blocks = vec![
				PromptBlock { bg: Color::DarkRed, fg: Color::Black, text: label },
				PromptBlock { bg: Color::DarkYellow, fg: Color::Black, text: msg },
			];
			Some(PromptLayout { rows, cursor_offset: 0, blocks })
		}
		Mode::SaveAs | Mode::ConfirmOverwrite => {
			let label = " Save as: ".to_string();
			let input_display = format!(" {} ", editor.save_as_input);

			let total = label.chars().count() + input_display.chars().count();
			let rows = ((total + w - 1) / w) as u16;
			let cursor_offset = (label.chars().count() + 1 + editor.save_as_cursor) as u16;

			let blocks = vec![
				PromptBlock { bg: Color::DarkGreen, fg: Color::Black, text: label },
				PromptBlock { bg: Color::DarkGrey, fg: Color::White, text: input_display },
			];
			Some(PromptLayout { rows, cursor_offset, blocks })
		}
		Mode::ConfirmQuit => {
			let label1 = " Quit warning: ".to_string();
			let label2 = " Unsaved changes! (s)ave and quit, (f)orce quit, Esc to cancel ".to_string();
			
			let total = label1.chars().count() + label2.chars().count();
			let rows = ((total + w - 1) / w) as u16;

			let blocks = vec![
				PromptBlock { bg: Color::DarkRed, fg: Color::Black, text: label1 },
				PromptBlock { bg: Color::Blue, fg: Color::Black, text: label2 },
			];
			Some(PromptLayout { rows, cursor_offset: 0, blocks })
		}
		_ => None
	}
}

/// Uniform renderer interpreting constructed runtime blocks natively.
pub fn render_prompt_overlay<W: Write>(
	w: &mut W,
	vp: &Viewport,
	layout: &PromptLayout,
) -> io::Result<()> {
	if layout.rows == 0 { return Ok(()); }
	let bar_y = vp.height.saturating_sub(1 + layout.rows);
	w.queue(cursor::MoveTo(0, bar_y))?;

	let mut used: usize = 0;

	for block in &layout.blocks {
		w.queue(SetBackgroundColor(block.bg))?;
		w.queue(SetForegroundColor(block.fg))?;
		w.queue(style::Print(&block.text))?;
		used += block.text.chars().count();
	}

	let total_cells = (layout.rows as usize) * (vp.width as usize);
	let remaining = total_cells.saturating_sub(used);
	if remaining > 0 {
		w.queue(SetBackgroundColor(Color::DarkGrey))?;
		write_spaces(w, remaining)?;
	}

	w.queue(SetBackgroundColor(Color::Reset))?;
	w.queue(SetForegroundColor(Color::Reset))?;
	Ok(())
}

mod chrome;
mod text;

use crossterm::{
	cursor,
	style::{self, Color, SetBackgroundColor, SetForegroundColor},
	terminal,
	QueueableCommand,
};
use std::io::{self, Write};

use crate::editor::Editor;
use crate::editor::mode::Mode;
use crate::editor::visual_rows_for;
use crate::utils::char_width;

/// Viewport dimensions, cached from the Editor.
pub struct Viewport {
	pub width: u16,
	pub height: u16,
	/// Fixed chrome rows (status bar = 1).
	pub chrome_rows: u16,
	/// Overlay bars that paint over the bottom text lines (help, search, goto).
	pub overlay_rows: u16,
}

impl Viewport {
	/// Query the actual terminal size and sync the editor's cached values.
	pub fn from_editor(editor: &mut Editor) -> Self {
		let (w, h) = terminal::size().unwrap_or((editor.terminal_width, editor.terminal_height));
		editor.terminal_width = w;
		editor.terminal_height = h;
		// Status bar is always 1 row — it never changes.
		let chrome: u16 = 1;
		let has_prompt = matches!(
			editor.mode,
			Mode::Searching | Mode::GoToLine | Mode::SaveAs | Mode::ConfirmOverwrite
		);
		let mut overlay: u16 = 0;

		// Automatically hide help if a prompt is showing above the toolbar
		if editor.show_help && !has_prompt {
			overlay += chrome::help_row_count(w);
		}
		if has_prompt {
			let (rows, _) = chrome::prompt_geometry(editor, w);
			overlay += rows;
		}
		Self {
			width: w,
			height: h,
			chrome_rows: chrome,
			overlay_rows: overlay,
		}
	}

	/// Height available for text (total height minus the fixed status bar).
	pub fn text_height(&self) -> u16 {
		self.height.saturating_sub(self.chrome_rows)
	}

	/// Effective visible text height: text rows not covered by overlays.
	/// Used for scroll clamping so the cursor stays above overlay bars.
	pub fn visible_text_height(&self) -> u16 {
		self.text_height().saturating_sub(self.overlay_rows)
	}
}

/// A reusable buffer of spaces for padding lines — avoids allocating
/// a new `String` every time we need to pad.
const PAD_CHUNK: &str = "                                                                                                                                                                                                                                                                ";

/// Write `n` space characters by repeatedly printing from PAD_CHUNK.
fn write_spaces<W: Write>(w: &mut W, n: usize) -> io::Result<()> {
	let mut remaining = n;
	while remaining > 0 {
		let chunk = remaining.min(PAD_CHUNK.len());
		w.queue(style::Print(&PAD_CHUNK[..chunk]))?;
		remaining -= chunk;
	}
	Ok(())
}

/// Calculate the width needed for line numbers.
fn line_number_width(total_lines: usize) -> usize {
	if total_lines == 0 {
		1
	} else {
		(total_lines as f64).log10().floor() as usize + 1
	}
}

/// Render the full editor frame to the terminal.
pub fn render<W: Write>(editor: &mut Editor, w: &mut W) -> io::Result<()> {
	let vp = Viewport::from_editor(editor);
	let text_height = vp.text_height() as usize;

	// Adjust scroll to keep cursor visible (with scroll_off padding)
	let cursor_line = editor.cursors.cursor().line;
	let scroll_off = if vp.height <= 20 { 0 } else { editor.config.scroll_off };
	if editor.config.wrap_lines {
		// Wrap mode: scroll must account for visual rows, not just buffer lines.
		// Re-derive text_area_width for the helper (gutter not computed yet, use temp)
		let line_count_tmp = editor.buffer().line_count();
		let gw_tmp = if editor.config.line_numbers {
			line_number_width(line_count_tmp)
		} else { 0 };
		let taw_tmp = (vp.width as usize).saturating_sub(gw_tmp + 1);
		let tab_w = editor.config.tab_width;
		if taw_tmp > 0 {
			// Find which visual row the cursor is on within its buffer line.
			let cur_text: String = editor.buffer().text.line_slice(cursor_line).chars().collect();
			let cur_vrows = visual_rows_for(&cur_text, tab_w, taw_tmp);
			let cursor_col = editor.cursors.cursor().col;
			let mut cur_vrow_idx = cur_vrows.len() - 1;
			for (i, &(start, end)) in cur_vrows.iter().enumerate() {
				if cursor_col >= start && (cursor_col < end || i == cur_vrows.len() - 1) {
					cur_vrow_idx = i;
					break;
				}
			}

			// --- Scroll UP: ensure scroll_off visual rows above the cursor ---
			// First, clamp scroll_y so it never goes past the cursor line.
			if editor.scroll_y > cursor_line {
				editor.scroll_y = cursor_line;
			}
			// Count visual rows from scroll_y to the cursor's visual row.
			// If it's less than scroll_off, scroll up.
			loop {
				if editor.scroll_y == 0 { break; }
				let mut rows_above: usize = 0;
				for bl in editor.scroll_y..cursor_line {
					let lt: String = editor.buffer().text.line_slice(bl).chars().collect();
					rows_above += visual_rows_for(&lt, tab_w, taw_tmp).len();
				}
				rows_above += cur_vrow_idx; // cursor's sub-row within its line
				if rows_above >= scroll_off { break; }
				editor.scroll_y -= 1;
			}

			// --- Scroll DOWN: ensure scroll_off visual rows below the cursor ---
			// The cursor's visual row (from top of viewport) must be at most
			// visible_height - 1 - scroll_off (so it stays above any overlay bars).
			let visible_height = vp.visible_text_height() as usize;
			let max_row = visible_height.saturating_sub(1 + scroll_off);
			loop {
				let mut vrow_from_top: usize = 0;
				for bl in editor.scroll_y..cursor_line {
					let lt: String = editor.buffer().text.line_slice(bl).chars().collect();
					vrow_from_top += visual_rows_for(&lt, tab_w, taw_tmp).len();
				}
				vrow_from_top += cur_vrow_idx;
				if vrow_from_top <= max_row {
					break;
				}
				editor.scroll_y += 1;
				if editor.scroll_y > cursor_line {
					editor.scroll_y = cursor_line;
					break;
				}
			}
		}
	} else {
		let visible_height = vp.visible_text_height() as usize;
		if cursor_line < editor.scroll_y + scroll_off {
			editor.scroll_y = cursor_line.saturating_sub(scroll_off);
		}
		if cursor_line + scroll_off >= editor.scroll_y + visible_height {
			editor.scroll_y = (cursor_line + scroll_off).saturating_sub(visible_height) + 1;
		}
	}

	// -- Horizontal scroll adjustment (only when wrap_lines = false) --
	let line_count = editor.buffer().line_count();
	let show_line_numbers = editor.config.line_numbers;
	let gutter_width = if show_line_numbers {
		line_number_width(line_count)
	} else {
		0
	};
	let text_area_width = (vp.width as usize).saturating_sub(gutter_width + 1);

	if !editor.config.wrap_lines {
		// Compute the cursor's visual column so we can center scroll_x on it.
		let cursor_pos = editor.cursors.cursor();
		let tab_w = editor.config.tab_width;
		let cursor_vcol = if cursor_pos.line < line_count {
			let lsl = editor.buffer().text.line_slice(cursor_pos.line);
			let mut vc: usize = 0;
			for (i, ch) in lsl.chars().enumerate() {
				if i >= cursor_pos.col { break; }
				if ch == '\t' { vc += tab_w - (vc % tab_w); }
				else { vc += char_width(ch, tab_w); }
			}
			vc
		} else {
			cursor_pos.col
		};
		let h_margin: usize = 5;
		if cursor_vcol < editor.scroll_x + h_margin {
			editor.scroll_x = cursor_vcol.saturating_sub(h_margin);
		}
		if cursor_vcol >= editor.scroll_x + text_area_width.saturating_sub(h_margin) {
			editor.scroll_x = cursor_vcol.saturating_sub(text_area_width.saturating_sub(h_margin + 1));
		}
	} else {
		editor.scroll_x = 0;
	}

	w.queue(cursor::Hide)?;
	w.queue(cursor::MoveTo(0, 0))?;

	// -- Render text lines --
	// Get selection range for highlighting
	let sel_range = editor.selection_range();
	let highlight_active = editor.config.highlight_active;

	if editor.config.wrap_lines {
		text::render_wrap(
			editor, w, &vp, text_height, gutter_width,
			show_line_numbers, text_area_width, sel_range,
			highlight_active, cursor_line,
		)?;
	} else {
		text::render_nowrap(
			editor, w, &vp, text_height, gutter_width,
			show_line_numbers, text_area_width, sel_range,
			highlight_active, cursor_line,
		)?;
	}

	// -- Render status bar (always the row just after text) --
	chrome::render_status_bar(editor, w, &vp)?;

	// -- Render help bar (only when toggled on & no prompt active) --
	let has_prompt = matches!(
		editor.mode,
		Mode::Searching | Mode::GoToLine | Mode::SaveAs | Mode::ConfirmOverwrite
	);
	if editor.show_help && !has_prompt {
		chrome::render_help_bar(editor, w, &vp)?;
	}

	// -- Render search prompt (when in search mode) --
	if editor.mode == Mode::Searching {
		chrome::render_search_bar(editor, w, &vp)?;
	}

	// -- Render go-to-line prompt (when in goto-line mode) --
	if editor.mode == Mode::GoToLine {
		chrome::render_goto_line_bar(editor, w, &vp)?;
	}

	// -- Render save-as prompt (when in save-as or confirm-overwrite mode) --
	if editor.mode == Mode::SaveAs || editor.mode == Mode::ConfirmOverwrite {
		chrome::render_save_as_bar(editor, w, &vp)?;
	}

	// -- Position the cursor --
	if editor.mode == Mode::Searching {
		// During search, draw an outline cursor in the document at the saved position.
		if let Some((saved_line, saved_col)) = editor.search_saved_cursor {
			if saved_line >= editor.scroll_y && saved_line < editor.scroll_y + text_height {
				let saved_screen_y = (saved_line - editor.scroll_y) as u16;
				let tab_w = editor.config.tab_width;
				let saved_visual_col = if saved_line < line_count {
					let line_slice = editor.buffer().text.line_slice(saved_line);
					let mut vc: usize = 0;
					for (i, ch) in line_slice.chars().enumerate() {
						if i >= saved_col {
							break;
						}
						if ch == '\t' {
							vc += tab_w - (vc % tab_w);
						} else {
							vc += char_width(ch, tab_w);
						}
					}
					vc
				} else {
					saved_col
				};
				let outline_x = (gutter_width + 1 + saved_visual_col.saturating_sub(editor.scroll_x)) as u16;
				// Draw the character (or space) at that position with an underline-style outline.
				w.queue(cursor::MoveTo(outline_x, saved_screen_y))?;
				w.queue(SetBackgroundColor(Color::DarkGrey))?;
				w.queue(SetForegroundColor(Color::White))?;
				w.queue(style::SetAttribute(style::Attribute::Underlined))?;
				// Print the actual character at the cursor position, or a space if past EOL.
				let outline_ch = if saved_line < line_count {
					let line_slice = editor.buffer().text.line_slice(saved_line);
					line_slice.chars().nth(saved_col)
						.filter(|c| *c != '\n' && *c != '\r')
						.unwrap_or(' ')
				} else {
					' '
				};
				if outline_ch == '\t' {
					w.queue(style::Print(" "))?;
				} else {
					w.queue(style::Print(format!("{}", outline_ch)))?;
				}
				w.queue(style::SetAttribute(style::Attribute::NoUnderline))?;
				w.queue(SetBackgroundColor(Color::Reset))?;
				w.queue(SetForegroundColor(Color::Reset))?;
			}
		}
	}

	let has_prompt = matches!(
		editor.mode,
		Mode::Searching | Mode::GoToLine | Mode::SaveAs | Mode::ConfirmOverwrite
	);

	if has_prompt {
		let (rows, offset) = chrome::prompt_geometry(editor, vp.width);
		if rows > 0 {
			let prompt_y = vp.height.saturating_sub(1 + rows);
			let cursor_x = offset % vp.width;
			let cursor_y = prompt_y + (offset / vp.width);
			w.queue(cursor::MoveTo(cursor_x, cursor_y))?;
			w.queue(cursor::Show)?;
			w.queue(cursor::SetCursorStyle::BlinkingBlock)?;
		}
	} else {
		// Normal mode — position cursor in the document.
		let cursor_pos = editor.cursors.cursor();
		let tab_w = editor.config.tab_width;

		let (screen_y, visual_col) = if editor.config.wrap_lines && text_area_width > 0 {
			// Wrap mode: screen_y must count visual rows, visual_col is
			// relative to the start of the cursor's visual row.
			let mut sy: usize = 0;
			for bl in editor.scroll_y..cursor_pos.line.min(line_count) {
				let lt: String = editor.buffer().text.line_slice(bl).chars().collect();
				sy += visual_rows_for(&lt, tab_w, text_area_width).len();
			}
			// Find cursor's visual row within its buffer line.
			let (vrow_idx, vrow_start) = if cursor_pos.line < line_count {
				let lt: String = editor.buffer().text.line_slice(cursor_pos.line).chars().collect();
				let vrows = visual_rows_for(&lt, tab_w, text_area_width);
				let mut idx = vrows.len() - 1;
				for (i, &(start, end)) in vrows.iter().enumerate() {
					if cursor_pos.col >= start && (cursor_pos.col < end || i == vrows.len() - 1) {
						idx = i;
						break;
					}
				}
				(idx, vrows[idx].0)
			} else {
				(0, 0)
			};
			sy += vrow_idx;

			// Compute visual column from the visual row's start char.
			let vc = if cursor_pos.line < line_count {
				let line_slice = editor.buffer().text.line_slice(cursor_pos.line);
				let mut v: usize = 0;
				for (i, ch) in line_slice.chars().enumerate() {
					if i < vrow_start { continue; }
					if i >= cursor_pos.col { break; }
					if ch == '\t' { v += tab_w - (v % tab_w); }
					else { v += char_width(ch, tab_w); }
				}
				v
			} else {
				cursor_pos.col
			};
			(sy, vc)
		} else {
			// No-wrap mode: 1 buffer line = 1 screen row.
			let sy = cursor_pos.line.saturating_sub(editor.scroll_y);
			let vc = if cursor_pos.line < line_count {
				let line_slice = editor.buffer().text.line_slice(cursor_pos.line);
				let mut v: usize = 0;
				for (i, ch) in line_slice.chars().enumerate() {
					if i >= cursor_pos.col { break; }
					if ch == '\t' { v += tab_w - (v % tab_w); }
					else { v += char_width(ch, tab_w); }
				}
				v
			} else {
				cursor_pos.col
			};
			(sy, vc)
		};
		let screen_x = (gutter_width + 1 + visual_col.saturating_sub(editor.scroll_x)) as u16;
		w.queue(cursor::MoveTo(screen_x, screen_y as u16))?;
		w.queue(cursor::Show)?;

		// Pico-style: always use a steady block cursor (like a normal text editor)
		w.queue(cursor::SetCursorStyle::SteadyBlock)?;
	}

	w.flush()?;
	Ok(())
}

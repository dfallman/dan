use crossterm::{
	cursor,
	style::{self, Color, SetBackgroundColor, SetForegroundColor},
	QueueableCommand,
};
use std::io::{self, Write};

use syntect::easy::HighlightLines;
use syntect::highlighting::FontStyle;

use crate::editor::Editor;
use crate::utils::char_width;
use super::{write_spaces, Viewport};

/// Convert a syntect RGBA color to a crossterm Color.
fn syntect_to_crossterm(c: syntect::highlighting::Color) -> Color {
	Color::Rgb {
		r: c.r,
		g: c.g,
		b: c.b,
	}
}

/// Build a per-char foreground color map for one buffer line using syntect.
///
/// Returns a Vec with one `Color` per character in `line_text` (excluding
/// trailing newlines). If highlighting is disabled, returns an empty Vec.
fn syntax_colors_for_line(
	editor: &Editor,
	hi: &mut HighlightLines<'_>,
	line_text: &str,
) -> Vec<(Color, bool)> {
	if !editor.config.syntax_highlighting {
		return Vec::new();
	}
	let ranges = hi
		.highlight_line(line_text, &editor.highlighter.syntax_set)
		.unwrap_or_default();

	let mut colors: Vec<(Color, bool)> = Vec::with_capacity(line_text.len());
	for (style, fragment) in &ranges {
		let fg = syntect_to_crossterm(style.foreground);
		let bold = style.font_style.contains(FontStyle::BOLD);
		for _ in fragment.chars() {
			colors.push((fg, bold));
		}
	}
	colors
}

/// Look up the syntax color for a character at `char_idx`.
/// Falls back to `Color::Reset` if the index is out of range or the map is empty.
#[inline]
fn syntax_fg(colors: &[(Color, bool)], char_idx: usize) -> Color {
	colors
		.get(char_idx)
		.map(|(c, _)| *c)
		.unwrap_or(Color::Reset)
}

/// Render text lines in wrap mode (soft-wrap).
///
/// Each buffer line may occupy multiple screen rows.
pub fn render_wrap<W: Write>(
	editor: &Editor,
	w: &mut W,
	vp: &Viewport,
	text_height: usize,
	gutter_width: usize,
	show_line_numbers: bool,
	text_area_width: usize,
	sel_range: Option<(usize, usize)>,
	highlight_active: bool,
	cursor_line: usize,
) -> io::Result<()> {
	let line_count = editor.buffer().line_count();
	let mut screen_row: usize = 0;
	let mut buf_line = editor.scroll_y;

	// Set up syntect highlighter for the buffer's detected syntax.
	let syntax = editor
		.highlighter
		.detect_syntax(editor.buffer().file_path.as_deref());
	let mut hi = HighlightLines::new(syntax, &editor.highlighter.theme);

	// We need to feed syntect all lines from the top of the file up to the
	// first visible line so the parse state is correct (syntect is stateful).
	// For moderate files this is fast; for very large files a cached parse
	// state would be better (future optimisation).
	for pre_line in 0..editor.scroll_y.min(line_count) {
		let pre_text = editor.buffer().text.line(pre_line);
		let _ = hi.highlight_line(&pre_text, &editor.highlighter.syntax_set);
	}

	while screen_row < text_height && buf_line < line_count {
		let is_active = highlight_active && buf_line == cursor_line;
		let base_bg = if is_active {
			Color::AnsiValue(236)
		} else {
			Color::Reset
		};

		// -- First screen row of this buffer line: draw the real line number --
		w.queue(cursor::MoveTo(0, screen_row as u16))?;
		w.queue(SetForegroundColor(Color::Reset))?;
		w.queue(SetBackgroundColor(base_bg))?;

		if show_line_numbers {
			let line_num = format!("{:>width$} ", buf_line + 1, width = gutter_width);
			if buf_line == cursor_line {
				w.queue(SetForegroundColor(Color::White))?;
			} else {
				w.queue(SetForegroundColor(Color::DarkGrey))?;
			}
			w.queue(style::Print(&line_num))?;
			w.queue(SetForegroundColor(Color::Reset))?;
		}

		let line_text = editor.buffer().text.line(buf_line);
		let line_start_pos = editor.buffer().text.line_to_char(buf_line);
		let tab_w = editor.config.tab_width;

		// Get per-char syntax colors for this line.
		let syn_colors = syntax_colors_for_line(editor, &mut hi, &line_text);

		let mut batch = String::new();
		let mut in_sel = false;
		let mut prev_in_search = false;
		let mut prev_is_current = false;
		let mut prev_syn_fg = Color::Reset;
		let mut vcol: usize = 0;
		let mut char_idx: usize = 0;
		let mut screen_col: usize = 0; // columns written on current screen row

		for ch in line_text.chars() {
			if ch == '\n' || ch == '\r' {
				char_idx += 1;
				continue;
			}

			// Compute the width this char will take
			let ch_w = if ch == '\t' { tab_w - (vcol % tab_w) } else { char_width(ch, tab_w) };

			// If this char would overflow the current screen row, wrap.
			if screen_col + ch_w > text_area_width {
				// Flush batch
				if !batch.is_empty() {
					w.queue(style::Print(&batch))?;
					batch.clear();
				}
				if in_sel || prev_in_search {
					w.queue(SetBackgroundColor(base_bg))?;
					w.queue(SetForegroundColor(Color::Reset))?;
				}
				// Pad rest of this row
				let remaining = text_area_width.saturating_sub(screen_col);
				if remaining > 0 {
					write_spaces(w, remaining)?;
				}
				screen_row += 1;
				if screen_row >= text_height { break; }

				// Start new screen row — continuation with ↳ gutter
				w.queue(cursor::MoveTo(0, screen_row as u16))?;
				w.queue(SetForegroundColor(Color::Reset))?;
				w.queue(SetBackgroundColor(base_bg))?;
				if show_line_numbers {
					// Right-align ↳ within the gutter width, followed by separator space
					let wrap_gutter = format!("{:>width$} ", "↳", width = gutter_width);
					if buf_line == cursor_line {
						w.queue(SetForegroundColor(Color::Blue))?;
					} else {
						w.queue(SetForegroundColor(Color::DarkGrey))?;
					}
					w.queue(style::Print(&wrap_gutter))?;
					w.queue(SetForegroundColor(Color::Reset))?;
				}
				screen_col = 0;
				in_sel = false;
				prev_in_search = false;
				prev_is_current = false;
				prev_syn_fg = Color::Reset;
			}

			// Highlight logic
			let char_pos = line_start_pos + char_idx;
			let want_sel = if let Some((sel_start, sel_end)) = sel_range {
				char_pos >= sel_start && char_pos < sel_end
			} else {
				false
			};
			let search_hit = editor.search_matches.iter().enumerate().find(
				|(_i, &(ms, me))| char_pos >= ms && char_pos < me,
			);
			let is_current_match = search_hit.as_ref().map(|(i, _)| *i == editor.search_match_idx).unwrap_or(false);
			let in_search = search_hit.is_some();

			// Determine the fg color for this char: selection/search override syntax.
			let cur_syn_fg = syntax_fg(&syn_colors, char_idx);
			let want_state = (want_sel, in_search, is_current_match, if want_sel || in_search { Color::Reset } else { cur_syn_fg });
			let prev_state = (in_sel, prev_in_search, prev_is_current, prev_syn_fg);
			if want_state != prev_state {
				if !batch.is_empty() {
					w.queue(style::Print(&batch))?;
					batch.clear();
				}
				if want_sel {
					w.queue(SetBackgroundColor(Color::DarkBlue))?;
					w.queue(SetForegroundColor(Color::White))?;
				} else if is_current_match {
					w.queue(SetBackgroundColor(Color::Green))?;
					w.queue(SetForegroundColor(Color::Black))?;
				} else if in_search {
					w.queue(SetBackgroundColor(Color::Yellow))?;
					w.queue(SetForegroundColor(Color::Black))?;
				} else {
					w.queue(SetBackgroundColor(base_bg))?;
					w.queue(SetForegroundColor(cur_syn_fg))?;
				}
				in_sel = want_sel;
				prev_in_search = in_search;
				prev_is_current = is_current_match;
				prev_syn_fg = if want_sel || in_search { Color::Reset } else { cur_syn_fg };
			}

			if ch == '\t' {
				let spaces = ch_w;
				for _ in 0..spaces {
					batch.push(' ');
				}
			} else {
				batch.push(ch);
			}
			vcol += ch_w;
			screen_col += ch_w;
			char_idx += 1;
		}

		// Flush remaining batch for this buffer line
		if !batch.is_empty() {
			w.queue(style::Print(&batch))?;
		}
		if in_sel || prev_in_search {
			w.queue(SetBackgroundColor(base_bg))?;
			w.queue(SetForegroundColor(Color::Reset))?;
		}

		// Pad rest of the last screen row for this buffer line
		let cols_used = gutter_width + 1 + screen_col;
		let remaining = (vp.width as usize).saturating_sub(cols_used);
		if remaining > 0 {
			w.queue(SetForegroundColor(Color::Reset))?;
			write_spaces(w, remaining)?;
		}
		// Reset bg after active-line padding so it doesn't bleed
		if is_active {
			w.queue(SetBackgroundColor(Color::Reset))?;
		}

		screen_row += 1;
		buf_line += 1;
	}

	// Fill remaining screen rows past EOF with tilde gutters
	while screen_row < text_height {
		w.queue(cursor::MoveTo(0, screen_row as u16))?;
		w.queue(SetForegroundColor(Color::Reset))?;
		w.queue(SetBackgroundColor(Color::Reset))?;
		let mut cols_written: usize = 0;
		if show_line_numbers {
			let tilde_gutter = format!("{:>width$} ", "~", width = gutter_width);
			w.queue(SetForegroundColor(Color::DarkGrey))?;
			w.queue(style::Print(&tilde_gutter))?;
			w.queue(SetForegroundColor(Color::Reset))?;
			cols_written = gutter_width + 1;
		}
		let remaining = (vp.width as usize).saturating_sub(cols_written);
		if remaining > 0 {
			write_spaces(w, remaining)?;
		}
		screen_row += 1;
	}

	Ok(())
}

/// Render text lines in no-wrap mode (horizontal scroll).
pub fn render_nowrap<W: Write>(
	editor: &Editor,
	w: &mut W,
	vp: &Viewport,
	text_height: usize,
	gutter_width: usize,
	show_line_numbers: bool,
	text_area_width: usize,
	sel_range: Option<(usize, usize)>,
	highlight_active: bool,
	cursor_line: usize,
) -> io::Result<()> {
	let line_count = editor.buffer().line_count();
	let sx = editor.scroll_x;

	// Set up syntect highlighter for the buffer's detected syntax.
	let syntax = editor
		.highlighter
		.detect_syntax(editor.buffer().file_path.as_deref());
	let mut hi = HighlightLines::new(syntax, &editor.highlighter.theme);

	// Pre-feed lines before the visible viewport so parse state is correct.
	for pre_line in 0..editor.scroll_y.min(line_count) {
		let pre_text = editor.buffer().text.line(pre_line);
		let _ = hi.highlight_line(&pre_text, &editor.highlighter.syntax_set);
	}

	for row in 0..text_height {
		let line_idx = editor.scroll_y + row;
		let is_active = highlight_active && line_idx == cursor_line;
		let base_bg = if is_active {
			Color::AnsiValue(236)
		} else {
			Color::Reset
		};
		w.queue(cursor::MoveTo(0, row as u16))?;
		w.queue(SetForegroundColor(Color::Reset))?;
		w.queue(SetBackgroundColor(base_bg))?;

		let mut cols_written: usize = 0;

		if line_idx < line_count {
			if show_line_numbers {
				let line_num = format!("{:>width$} ", line_idx + 1, width = gutter_width);
				cols_written += line_num.len();
				if line_idx == cursor_line {
					w.queue(SetForegroundColor(Color::Blue))?;
				} else {
					w.queue(SetForegroundColor(Color::White))?;
				}
				w.queue(style::Print(&line_num))?;
				w.queue(SetForegroundColor(Color::Reset))?;
			}

			let line_text = editor.buffer().text.line(line_idx);
			let line_start_pos = editor.buffer().text.line_to_char(line_idx);
			let tab_w = editor.config.tab_width;

			// Get per-char syntax colors for this line.
			let syn_colors = syntax_colors_for_line(editor, &mut hi, &line_text);

			let mut batch = String::new();
			let mut in_sel = false;
			let mut prev_in_search = false;
			let mut prev_is_current = false;
			let mut prev_syn_fg = Color::Reset;
			let mut vcol: usize = 0;
			let mut visible_written: usize = 0;
			let mut char_idx: usize = 0;

			for ch in line_text.chars() {
				if visible_written >= text_area_width {
					break;
				}
				if ch == '\n' || ch == '\r' {
					char_idx += 1;
					continue;
				}

				let ch_w = if ch == '\t' { tab_w - (vcol % tab_w) } else { char_width(ch, tab_w) };
				let vcol_end = vcol + ch_w;

				// Skip chars entirely before scroll_x
				if vcol_end <= sx {
					vcol = vcol_end;
					char_idx += 1;
					continue;
				}

				// Highlight logic
				let char_pos = line_start_pos + char_idx;
				let want_sel = if let Some((sel_start, sel_end)) = sel_range {
					char_pos >= sel_start && char_pos < sel_end
				} else {
					false
				};
				let search_hit = editor.search_matches.iter().enumerate().find(
					|(_i, &(ms, me))| char_pos >= ms && char_pos < me,
				);
				let is_current_match = search_hit.as_ref().map(|(i, _)| *i == editor.search_match_idx).unwrap_or(false);
				let in_search = search_hit.is_some();

				let cur_syn_fg = syntax_fg(&syn_colors, char_idx);
				let want_state = (want_sel, in_search, is_current_match, if want_sel || in_search { Color::Reset } else { cur_syn_fg });
				let prev_state = (in_sel, prev_in_search, prev_is_current, prev_syn_fg);
				if want_state != prev_state {
					if !batch.is_empty() {
						w.queue(style::Print(&batch))?;
						batch.clear();
					}
					if want_sel {
						w.queue(SetBackgroundColor(Color::DarkBlue))?;
						w.queue(SetForegroundColor(Color::White))?;
					} else if is_current_match {
						w.queue(SetBackgroundColor(Color::DarkYellow))?;
						w.queue(SetForegroundColor(Color::Black))?;
					} else if in_search {
						w.queue(SetBackgroundColor(Color::Yellow))?;
						w.queue(SetForegroundColor(Color::Black))?;
					} else {
						w.queue(SetBackgroundColor(base_bg))?;
						w.queue(SetForegroundColor(cur_syn_fg))?;
					}
					in_sel = want_sel;
					prev_in_search = in_search;
					prev_is_current = is_current_match;
					prev_syn_fg = if want_sel || in_search { Color::Reset } else { cur_syn_fg };
				}

				// Render the character (may be partially clipped at scroll_x boundary)
				if ch == '\t' {
					// Number of visible spaces from this tab
					let start = if vcol < sx { sx } else { vcol };
					let vis_spaces = vcol_end.saturating_sub(start).min(text_area_width - visible_written);
					for _ in 0..vis_spaces {
						batch.push(' ');
					}
					visible_written += vis_spaces;
				} else {
					batch.push(ch);
					visible_written += ch_w;
				}
				vcol = vcol_end;
				char_idx += 1;
			}

			if !batch.is_empty() {
				w.queue(style::Print(&batch))?;
			}
			if in_sel || prev_in_search {
				w.queue(SetBackgroundColor(base_bg))?;
				w.queue(SetForegroundColor(Color::Reset))?;
			}
			cols_written += gutter_width + 1 + visible_written;
		} else {
			if show_line_numbers {
				let line_num = format!("{:>width$} ", line_idx + 1, width = gutter_width);
				cols_written += line_num.len();
				w.queue(SetForegroundColor(Color::DarkGrey))?;
				w.queue(style::Print(&line_num))?;
				w.queue(SetForegroundColor(Color::Reset))?;
			}
		}

		let remaining = (vp.width as usize).saturating_sub(cols_written);
		if remaining > 0 {
			w.queue(SetForegroundColor(Color::Reset))?;
			write_spaces(w, remaining)?;
		}
		// Reset bg after active-line padding
		if is_active {
			w.queue(SetBackgroundColor(Color::Reset))?;
		}
	}

	Ok(())
}

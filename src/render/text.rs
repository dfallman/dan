use crossterm::style::Color;

use super::Viewport;
use crate::editor::layout::{self, WrapOptions};
use crate::editor::Editor;
use crate::syntax::LineHighlighter;
use crate::utils::char_width;

/// When show_whitespace is on, replace whitespace chars with visible markers.
/// Returns (char_to_display, is_whitespace_marker). The marker bool is used
/// to override the foreground color with a dim marker color.
///
/// Tabs are *always* replaced — even with show_whitespace off — because a
/// literal '\t' in a grid cell becomes a `Print('\t')` in the diff, and the
/// terminal interprets that byte as the C0 TAB control character (jump to
/// next tab stop), bypassing our cell-by-cell cursor positioning. The result
/// is shifted/ghosted cells on every tab-indented line. Substituting ' ' here
/// lets the tab-expansion loop in `render_*` fill the run with plain spaces.
fn whitespace_marker(ch: char, show: bool) -> (char, bool) {
	if !show {
		if ch == '\t' {
			return (' ', false);
		}
		return (ch, false);
	}
	match ch {
		' ' => ('·', true),
		'\t' => ('→', true),
		_ => (ch, false),
	}
}

/// Convert a syntect RGBA color to a crossterm Color.
/// Build a per-char foreground color map for one buffer line using syntect.
///
/// Returns a Vec with one `Color` per character in `line_text` (excluding
/// trailing newlines). If highlighting is disabled, returns an empty Vec.
/// Results are cached on `editor.highlight_cache` so soft-wrap scrolling
/// (same buffer line, changing `scroll_vrow`) does not re-lex every frame.
fn syntax_colors_for_line(
	editor: &Editor,
	hi: &mut LineHighlighter<'_>,
	line_idx: usize,
	line_text: &str,
) -> Vec<(Color, bool, bool, bool)> {
	if !editor.config.syntax_highlight {
		return Vec::new();
	}
	let mut cache = editor.highlight_cache.borrow_mut();
	let packed = hi.highlight_line_cached(
		&mut cache,
		line_idx,
		line_text,
		&editor.highlighter.syntax_set,
	);
	packed
		.into_iter()
		.map(|(r, g, b, bold, italic, underline)| {
			(Color::Rgb { r, g, b }, bold, italic, underline)
		})
		.collect()
}

/// Look up the syntax color for a character at `char_idx`.
/// Falls back to `Color::Reset` if the index is out of range or the map is empty.
#[inline]
fn syntax_fg(colors: &[(Color, bool, bool, bool)], char_idx: usize) -> (Color, bool, bool, bool) {
	colors
		.get(char_idx)
		.copied()
		.unwrap_or((Color::Reset, false, false, false))
}

/// Render text lines in wrap mode (soft-wrap). Each buffer line may occupy
/// multiple screen rows.
#[allow(clippy::too_many_arguments)]
pub fn render_wrap(
	editor: &Editor,
	screen: &mut super::buffer::ScreenBuffer,
	vp: &Viewport,
	text_height: usize,
	gutter_width: usize,
	show_line_numbers: bool,
	text_area_width: usize,
	sel_range: Option<(usize, usize)>,
	highlight_active: bool,
	cursor_line: usize,
) {
	let line_count = editor.buffer().line_count();
	let mut screen_row: usize = 0;
	let mut buf_line = editor.buffer().scroll_y;

	let syntax = editor
		.highlighter
		.detect_syntax(editor.buffer().file_path.as_deref());
	let buffer_version = editor.buffer().version;
	let mut hi = editor.highlighter.primed(
		&editor.highlight_cache,
		syntax,
		editor.buffer().scroll_y.min(line_count),
		buffer_version,
		|line_idx| editor.buffer().text.line(line_idx),
	);

	let tab_w = editor.tab_width();
	let opts = WrapOptions::new(tab_w, text_area_width).with_breakindent(editor.config.breakindent);

	while screen_row < text_height && buf_line < line_count {
		let is_active = highlight_active && buf_line == cursor_line;
		let base_bg = if is_active {
			editor.theme.active_row_bg
		} else {
			Color::Reset
		};
		let skip_rows = if buf_line == editor.buffer().scroll_y {
			editor.buffer().scroll_vrow
		} else {
			0
		};

		let line_text = editor.buffer().text.line(buf_line);
		let line_start_pos = editor.buffer().text.line_to_char(buf_line);
		let syn_colors = syntax_colors_for_line(editor, &mut hi, buf_line, &line_text);
		let rows = layout::visual_rows(&line_text, opts);
		let cont_indent = if opts.breakindent {
			layout::leading_indent_width(&line_text, tab_w).min(text_area_width.saturating_sub(1))
		} else {
			0
		};

		for (vrow_idx, &(row_start, row_end)) in rows.iter().enumerate().skip(skip_rows) {
			if screen_row >= text_height {
				break;
			}

			screen.mov_to(0, screen_row as u16);
			screen.clear_attrs();
			if show_line_numbers {
				let gutter = if vrow_idx == 0 {
					format!("{:>width$} ", buf_line + 1, width = gutter_width)
				} else {
					format!("{:>width$} ", "↳", width = gutter_width)
				};
				screen.set_bg(base_bg);
				screen.set_fg(if buf_line == cursor_line {
					editor.theme.line_nr_active
				} else {
					editor.theme.line_nr
				});
				screen.put_str(&gutter);
			}

			let mut screen_col: usize = 0;
			if vrow_idx > 0 && cont_indent > 0 {
				screen.set_bg(base_bg);
				screen.clear_attrs();
				for _ in 0..cont_indent {
					screen.put_char(' ');
				}
				screen_col = cont_indent;
			}

			let mut char_idx = row_start;
			for ch in line_text.chars().skip(row_start) {
				if char_idx >= row_end {
					break;
				}
				if ch == '\n' || ch == '\r' {
					char_idx += 1;
					continue;
				}

				let ch_w = layout::char_display_width(ch, screen_col, tab_w);

				let char_pos = line_start_pos + char_idx;
				let want_sel = if let Some((sel_start, sel_end)) = sel_range {
					char_pos >= sel_start && char_pos < sel_end
				} else {
					false
				};
				let search_hit = editor
					.buffer()
					.search_matches
					.iter()
					.enumerate()
					.find(|(_i, &(ms, me))| char_pos >= ms && char_pos < me);
				let is_current_match = search_hit
					.as_ref()
					.map(|(i, _)| *i == editor.buffer().search_match_idx)
					.unwrap_or(false);
				let in_search = search_hit.is_some();
				let (cur_syn_fg, cur_syn_bold, cur_syn_italic, cur_syn_underline) =
					syntax_fg(&syn_colors, char_idx);

				let (display_ch, is_ws_marker) =
					whitespace_marker(ch, editor.config.show_whitespace);

				if want_sel {
					screen.set_bg(editor.theme.selection_bg);
					screen.set_fg(editor.theme.selection_fg);
					screen.bold = cur_syn_bold;
					screen.italic = cur_syn_italic;
					screen.underline = cur_syn_underline;
				} else if is_current_match {
					screen.set_bg(editor.theme.active_match_bg);
					screen.set_fg(editor.theme.active_match_fg);
					screen.bold = cur_syn_bold;
					screen.italic = cur_syn_italic;
					screen.underline = cur_syn_underline;
				} else if in_search {
					screen.set_bg(editor.theme.match_bg);
					screen.set_fg(editor.theme.match_fg);
					screen.bold = cur_syn_bold;
					screen.italic = cur_syn_italic;
					screen.underline = cur_syn_underline;
				} else {
					screen.set_bg(base_bg);
					let effective_fg = if is_ws_marker {
						editor.theme.line_nr
					} else {
						cur_syn_fg
					};
					screen.set_fg(effective_fg);
					screen.bold = cur_syn_bold;
					screen.italic = cur_syn_italic;
					screen.underline = cur_syn_underline;
				}

				if ch == '\t' {
					screen.put_char(display_ch);
					let pad_ch = if editor.config.show_whitespace {
						'·'
					} else {
						' '
					};
					for _ in 1..ch_w {
						screen.put_char(pad_ch);
					}
				} else {
					screen.put_char(display_ch);
				}

				screen_col += ch_w;
				char_idx += 1;
			}

			{
				let cols_used = gutter_width + 1 + screen_col;
				let remaining = (vp.width as usize).saturating_sub(cols_used);
				if remaining > 0 {
					screen.set_bg(base_bg);
					screen.clear_attrs();
					let is_last_vrow = vrow_idx + 1 == rows.len();
					if is_last_vrow && editor.config.show_whitespace {
						screen.set_fg(editor.theme.line_nr);
						screen.put_char('↵');
						screen.set_fg(Color::Reset);
						for _ in 0..remaining.saturating_sub(1) {
							screen.put_char(' ');
						}
					} else {
						screen.set_fg(Color::Reset);
						for _ in 0..remaining {
							screen.put_char(' ');
						}
					}
				}
			}

			screen_row += 1;
		}

		buf_line += 1;
	}

	while screen_row < text_height {
		screen.mov_to(0, screen_row as u16);
		screen.set_bg(Color::Reset);
		screen.clear_attrs();
		let mut cols_written: usize = 0;
		if show_line_numbers {
			let tilde_gutter = format!("{:>width$} ", "⋅", width = gutter_width);
			screen.set_fg(editor.theme.eof_marker);
			screen.put_str(&tilde_gutter);
			cols_written = gutter_width + 1;
		}
		let remaining = (vp.width as usize).saturating_sub(cols_written);
		if remaining > 0 {
			for _ in 0..remaining {
				screen.put_char(' ');
			}
		}
		screen_row += 1;
	}
}

/// Render text lines in no-wrap mode (horizontal scroll).
#[allow(clippy::too_many_arguments)]
pub fn render_nowrap(
	editor: &Editor,
	screen: &mut super::buffer::ScreenBuffer,
	vp: &Viewport,
	text_height: usize,
	gutter_width: usize,
	show_line_numbers: bool,
	text_area_width: usize,
	sel_range: Option<(usize, usize)>,
	highlight_active: bool,
	cursor_line: usize,
) {
	let line_count = editor.buffer().line_count();
	let sx = editor.scroll_x;

	let syntax = editor
		.highlighter
		.detect_syntax(editor.buffer().file_path.as_deref());
	let buffer_version = editor.buffer().version;
	let mut hi = editor.highlighter.primed(
		&editor.highlight_cache,
		syntax,
		editor.buffer().scroll_y.min(line_count),
		buffer_version,
		|line_idx| editor.buffer().text.line(line_idx),
	);

	for row in 0..text_height {
		let line_idx = editor.buffer().scroll_y + row;
		let is_active = highlight_active && line_idx == cursor_line;
		let base_bg = if is_active {
			editor.theme.active_row_bg
		} else {
			Color::Reset
		};
		screen.mov_to(0, row as u16);
		screen.set_bg(base_bg);
		screen.clear_attrs();
		let mut cols_written: usize = 0;

		if line_idx < line_count {
			if show_line_numbers {
				let line_num = format!("{:>width$} ", line_idx + 1, width = gutter_width);
				cols_written += line_num.len();
				screen.set_fg(if line_idx == cursor_line {
					editor.theme.line_nr_active
				} else {
					editor.theme.line_nr
				});
				screen.put_str(&line_num);
			}

			let line_text = editor.buffer().text.line(line_idx);
			let line_start_pos = editor.buffer().text.line_to_char(line_idx);
			let tab_w = editor.tab_width();
			let syn_colors = syntax_colors_for_line(editor, &mut hi, line_idx, &line_text);

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

				let ch_w = if ch == '\t' {
					tab_w - (vcol % tab_w)
				} else {
					char_width(ch, tab_w)
				};
				let vcol_end = vcol + ch_w;

				if vcol_end <= sx {
					vcol = vcol_end;
					char_idx += 1;
					continue;
				}

				let char_pos = line_start_pos + char_idx;
				let want_sel = if let Some((sel_start, sel_end)) = sel_range {
					char_pos >= sel_start && char_pos < sel_end
				} else {
					false
				};
				let search_hit = editor
					.buffer()
					.search_matches
					.iter()
					.enumerate()
					.find(|(_i, &(ms, me))| char_pos >= ms && char_pos < me);
				let is_current_match = search_hit
					.as_ref()
					.map(|(i, _)| *i == editor.buffer().search_match_idx)
					.unwrap_or(false);
				let in_search = search_hit.is_some();
				let (cur_syn_fg, cur_syn_bold, cur_syn_italic, cur_syn_underline) =
					syntax_fg(&syn_colors, char_idx);

				let (display_ch, is_ws_marker) =
					whitespace_marker(ch, editor.config.show_whitespace);

				if want_sel {
					screen.set_bg(editor.theme.selection_bg);
					screen.set_fg(editor.theme.selection_fg);
					screen.bold = cur_syn_bold;
					screen.italic = cur_syn_italic;
					screen.underline = cur_syn_underline;
				} else if is_current_match {
					screen.set_bg(editor.theme.active_match_bg);
					screen.set_fg(editor.theme.active_match_fg);
					screen.bold = cur_syn_bold;
					screen.italic = cur_syn_italic;
					screen.underline = cur_syn_underline;
				} else if in_search {
					screen.set_bg(editor.theme.match_bg);
					screen.set_fg(editor.theme.match_fg);
					screen.bold = cur_syn_bold;
					screen.italic = cur_syn_italic;
					screen.underline = cur_syn_underline;
				} else {
					screen.set_bg(base_bg);
					let effective_fg = if is_ws_marker {
						editor.theme.line_nr
					} else {
						cur_syn_fg
					};
					screen.set_fg(effective_fg);
					screen.bold = cur_syn_bold;
					screen.italic = cur_syn_italic;
					screen.underline = cur_syn_underline;
				}

				if ch == '\t' {
					let start = if vcol < sx { sx } else { vcol };
					let vis_count = vcol_end
						.saturating_sub(start)
						.min(text_area_width - visible_written);
					// First visible cell: tab marker (→ or space).
					let mut emitted = 0usize;
					if vis_count > 0 {
						screen.put_char(display_ch);
						emitted += 1;
					}
					// Remaining cells: pad with · or space.
					let pad_ch = if editor.config.show_whitespace { '·' } else { ' ' };
					for _ in emitted..vis_count {
						screen.put_char(pad_ch);
					}
					visible_written += vis_count;
				} else {
					screen.put_char(display_ch);
					visible_written += ch_w;
				}
				vcol = vcol_end;
				char_idx += 1;
			}

			cols_written += visible_written;
		} else {
			if show_line_numbers {
				let tilde_gutter = format!("{:>width$} ", "⋅", width = gutter_width);
				cols_written += gutter_width + 1;
				screen.set_fg(editor.theme.eof_marker);
				screen.put_str(&tilde_gutter);
				screen.set_fg(Color::Reset);
			}
		}

		let remaining = (vp.width as usize).saturating_sub(cols_written);
		if remaining > 0 {
			screen.set_bg(base_bg);
			screen.clear_attrs();
			// EOL marker: emit ↵ before padding for real lines when show_whitespace is on.
			let is_real_line = line_idx < line_count;
			if editor.config.show_whitespace && is_real_line {
				screen.set_fg(editor.theme.line_nr);
				screen.put_char('↵');
			}
			screen.set_fg(Color::Reset);
			let pad_remaining = if editor.config.show_whitespace && is_real_line {
				remaining.saturating_sub(1)
			} else {
				remaining
			};
			for _ in 0..pad_remaining {
				screen.put_char(' ');
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use crate::buffer::rope::TextRope;
	use crate::editor::Editor;
	use std::path::PathBuf;

	/// Render two lines where line 0 is markdown emphasis (forced italic by
	/// `Editor::new`'s markup.italic theme override) and line 1 is plain.
	/// Returns the composed screen grid as (chars, italic-flags) per row.
	fn render_grid(wrap: bool) -> Vec<(Vec<char>, Vec<bool>)> {
		let mut e = Editor::new();
		e.config.wrap_lines = wrap;
		e.config.line_numbers = true;
		e.config.syntax_highlight = true;
		e.buffer_mut().file_path = Some(PathBuf::from("test.md"));
		e.buffer_mut().text = TextRope::from_str("*italic text*\nplain second line\n");
		let mut out: Vec<u8> = Vec::new();
		crate::render::render(&mut e, &mut out).unwrap();
		let scr = e.last_screen.as_ref().unwrap();
		let w = scr.width as usize;
		let h = scr.height as usize;
		(0..h)
			.map(|y| {
				let cells = &scr.grid[y * w..(y + 1) * w];
				(
					cells.iter().map(|c| c.ch).collect(),
					cells.iter().map(|c| c.italic).collect(),
				)
			})
			.collect()
	}

	/// Regression: line numbers were rendered italic when the previous line
	/// ended in an italic syntax token, because the gutter inherited the
	/// buffer's sticky `italic` cell-state instead of resetting it.
	fn assert_gutter_not_italic(wrap: bool) {
		let grid = render_grid(wrap);

		// Precondition: line 0's emphasis really did highlight as italic,
		// otherwise the test can't observe the leak it's guarding against.
		let (row0_ch, row0_it) = &grid[0];
		let last_italic_text = row0_ch
			.iter()
			.zip(row0_it)
			.any(|(c, it)| *it && !c.is_whitespace());
		assert!(last_italic_text, "setup: line 0 text should be italic (wrap={wrap})");

		// The gutter of line 1 (the row whose first glyph is the digit '2')
		// must not carry any italic cells.
		let (row1_ch, row1_it) = &grid[1];
		assert_eq!(row1_ch[0], '2', "row 1 should start with line number 2 (wrap={wrap})");
		let gutter_italic = row1_ch
			.iter()
			.zip(row1_it)
			.take_while(|(c, _)| c.is_ascii_digit() || **c == ' ')
			.any(|(_, it)| *it);
		assert!(!gutter_italic, "line number gutter leaked italic from prior line (wrap={wrap})");
	}

	#[test]
	fn line_number_not_italic_after_italic_line_nowrap() {
		assert_gutter_not_italic(false);
	}

	#[test]
	fn line_number_not_italic_after_italic_line_wrap() {
		assert_gutter_not_italic(true);
	}
}

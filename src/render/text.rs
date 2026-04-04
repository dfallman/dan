use crossterm::style::Color;

use syntect::easy::HighlightLines;
use syntect::highlighting::FontStyle;

use super::Viewport;
use crate::editor::Editor;
use crate::utils::char_width;

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
	if !editor.config.syntax_highlight {
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

/// Calculate a subtle active line highlight background dynamically derived from
/// the theme's default background luminance. This safely avoids garish active
/// line colors defined by eccentric theme authors.

/// Render text lines in wrap mode (soft-wrap).
///
/// Each buffer line may occupy multiple screen rows.
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
	let mut buf_line = editor.scroll_y;

	let syntax = editor
		.highlighter
		.detect_syntax(editor.buffer().file_path.as_deref());
	let mut hi = HighlightLines::new(syntax, &editor.highlighter.theme);

	for pre_line in 0..editor.scroll_y.min(line_count) {
		let pre_text = editor.buffer().text.line(pre_line);
		let _ = hi.highlight_line(&pre_text, &editor.highlighter.syntax_set);
	}

	while screen_row < text_height && buf_line < line_count {
		let is_active = highlight_active && buf_line == cursor_line;
		let base_bg = if is_active {
			if editor.is_light_bg {
				Color::AnsiValue(254)
			} else {
				Color::AnsiValue(236)
			}
		} else {
			Color::Reset
		};
		let skip_lines = if buf_line == editor.scroll_y {
			editor.scroll_vrow
		} else {
			0
		};

		let line_text = editor.buffer().text.line(buf_line);
		let line_start_pos = editor.buffer().text.line_to_char(buf_line);
		let tab_w = editor.tab_width();
		let syn_colors = syntax_colors_for_line(editor, &mut hi, &line_text);

		let mut vcol: usize = 0;
		let mut char_idx: usize = 0;
		let mut screen_col: usize = 0;
		let mut current_vrow: usize = 0;

		if skip_lines == 0 {
			screen.mov_to(0, screen_row as u16);
			if show_line_numbers {
				let line_num = format!("{:>width$} ", buf_line + 1, width = gutter_width);
				screen.set_bg(base_bg);
				screen.set_fg(if buf_line == cursor_line {
					Color::White
				} else {
					Color::DarkGrey
				});
				screen.put_str(&line_num);
			}
		}

		for ch in line_text.chars() {
			if ch == '\n' || ch == '\r' {
				char_idx += 1;
				continue;
			}

			let ch_w = if ch == '\t' {
				tab_w - (vcol % tab_w)
			} else {
				char_width(ch, tab_w)
			};

			if screen_col + ch_w > text_area_width {
				if current_vrow >= skip_lines {
					let remaining = text_area_width.saturating_sub(screen_col);
					if remaining > 0 {
						screen.set_bg(base_bg);
						for _ in 0..remaining {
							screen.put_char(' ');
						}
					}
					screen_row += 1;
					if screen_row >= text_height {
						break;
					}
				}
				current_vrow += 1;
				screen_col = 0;

				if current_vrow >= skip_lines {
					screen.mov_to(0, screen_row as u16);
					if show_line_numbers {
						let wrap_gutter = format!("{:>width$} ", "↳", width = gutter_width);
						screen.set_bg(base_bg);
						screen.set_fg(if buf_line == cursor_line {
							Color::Blue
						} else {
							Color::DarkGrey
						});
						screen.put_str(&wrap_gutter);
					}
				}
			}

			if current_vrow >= skip_lines {
				let char_pos = line_start_pos + char_idx;
				let want_sel = if let Some((sel_start, sel_end)) = sel_range {
					char_pos >= sel_start && char_pos < sel_end
				} else {
					false
				};
				let search_hit = editor
					.search_matches
					.iter()
					.enumerate()
					.find(|(_i, &(ms, me))| char_pos >= ms && char_pos < me);
				let is_current_match = search_hit
					.as_ref()
					.map(|(i, _)| *i == editor.search_match_idx)
					.unwrap_or(false);
				let in_search = search_hit.is_some();
				let cur_syn_fg = syntax_fg(&syn_colors, char_idx);

				if want_sel {
					screen.set_bg(Color::Cyan);
					screen.set_fg(Color::Black);
				} else if is_current_match {
					screen.set_bg(Color::Green);
					screen.set_fg(Color::Black);
				} else if in_search {
					screen.set_bg(Color::Yellow);
					screen.set_fg(Color::Black);
				} else {
					screen.set_bg(base_bg);
					screen.set_fg(cur_syn_fg);
				}

				if ch == '\t' {
					for _ in 0..ch_w {
						screen.put_char(' ');
					}
				} else {
					screen.put_char(ch);
				}
			}

			vcol += ch_w;
			screen_col += ch_w;
			char_idx += 1;
		}

		if current_vrow >= skip_lines {
			let cols_used = gutter_width + 1 + screen_col;
			let remaining = (vp.width as usize).saturating_sub(cols_used);
			if remaining > 0 {
				screen.set_bg(base_bg);
				for _ in 0..remaining {
					screen.put_char(' ');
				}
			}
			screen_row += 1;
		}
		buf_line += 1;
	}

	while screen_row < text_height {
		screen.mov_to(0, screen_row as u16);
		screen.set_bg(Color::Reset);
		let mut cols_written: usize = 0;
		if show_line_numbers {
			let tilde_gutter = format!("{:>width$} ", ".", width = gutter_width);
			screen.set_fg(Color::DarkGrey);
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
	let mut hi = HighlightLines::new(syntax, &editor.highlighter.theme);

	for pre_line in 0..editor.scroll_y.min(line_count) {
		let pre_text = editor.buffer().text.line(pre_line);
		let _ = hi.highlight_line(&pre_text, &editor.highlighter.syntax_set);
	}

	for row in 0..text_height {
		let line_idx = editor.scroll_y + row;
		let is_active = highlight_active && line_idx == cursor_line;
		let base_bg = if is_active {
			if editor.is_light_bg {
				Color::AnsiValue(254)
			} else {
				Color::AnsiValue(236)
			}
		} else {
			Color::Reset
		};
		screen.mov_to(0, row as u16);
		screen.set_bg(base_bg);
		let mut cols_written: usize = 0;

		if line_idx < line_count {
			if show_line_numbers {
				let line_num = format!("{:>width$} ", line_idx + 1, width = gutter_width);
				cols_written += line_num.len();
				screen.set_fg(if line_idx == cursor_line {
					Color::White
				} else {
					Color::DarkGrey
				});
				screen.put_str(&line_num);
			}

			let line_text = editor.buffer().text.line(line_idx);
			let line_start_pos = editor.buffer().text.line_to_char(line_idx);
			let tab_w = editor.tab_width();
			let syn_colors = syntax_colors_for_line(editor, &mut hi, &line_text);

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
					.search_matches
					.iter()
					.enumerate()
					.find(|(_i, &(ms, me))| char_pos >= ms && char_pos < me);
				let is_current_match = search_hit
					.as_ref()
					.map(|(i, _)| *i == editor.search_match_idx)
					.unwrap_or(false);
				let in_search = search_hit.is_some();
				let cur_syn_fg = syntax_fg(&syn_colors, char_idx);

				if want_sel {
					screen.set_bg(Color::Cyan);
					screen.set_fg(Color::Black);
				} else if is_current_match {
					screen.set_bg(Color::DarkYellow);
					screen.set_fg(Color::Black);
				} else if in_search {
					screen.set_bg(Color::Yellow);
					screen.set_fg(Color::Black);
				} else {
					screen.set_bg(base_bg);
					screen.set_fg(cur_syn_fg);
				}

				if ch == '\t' {
					let start = if vcol < sx { sx } else { vcol };
					let vis_spaces = vcol_end
						.saturating_sub(start)
						.min(text_area_width - visible_written);
					for _ in 0..vis_spaces {
						screen.put_char(' ');
					}
					visible_written += vis_spaces;
				} else {
					screen.put_char(ch);
					visible_written += ch_w;
				}
				vcol = vcol_end;
				char_idx += 1;
			}

			cols_written += visible_written;
		} else {
			if show_line_numbers {
				let line_num = format!("{:>width$} ", line_idx + 1, width = gutter_width);
				cols_written += line_num.len();
				screen.set_fg(Color::DarkGrey);
				screen.put_str(&line_num);
				screen.set_fg(Color::Reset);
			}
		}

		let remaining = (vp.width as usize).saturating_sub(cols_written);
		if remaining > 0 {
			screen.set_fg(Color::Reset);
			screen.set_bg(base_bg);
			for _ in 0..remaining {
				screen.put_char(' ');
			}
		}
	}
}

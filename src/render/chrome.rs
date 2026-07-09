use crossterm::style::Color;
use std::collections::HashSet;

use super::Viewport;
use crate::editor::mode::Mode;
use crate::editor::Editor;
use crate::palette::match_::score_with_indices;
use crate::palette::{PaletteItem, PaletteRow};
use nucleo::Matcher;

use crate::ui::layout::{Gravity, Rect, UiFragment, Window};
use crate::ui::overlay::{OverlayBlock, OverlayBuilder};
use crate::ui::i18n::Message;

fn truncate_path(path: &str, max_len: usize) -> String {
    if path.chars().count() <= max_len {
        return path.to_string();
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() <= 3 {
        let start_len = 6.min(max_len.saturating_sub(4));
        let end_len = max_len.saturating_sub(start_len + 3);
        let start: String = path.chars().take(start_len).collect();
        let end: String = path.chars().skip(path.chars().count().saturating_sub(end_len)).collect();
        return format!("{}...{}", start, end);
    }
    let start_idx = if path.starts_with('/') { 2 } else { 1 };
    let start_end = start_idx.min(parts.len().saturating_sub(2));
    let start_str = parts[0..=start_end].join("/"); 

    let mut end_idx = parts.len() - 1;
    let mut end_str = parts[end_idx].to_string();
    
    while end_idx > start_end + 1 {
        let next_end = format!("{}/{}", parts[end_idx - 1], end_str);
        if start_str.chars().count() + 4 + next_end.chars().count() <= max_len {
            end_str = next_end;
            end_idx -= 1;
        } else {
            break;
        }
    }
    if start_str.chars().count() + 4 + end_str.chars().count() <= max_len {
        format!("{}/.../{}", start_str, end_str)
    } else {
        let start_len = 6.min(max_len.saturating_sub(4));
        let end_len = max_len.saturating_sub(start_len + 3);
        let start: String = path.chars().take(start_len).collect();
        let end: String = path.chars().skip(path.chars().count().saturating_sub(end_len)).collect();
        format!("{}...{}", start, end)
	}
}

/// Matched char indices for palette fuzzy highlighting (one matcher per call).
fn palette_match_indices(query: &str, haystack: &str) -> Vec<u32> {
	if query.is_empty() {
		return Vec::new();
	}
	let mut matcher = Matcher::new(nucleo::Config::DEFAULT);
	score_with_indices(&mut matcher, query, haystack)
		.map(|(_, idx)| idx)
		.unwrap_or_default()
}

/// Map nucleo indices from `original` onto displayed text after left-truncation.
fn remap_indices_fit_left(original: &str, display: &str, indices: &[u32]) -> HashSet<usize> {
	let orig_len = original.chars().count();
	let disp_len = display.chars().count();
	if orig_len <= disp_len && !display.starts_with('…') {
		return indices.iter().map(|&i| i as usize).collect();
	}
	if display.starts_with('…') {
		let kept_len = disp_len.saturating_sub(1);
		let start = orig_len.saturating_sub(kept_len);
		return indices
			.iter()
			.filter_map(|&idx| {
				let i = idx as usize;
				(i >= start).then_some(i - start + 1)
			})
			.collect();
	}
	HashSet::new()
}

/// Map nucleo indices after right-truncation to `display_len` columns.
fn remap_indices_trunc_right(display_len: usize, indices: &[u32]) -> HashSet<usize> {
	indices
		.iter()
		.filter_map(|&idx| ((idx as usize) < display_len).then_some(idx as usize))
		.collect()
}

fn palette_frag(text: String, fg: Color, bg: Color, bold: bool) -> UiFragment {
	UiFragment {
		text,
		fg,
		bg,
		is_flex: false,
		is_bold: bold,
	}
}

/// Split `text` into fragments, bolding chars whose indices appear in `highlight`.
fn highlight_text_frags(
	text: &str,
	highlight: &HashSet<usize>,
	fg: Color,
	bg: Color,
) -> Vec<UiFragment> {
	let mut frags = Vec::new();
	let mut run = String::new();
	let mut run_bold = false;
	for (i, ch) in text.chars().enumerate() {
		let bold = highlight.contains(&i);
		if run.is_empty() {
			run_bold = bold;
			run.push(ch);
		} else if bold == run_bold {
			run.push(ch);
		} else {
			frags.push(palette_frag(run, fg, bg, run_bold));
			run = ch.to_string();
			run_bold = bold;
		}
	}
	if !run.is_empty() {
		frags.push(palette_frag(run, fg, bg, run_bold));
	}
	frags
}

/// Pad/truncate `s` to exactly `n` display columns.
fn palette_fit(s: &str, n: usize) -> String {
	let count = s.chars().count();
	if count == n {
		s.to_string()
	} else if count > n {
		s.chars().take(n).collect()
	} else {
		let mut out = s.to_string();
		for _ in 0..(n - count) {
			out.push(' ');
		}
		out
	}
}

/// Left-truncate `s` to `n` columns with a leading "…".
fn palette_fit_left(s: &str, n: usize) -> String {
	let count = s.chars().count();
	if count <= n {
		let mut out = s.to_string();
		for _ in 0..(n - count) {
			out.push(' ');
		}
		out
	} else if n == 0 {
		String::new()
	} else {
		let kept: String = s
			.chars()
			.rev()
			.take(n - 1)
			.collect::<Vec<_>>()
			.into_iter()
			.rev()
			.collect();
		format!("…{}", kept)
	}
}

/// Build highlighted body fragments for one palette result row.
fn palette_row_body_frags(
	item: &PaletteItem,
	query: &str,
	body_inner: usize,
	row_fg: Color,
	row_bg: Color,
	dim: Color,
) -> Vec<UiFragment> {
	let match_idx = palette_match_indices(query, item.search_text());
	let mut frags = Vec::new();

	match item {
		PaletteItem::Action { label, hint, .. } => {
			let hint_str = hint.as_deref().unwrap_or("");
			if hint_str.is_empty() {
				let text = palette_fit(label, body_inner);
				let hi = remap_indices_trunc_right(text.chars().count(), &match_idx);
				frags.extend(highlight_text_frags(&text, &hi, row_fg, row_bg));
			} else {
				let lcount = label.chars().count();
				let hcount = hint_str.chars().count();
				if lcount + 2 + hcount <= body_inner {
					let hi = remap_indices_trunc_right(lcount, &match_idx);
					frags.extend(highlight_text_frags(label, &hi, row_fg, row_bg));
					let pad = body_inner - lcount - hcount;
					if pad > 0 {
						frags.push(palette_frag(" ".repeat(pad), row_fg, row_bg, false));
					}
					frags.push(palette_frag(hint_str.to_string(), dim, row_bg, true));
				} else {
					let text = palette_fit(label, body_inner);
					let hi = remap_indices_trunc_right(text.chars().count(), &match_idx);
					frags.extend(highlight_text_frags(&text, &hi, row_fg, row_bg));
				}
			}
		}
		PaletteItem::Buffer {
			path_display,
			dirty,
			..
		} => {
			let dirty_w = if *dirty { 2 } else { 0 };
			let path_room = body_inner.saturating_sub(dirty_w);
			let path_chars: Vec<char> = path_display.chars().collect();
			let path_str: String = if path_chars.len() <= path_room {
				path_display.to_string()
			} else if path_room == 0 {
				String::new()
			} else {
				let kept: String = path_chars
					.iter()
					.rev()
					.take(path_room - 1)
					.copied()
					.collect::<Vec<_>>()
					.into_iter()
					.rev()
					.collect();
				format!("…{}", kept)
			};
			let hi = remap_indices_fit_left(path_display, &path_str, &match_idx);
			frags.extend(highlight_text_frags(&path_str, &hi, row_fg, row_bg));
			if *dirty {
				frags.push(palette_frag(" ".to_string(), row_fg, row_bg, false));
				frags.push(palette_frag("●".to_string(), row_fg, row_bg, true));
			}
			let body_chars: usize = frags.iter().map(|f| f.text.chars().count()).sum();
			let body_pad = body_inner.saturating_sub(body_chars);
			if body_pad > 0 {
				frags.push(palette_frag(" ".repeat(body_pad), row_fg, row_bg, false));
			}
		}
		PaletteItem::File { display, .. } => {
			let text = palette_fit_left(display, body_inner);
			let hi = remap_indices_fit_left(display, &text, &match_idx);
			frags.extend(highlight_text_frags(&text, &hi, row_fg, row_bg));
		}
	}

	let body_chars: usize = frags.iter().map(|f| f.text.chars().count()).sum();
	let body_pad = body_inner.saturating_sub(body_chars);
	if body_pad > 0 {
		frags.push(palette_frag(" ".repeat(body_pad), row_fg, row_bg, false));
	}
	frags
}

fn palette_hline(left: char, right: char, inner: usize) -> String {
	format!("{}{}{}", left, "─".repeat(inner), right)
}

/// `│` + inner content (padded) + `│`
fn palette_border_row(
	inner_content: Vec<UiFragment>,
	inner: usize,
	line: Color,
	bg: Color,
) -> Vec<UiFragment> {
	let used: usize = inner_content.iter().map(|f| f.text.chars().count()).sum();
	let mut content = inner_content;
	if used < inner {
		let (fg, row_bg) = content
			.last()
			.map(|f| (f.fg, f.bg))
			.unwrap_or((line, bg));
		content.push(palette_frag(" ".repeat(inner - used), fg, row_bg, false));
	}
	let mut row = Vec::with_capacity(2 + content.len());
	row.push(palette_frag("│".to_string(), line, bg, false));
	row.extend(content);
	row.push(palette_frag("│".to_string(), line, bg, false));
	row
}

/// Localized section title for palette group ids (see `group_of` in palette state).
fn palette_section_label(editor: &Editor, group: u8) -> String {
	match group {
		0 => editor.locale.translate(Message::PaletteSectionBuffers),
		1 => editor.locale.translate(Message::PaletteSectionFiles),
		2 => editor.locale.translate(Message::PaletteSectionCommands),
		_ => String::new(),
	}
}

/// Render the status bar.
pub fn build_status_bar(editor: &Editor, vp: &Viewport) -> Window {
	let mut fragments = Vec::new();

	let c = editor.buffer().cursors.cursor();
	let mut right_parts = Vec::new();

	if editor.config.show_help {
		right_parts.push(editor.locale.translate(Message::HelpCommandKey));
	}
	if editor.config.show_lang {
		let syntax = editor
			.highlighter
			.detect_syntax(editor.buffer().file_path.as_deref());
		right_parts.push(syntax.name.clone());
	}
	if editor.config.show_encoding {
		right_parts.push(editor.buffer().encoding.name().to_string());
	}
	right_parts.push(editor.locale.translate(Message::LineCol(c.line + 1, c.col + 1)));

	let right = format!(" {} ", right_parts.join("  "));
	let right_width = right.chars().count();

	fragments.push(UiFragment {
		text: editor.locale.translate(Message::ToolbarPrefix),
		fg: editor.theme.status_bg,
		bg: editor.theme.toolbar_bg,
		is_flex: false, is_bold: false,
	});

	fragments.push(UiFragment { text: " ".to_string(), fg: editor.theme.toolbar_bg, bg: editor.theme.toolbar_bg, is_flex: false, is_bold: false });

	let mode_text = if editor.has_selection() {
		editor.locale.translate(Message::SelectionModeLabel)
	} else {
		editor.locale.translate(Message::ModeLabelEditing)
	};
	let mode_color = if editor.has_selection() {
		editor.theme.mode_select
	} else {
		editor.mode.color(&editor.theme)
	};

	let mut left_width = editor.locale.translate(Message::ToolbarPrefix).chars().count() + 1 + mode_text.chars().count() + 1;

	fragments.push(UiFragment {
		text: mode_text,
		fg: mode_color,
		bg: editor.theme.toolbar_bg,
		is_flex: false, is_bold: true,
	});

	fragments.push(UiFragment { text: " ".to_string(), fg: editor.theme.toolbar_bg, bg: editor.theme.toolbar_bg, is_flex: false, is_bold: false });

	if editor.buffer().dirty {
		left_width += 1 + editor.locale.translate(Message::DirtyFlag).chars().count();
	}
	if let Some(ref msg) = editor.status_msg {
		left_width += 1 + editor.locale.translate(Message::StatusMessage(msg.clone())).chars().count();
	}

	let label_margin = editor.locale.translate(Message::FilenameLabel(String::new())).chars().count();
	let max_name_len = (vp.width as usize).saturating_sub(left_width + right_width + label_margin + 5);

	let raw_name = if editor.config.show_full_path {
		editor.buffer().full_path_display()
	} else {
		editor.buffer().display_name()
	};

	let name = if raw_name.chars().count() > max_name_len && max_name_len > 0 {
		if editor.config.show_full_path {
			truncate_path(&raw_name, max_name_len)
		} else {
			let trunc: String = raw_name.chars().take(max_name_len.saturating_sub(3)).collect();
			format!("{}...", trunc)
		}
	} else {
		raw_name
	};

	fragments.push(UiFragment {
		text: editor.locale.translate(Message::FilenameLabel(name)),
		fg: editor.theme.toolbar_fg,
		bg: editor.theme.toolbar_bg,
		is_flex: false, is_bold: false,
	});

	if editor.buffer().dirty {
		fragments.push(UiFragment { text: " ".to_string(), fg: editor.theme.toolbar_bg, bg: editor.theme.toolbar_bg, is_flex: false, is_bold: false });
		fragments.push(UiFragment {
			text: editor.locale.translate(Message::DirtyFlag),
			fg: editor.theme.dirty_flag,
			bg: editor.theme.toolbar_bg,
			is_flex: false, is_bold: false,
		});
	}

	if let Some(ref msg) = editor.status_msg {
		fragments.push(UiFragment { text: " ".to_string(), fg: editor.theme.toolbar_bg, bg: editor.theme.toolbar_bg, is_flex: false, is_bold: false });
		fragments.push(UiFragment {
			text: editor.locale.translate(Message::StatusMessage(msg.clone())),
			fg: editor.theme.dirty_flag,
			bg: editor.theme.toolbar_bg,
			is_flex: false, is_bold: false,
		});
	}

	fragments.push(UiFragment {
		text: String::new(),
		fg: editor.theme.toolbar_bg,
		bg: editor.theme.toolbar_bg,
		is_flex: true, is_bold: false,
	});

	fragments.push(UiFragment {
		text: right,
		fg: editor.theme.toolbar_info,
		bg: editor.theme.toolbar_bg,
		is_flex: false, is_bold: false,
	});

	let status_y = vp.height.saturating_sub(vp.chrome_rows);

	Window {
		rect: Rect {
			x: 0,
			y: status_y,
			width: vp.width,
			height: 1,
		},
		gravity: Gravity::BottomLeft,
		z_index: 0,
		cursor_bounds: None,
		fragments,
	}
}

/// Shortcut definitions for the help bar.
fn help_shortcuts(editor: &Editor) -> Vec<(String, String)> {
	vec![
		("^S".to_string(), editor.locale.translate(Message::HelpShortcutSave)),
		("^A".to_string(), editor.locale.translate(Message::HelpShortcutSaveAs)),
		("^Q".to_string(), editor.locale.translate(Message::HelpShortcutQuit)),
		("^Z".to_string(), editor.locale.translate(Message::HelpShortcutUndo)),
		("^Y".to_string(), editor.locale.translate(Message::HelpShortcutRedo)),
		("^C".to_string(), editor.locale.translate(Message::HelpShortcutCopy)),
		("^X".to_string(), editor.locale.translate(Message::HelpShortcutCut)),
		("^V".to_string(), editor.locale.translate(Message::HelpShortcutPaste)),
		("^F".to_string(), editor.locale.translate(Message::HelpShortcutFind)),
		("^G".to_string(), editor.locale.translate(Message::HelpShortcutGoto)),
		("^P".to_string(), editor.locale.translate(Message::HelpShortcutPalette)),
		("^N".to_string(), editor.locale.translate(Message::HelpShortcutNewBuffer)),
		("^D".to_string(), editor.locale.translate(Message::HelpShortcutDuplicate)),
		("^K".to_string(), editor.locale.translate(Message::HelpShortcutDelete)),
		("^W".to_string(), editor.locale.translate(Message::HelpShortcutWrap)),
		("^L".to_string(), editor.locale.translate(Message::HelpShortcutLint)),
		("^E".to_string(), editor.locale.translate(Message::HelpShortcutComment)),
		("^T".to_string(), editor.locale.translate(Message::HelpShortcutSyntax)),
		("^R".to_string(), editor.locale.translate(Message::HelpShortcutWhitespace)),
		("^H".to_string(), editor.locale.translate(Message::HelpShortcutHelp)),
	]
}



/// Palette modal size `(width, height)` in terminal cells, or `None` if too small.
pub fn palette_modal_size(vw: u16, vh: u16) -> Option<(u16, u16)> {
	let width = vw.saturating_sub(4).min(80);
	let height = vh.saturating_sub(4).min(20);
	if width < 30 || height < 6 {
		None
	} else {
		Some((width, height))
	}
}

/// Centered bounding box `(x, y, width, height)` for the palette modal.
pub fn palette_modal_rect(vw: u16, vh: u16) -> Option<(u16, u16, u16, u16)> {
	let (w, h) = palette_modal_size(vw, vh)?;
	let x = vw.saturating_sub(w) / 2;
	let y = vh.saturating_sub(h) / 2;
	Some((x, y, w, h))
}

/// Build a centered modal window for the command palette.
///
/// Returns one `Window` per row (top border, query bar, separator, result rows,
/// footer separator, status, bottom border). All rows share the same
/// `rect.width` and `rect.height` so `Gravity::Center` positions them as a
/// single bounding box; `rect.y` is treated as an additive offset within that
/// box by the render loop.
///
/// Returns an empty vec if the viewport is too small to render the modal.
pub fn build_palette_window(editor: &Editor, vw: u16, vh: u16) -> Vec<Window> {
	let mut windows: Vec<Window> = Vec::new();

	let Some((palette_w, palette_h)) = palette_modal_size(vw, vh) else {
		return windows;
	};
	let visible_rows = (palette_h as usize).saturating_sub(6);
	let total = editor.palette.filtered.len();
	let query = &editor.palette.query;

	let p = &editor.theme.palette;
	let bg = p.bg;
	let fg = p.fg;
	let line = p.border;
	let dim = p.dim;
	let accent = p.accent;

	let make_row = |y: u16, frags: Vec<UiFragment>| -> Window {
		Window {
			rect: Rect { x: 0, y, width: palette_w, height: palette_h },
			gravity: Gravity::Center,
			z_index: 250,
			cursor_bounds: None,
			fragments: frags,
		}
	};

	let inner = palette_w as usize - 2;

	fn frag(text: String, fg: Color, bg: Color) -> UiFragment {
		palette_frag(text, fg, bg, false)
	}

	fn hline_row(left: char, right: char, inner: usize, line: Color, bg: Color) -> Vec<UiFragment> {
		vec![frag(palette_hline(left, right, inner), line, bg)]
	}

	// Row 0: top border
	windows.push(make_row(
		0,
		hline_row('┌', '┐', inner, line, bg),
	));

	// Row 1: query bar
	let query_inner = inner.saturating_sub(3);
	let query_frags: Vec<UiFragment> = if query.is_empty() {
		vec![frag(
			palette_fit(
				&editor.locale.translate(Message::PalettePlaceholder),
				query_inner,
			),
			dim,
			bg,
		)]
	} else {
		vec![palette_frag(
			palette_fit(query, query_inner),
			fg,
			bg,
			true,
		)]
	};
	let cursor_col = query
		.get(..editor.palette.query_cursor)
		.map(|s| s.chars().count())
		.unwrap_or(0);
	let cursor_cx = (4u16 + cursor_col as u16).min(palette_w.saturating_sub(2));
	let mut query_content = vec![
		frag(" ".to_string(), fg, bg),
		palette_frag("▶".to_string(), accent, bg, true),
		frag(" ".to_string(), fg, bg),
	];
	query_content.extend(query_frags);
	windows.push(Window {
		rect: Rect {
			x: 0,
			y: 1,
			width: palette_w,
			height: palette_h,
		},
		gravity: Gravity::Center,
		z_index: 250,
		cursor_bounds: Some((cursor_cx, 0)),
		fragments: palette_border_row(query_content, inner, line, bg),
	});

	// Row 2: separator under query
	windows.push(make_row(
		2,
		hline_row('├', '┤', inner, line, bg),
	));

	// Result rows — or the dirty-buffer close prompt when close_prompt_idx is set.
	if editor.palette.close_prompt_idx.is_some() {
		let prompt_lines: [&str; 5] = [
			"  Buffer has unsaved changes:",
			"",
			"  \u{2303}S  Save and close",
			"  \u{2303}D  Discard and close",
			"  Esc  Cancel",
		];
		for screen_idx in 0..visible_rows as u16 {
			let row_y = 3 + screen_idx;
			let prompt_line = prompt_lines.get(screen_idx as usize).copied().unwrap_or("");
			let line_fg = if screen_idx == 0 { fg } else { dim };
			let bold = screen_idx == 0;
			windows.push(make_row(
				row_y,
				palette_border_row(
					vec![palette_frag(
						palette_fit(prompt_line, inner),
						line_fg,
						bg,
						bold,
					)],
					inner,
					line,
					bg,
				),
			));
		}
	} else {
		let no_matches = total == 0 && !query.is_empty();
		let rows = editor.palette.display_rows();
		let mut screen_idx: u16 = 0;
		let mut row_cursor = editor.palette.scroll;
		while screen_idx < visible_rows as u16 {
			let row_y = 3 + screen_idx;
			if let Some(PaletteRow::Section(g)) = rows.get(row_cursor) {
				let label = format!("  {}", palette_section_label(editor, *g));
				let section_fg = p.section_fg(*g);
				windows.push(make_row(
					row_y,
					palette_border_row(
						vec![palette_frag(
							palette_fit(&label, inner),
							section_fg,
							bg,
							true,
						)],
						inner,
						line,
						bg,
					),
				));
				screen_idx += 1;
				row_cursor += 1;
				continue;
			}
			let filtered_idx = match rows.get(row_cursor) {
				Some(&crate::palette::PaletteRow::Item(i)) => i,
				_ => {
					if no_matches && screen_idx == 0 {
						let label = editor.locale.translate(Message::PaletteNoMatches);
						let body_inner = inner.saturating_sub(4);
						let pad = body_inner.saturating_sub(label.chars().count());
						windows.push(make_row(
							row_y,
							palette_border_row(
								vec![
									frag("   ".to_string(), fg, bg),
									frag(label.to_string(), dim, bg),
									frag(" ".repeat(pad), fg, bg),
									frag(" ".to_string(), fg, bg),
								],
								inner,
								line,
								bg,
							),
						));
					} else {
						windows.push(make_row(
							row_y,
							palette_border_row(vec![], inner, line, bg),
						));
					}
					screen_idx += 1;
					row_cursor += 1;
					continue;
				}
			};
			let (item_index_in_all, _score) = editor.palette.filtered[filtered_idx];
			let item = &editor.palette.all_items[item_index_in_all];
			let is_selected = filtered_idx == editor.palette.selection;
			let row_bg = if is_selected { fg } else { bg };
			let row_fg = if is_selected { bg } else { fg };
			let body_inner = inner.saturating_sub(4);

			let body_frags =
				palette_row_body_frags(item, query, body_inner, row_fg, row_bg, p.hint);

			let marker_str = if is_selected { "→" } else { " " };
			let content = vec![
				palette_frag(" ".to_string(), row_fg, row_bg, false),
				palette_frag(marker_str.to_string(), row_fg, row_bg, is_selected),
				palette_frag(" ".to_string(), row_fg, row_bg, false),
			]
			.into_iter()
			.chain(body_frags)
			.chain(std::iter::once(palette_frag(
				" ".to_string(),
				row_fg,
				row_bg,
				false,
			)))
			.collect();

			windows.push(make_row(
				row_y,
				palette_border_row(content, inner, line, bg),
			));
			screen_idx += 1;
			row_cursor += 1;
		}
	}

	// Footer separator
	let footer_y = 3 + visible_rows as u16;
	windows.push(make_row(
		footer_y,
		hline_row('├', '┤', inner, line, bg),
	));

	// Status row: keyboard hints left, match count right.
	let total_all = editor.palette.all_items.len();
	let indexing = if editor.project_index_rx.is_some() {
		editor.locale.translate(Message::PaletteIndexingSuffix)
	} else {
		String::new()
	};
	let hints = if editor.palette.close_prompt_idx.is_some() {
		editor.locale.translate(Message::PaletteFooterCloseHints)
	} else {
		editor.locale.translate(Message::PaletteFooterHints)
	};
	let mut count = editor
		.locale
		.translate(Message::PaletteResultCount(total, total_all));
	count.push_str(&indexing);
	let count_len = count.chars().count();
	let hints_room = inner.saturating_sub(count_len + 1);
	let hints_display = palette_fit(&hints, hints_room);
	let hints_len = hints_display.chars().count();
	let pad = inner.saturating_sub(hints_len + count_len);
	let footer_content = vec![
		frag(hints_display, dim, bg),
		frag(" ".repeat(pad), dim, bg),
		frag(count, dim, bg),
	];
	windows.push(make_row(
		footer_y + 1,
		palette_border_row(footer_content, inner, line, bg),
	));

	// Bottom border
	windows.push(make_row(
		footer_y + 2,
		hline_row('└', '┘', inner, line, bg),
	));

	windows
}

pub fn build_help_bar(editor: &Editor, width: u16, h: u16) -> Vec<Window> {
	let shortcuts = help_shortcuts(editor);
	let help_title = editor.locale.translate(Message::HelpTitle);
	let help_width = 1 + help_title.chars().count();
	let overflow_padding = " ".repeat(help_width);
	let prefix_str = editor.locale.translate(Message::ToolbarPrefix);
	
	let mut builder = OverlayBuilder::new(editor.theme.toolbar_bg, 0)
		.with_prefix(UiFragment {
			text: prefix_str.clone(),
			fg: editor.theme.status_bg,
			bg: editor.theme.toolbar_bg,
			is_flex: false, is_bold: false,
		})
		.with_overflow_prefix(UiFragment {
			text: format!("{}{}", prefix_str, overflow_padding),
			fg: editor.theme.status_bg,
			bg: editor.theme.toolbar_bg,
			is_flex: false, is_bold: false,
		});


	builder.add_block(OverlayBlock {
		fragments: vec![
			UiFragment { text: " ".to_string(), fg: editor.theme.toolbar_bg, bg: editor.theme.toolbar_bg, is_flex: false, is_bold: false },
			UiFragment {
				text: editor.locale.translate(Message::HelpTitle),
				fg: editor.theme.help_label,
				bg: editor.theme.toolbar_bg,
				is_flex: false, is_bold: false,
			},
		],
	});

	for (key, label) in &shortcuts {
		builder.add_block(OverlayBlock {
			fragments: vec![
				UiFragment { text: " ".to_string(), fg: editor.theme.toolbar_bg, bg: editor.theme.toolbar_bg, is_flex: false, is_bold: false },
				UiFragment {
					text: key.to_string(),
					fg: editor.theme.hotkey,
					bg: editor.theme.toolbar_bg,
					is_flex: false, is_bold: false,
				},
				UiFragment { text: " ".to_string(), fg: editor.theme.toolbar_bg, bg: editor.theme.toolbar_bg, is_flex: false, is_bold: false },
				UiFragment {
					text: label.clone(),
					fg: editor.theme.toolbar_fg,
					bg: editor.theme.toolbar_bg,
					is_flex: false, is_bold: false,
				},
			],
		});
	}

	builder.add_block(OverlayBlock {
		fragments: vec![
			UiFragment { text: " ".to_string(), fg: editor.theme.toolbar_bg, bg: editor.theme.toolbar_bg, is_flex: false, is_bold: false },
			UiFragment {
				text: editor.locale.translate(Message::Version(crate::VERSION.trim().to_string(), crate::GIT_HASH.to_string())),
				fg: editor.theme.toolbar_fg_dim,
				bg: editor.theme.toolbar_bg,
				is_flex: false, is_bold: false,
			}
		],
	});

	builder.build(width, h.saturating_sub(2))
}

pub fn build_info_banner(editor: &Editor, width: u16, base_y: u16) -> Vec<Window> {
	let Some(banner) = editor.info_banner.as_ref() else {
		return Vec::new();
	};
	if banner.pending {
		return Vec::new();
	}

	let desc = if banner.expand_tab {
		editor
			.locale
			.translate(Message::InfoBannerIndentSpaces(banner.tab_width))
	} else {
		editor.locale.translate(Message::InfoBannerIndentTabs)
	};
	let label = editor.locale.translate(Message::InfoBannerLabel);
	let body = editor.locale.translate(Message::InfoBannerBody(desc));
	let prefix_str = editor.locale.translate(Message::ToolbarPrefix);

	let mut builder = OverlayBuilder::new(editor.theme.toolbar_bg, 1)
		.with_prefix(UiFragment {
			text: prefix_str.clone(),
			fg: editor.theme.status_bg,
			bg: editor.theme.toolbar_bg,
			is_flex: false,
			is_bold: false,
		})
		.with_overflow_prefix(UiFragment {
			text: prefix_str,
			fg: editor.theme.status_bg,
			bg: editor.theme.toolbar_bg,
			is_flex: false,
			is_bold: false,
		});

	builder.add_block(OverlayBlock {
		fragments: vec![
			UiFragment {
				text: " ".to_string(),
				fg: editor.theme.toolbar_fg,
				bg: editor.theme.toolbar_bg,
				is_flex: false,
				is_bold: false,
			},
			UiFragment {
				text: label,
				fg: editor.theme.help_label,
				bg: editor.theme.toolbar_bg,
				is_flex: false,
				is_bold: true,
			},
			UiFragment {
				text: body,
				fg: editor.theme.toolbar_fg,
				bg: editor.theme.toolbar_bg,
				is_flex: false,
				is_bold: false,
			},
		],
	});

	builder.build(width, base_y)
}

pub fn build_prompt(editor: &Editor, width: u16, h: u16) -> Option<Vec<Window>> {
	if width == 0 {
		return None;
	}

	let bg_col = match editor.mode {
		Mode::RecoverSwap | Mode::ConfirmQuit => editor.theme.toolbar_bg,
		_ => editor.theme.prompt_bg,
	};
	
	let prefix = match editor.mode {
		Mode::ConfirmQuit | Mode::RecoverSwap => UiFragment {
			text: editor.locale.translate(Message::ToolbarPrefix),
			fg: editor.theme.prompt_danger_bg,
			bg: bg_col,
			is_flex: false, is_bold: true,
		},
		_ => UiFragment {
			text: editor.locale.translate(Message::ToolbarPrefix),
			fg: editor.theme.status_bg,
			bg: bg_col,
			is_flex: false, is_bold: false,
		},
	};

	let mut builder = OverlayBuilder::new(bg_col, 10).with_prefix(prefix);

	// Dynamic inputs require generic sliding viewport mapping 
	let label;
	let label_color;
	let query_text;
	let mut info_prefix = String::new();
	let info_color = editor.theme.prompt_info;
	let info_bg = bg_col;
	let info_suffix;
	let text_cursor;

	match editor.mode {

		Mode::ReplacingWith => {
			label = editor.locale.translate(Message::PromptReplaceWith);
			label_color = editor.theme.mode_replace;
			query_text = editor.replace_with.clone();
			text_cursor = editor.prompt_cursor;

			if !editor.buffer().search_matches.is_empty() {
				info_prefix = editor.locale.translate(Message::MatchFraction(editor.buffer().search_match_idx + 1, editor.buffer().search_matches.len()));
			}
			info_suffix = editor.locale.translate(Message::ReplaceShortcuts);
		}
		Mode::GoToLine => {
			let total_lines = editor.buffers[editor.active_buffer].line_count();
			label = editor.locale.translate(Message::PromptGoToLine);
			label_color = editor.theme.mode_goto;
			query_text = editor.goto_line_input.clone();
			text_cursor = editor.prompt_cursor;
			info_suffix = editor.locale.translate(Message::PromptGoToLineHint(total_lines));
		}
		Mode::SaveAs | Mode::ConfirmOverwrite => {
			label = editor.locale.translate(Message::PromptSaveAs);
			label_color = editor.theme.mode_save;
			query_text = editor.save_as_input.clone();
			text_cursor = editor.prompt_cursor;
			
			if editor.mode == Mode::ConfirmOverwrite {
				info_suffix = editor.locale.translate(Message::PromptConfirmOverwrite);
			} else {
				info_suffix = editor.locale.translate(Message::PromptSaveAsShortcuts);
			}
		}
		Mode::Searching => {
			label = editor.locale.translate(Message::PromptSearch);
			label_color = editor.theme.mode_search;
			query_text = editor.search_query.clone();
			text_cursor = editor.prompt_cursor;
			
			if editor.buffer().search_matches.is_empty() {
				if editor.search_query.is_empty() {
					info_suffix = format!(" {} ", editor.locale.translate(Message::EscToClose));
				} else if editor.search_regex_error {
					info_prefix = editor.locale.translate(Message::InvalidRegex);
					info_suffix = format!(" {} ", editor.locale.translate(Message::EscToClose));
				} else {
					info_prefix = editor.locale.translate(Message::ZeroMatches);
					info_suffix = editor.locale.translate(Message::SearchShortcuts);
				}
			} else {
				info_prefix = editor.locale.translate(Message::MatchFraction(editor.buffer().search_match_idx + 1, editor.buffer().search_matches.len()));
				info_suffix = editor.locale.translate(Message::SearchReplaceShortcuts);
			}
		}
		Mode::ReplacingStep => {
			let label = editor.locale.translate(Message::PromptReplaceStep);
			let mut fragments = vec![
				UiFragment { bg: bg_col, fg: editor.theme.prompt_fg, text: " ".to_string(), is_flex: false, is_bold: false },
			];
			fragments.extend(parse_hotkeys(&label, bg_col, editor.theme.prompt_info, Some(editor.theme.prompt_fg), editor.theme.hotkey));
			builder.add_block(OverlayBlock { fragments });
			return Some(builder.build(width, h.saturating_sub(1)));
		}
		Mode::RecoverSwap => {
			let label = editor.locale.translate(Message::PromptRecoverTitle);
			let msg = editor.locale.translate(Message::PromptRecoverMsg);
			
			let mut fragments = vec![
				UiFragment { bg: editor.theme.prompt_danger_bg, fg: editor.theme.prompt_danger_fg, text: format!(" {} ", label), is_flex: false, is_bold: true },
				UiFragment { bg: bg_col, fg: editor.theme.prompt_fg, text: " ".to_string(), is_flex: false, is_bold: false },
			];
			fragments.extend(parse_hotkeys(&msg, bg_col, editor.theme.prompt_info, Some(editor.theme.prompt_fg), editor.theme.hotkey));

			let cursor_pos = fragments.iter().map(|f| f.text.chars().count()).sum::<usize>();
			builder = builder.with_cursor(cursor_pos);
			builder.add_block(OverlayBlock { fragments });

			return Some(builder.build(width, h.saturating_sub(1)));
		}
		Mode::ConfirmQuit => {
			let label1 = editor.locale.translate(Message::PromptQuitWarning);
			let label2 = editor.locale.translate(Message::PromptQuitMsg);

			let mut fragments = vec![
				UiFragment { bg: editor.theme.prompt_danger_bg, fg: editor.theme.prompt_danger_fg, text: format!(" {} ", label1), is_flex: false, is_bold: true },
				UiFragment { bg: bg_col, fg: editor.theme.prompt_fg, text: " ".to_string(), is_flex: false, is_bold: false },
			];
			fragments.extend(parse_hotkeys(&label2, bg_col, editor.theme.prompt_info, Some(editor.theme.prompt_fg), editor.theme.hotkey));

			let cursor_pos = fragments.iter().map(|f| f.text.chars().count()).sum::<usize>();
			builder = builder.with_cursor(cursor_pos);
			builder.add_block(OverlayBlock { fragments });

			return Some(builder.build(width, h.saturating_sub(1)));
		}
		_ => return None,
	};

	let mut info_chars = info_prefix.chars().count() + info_suffix.chars().count();
	if !info_prefix.is_empty() && !info_suffix.is_empty() {
		info_chars += 1;
	}
	let layout_width = (width as usize).saturating_sub(
		1 + 1 + label.chars().count() + 1 + 1 + info_chars + 2 // padding
	);

	let mut view_start = editor.prompt_view_start.get();
	let screen_cursor_x = crate::ui::layout::calculate_viewport(
		0,
		text_cursor,
		layout_width.max(10), // minimum sensible width for input
		&mut view_start
	);
	editor.prompt_view_start.set(view_start);

	let available_width = layout_width.max(10);
	let visible_slice: String = query_text.chars().skip(view_start).take(available_width).collect();
	
	let has_left = view_start > 0;
	let has_right = query_text.chars().count() > view_start + available_width;

	let cursor_offset = 1 + label.chars().count() + 1 + screen_cursor_x + (if has_left { 1 } else { 0 });
	builder = builder.with_cursor(cursor_offset);

	let mut prompt_frags = vec![
		UiFragment { bg: bg_col, fg: editor.theme.prompt_fg, text: " ".to_string(), is_flex: false, is_bold: false },
		UiFragment { bg: bg_col, fg: label_color, text: label, is_flex: false, is_bold: true },
		UiFragment { bg: bg_col, fg: editor.theme.prompt_fg, text: " ".to_string(), is_flex: false, is_bold: false },
	];

	if has_left {
		prompt_frags.push(UiFragment { bg: bg_col, fg: editor.theme.toolbar_fg_dim, text: editor.locale.translate(Message::PromptClipLeft), is_flex: false, is_bold: false });
	}

	prompt_frags.push(UiFragment { bg: bg_col, fg: editor.theme.prompt_fg, text: visible_slice, is_flex: false, is_bold: false });
	
	if has_right {
		prompt_frags.push(UiFragment { bg: bg_col, fg: editor.theme.toolbar_fg_dim, text: editor.locale.translate(Message::PromptClipRight), is_flex: false, is_bold: false });
	}
	
	prompt_frags.push(UiFragment { bg: bg_col, fg: editor.theme.prompt_fg, text: "  ".to_string(), is_flex: false, is_bold: false });

	if !info_prefix.is_empty() {
		prompt_frags.push(UiFragment { bg: bg_col, fg: editor.theme.prompt_fg, text: info_prefix.clone(), is_flex: false, is_bold: false });
	}
	if !info_suffix.is_empty() {
		if !info_prefix.is_empty() {
			prompt_frags.push(UiFragment { bg: bg_col, fg: editor.theme.prompt_fg, text: " ".to_string(), is_flex: false, is_bold: false });
		}
		prompt_frags.extend(parse_hotkeys(&info_suffix, info_bg, info_color, Some(editor.theme.prompt_fg), editor.theme.hotkey));
	}

	builder.add_block(OverlayBlock { fragments: prompt_frags });

	Some(builder.build(width, h.saturating_sub(1)))
}
pub fn render_ui(
	editor: &Editor,
	screen: &mut super::buffer::ScreenBuffer,
	vp: &Viewport,
) -> Option<(u16, u16)> {
	let mut windows = Vec::new();

	windows.push(build_status_bar(editor, vp));

	let prompt = build_prompt(editor, vp.width, vp.height);
	let mut bottom_overlay: u16 = 0;

	if prompt.is_none() && editor.show_help && !editor.palette.open {
		let help = build_help_bar(editor, vp.width, vp.height);
		bottom_overlay = help.len() as u16;
		windows.extend(help);
	}

	if let Some(p) = prompt {
		bottom_overlay = p.len() as u16;
		windows.extend(p);
	}

	if editor.info_banner_visible() {
		let base_y = vp.height.saturating_sub(1 + bottom_overlay + 1);
		windows.extend(build_info_banner(editor, vp.width, base_y));
	}

	if editor.palette.open {
		windows.extend(build_palette_window(editor, vp.width, vp.height));
	}

	windows.sort_by_key(|w| w.z_index);

	let mut interactive_cursor: Option<(u16, u16)> = None;

	for window in &windows {
		let mut start_y = match window.gravity {
			Gravity::BottomLeft | Gravity::BottomRight => {
				vp.height.saturating_sub(window.rect.height)
			}
			Gravity::TopLeft | Gravity::TopRight => 0,
			Gravity::Center => vp.height.saturating_sub(window.rect.height) / 2,
			Gravity::Fill => 0,
		};
		// Direct translation applying stacked overrides
		if window.rect.y > 0 && window.gravity == Gravity::BottomLeft {
			start_y = window.rect.y;
		}
		// For Center gravity, treat rect.y as an additive offset from the
		// centered origin so callers can stack multiple 1-row windows
		// (e.g. the command palette modal) within a centered bounding box.
		if window.gravity == Gravity::Center {
			start_y = start_y.saturating_add(window.rect.y);
		}

		let start_x = match window.gravity {
			Gravity::BottomRight | Gravity::TopRight => {
				vp.width.saturating_sub(window.rect.width)
			}
			Gravity::Center => vp.width.saturating_sub(window.rect.width) / 2,
			_ => window.rect.x,
		};

		if let Some((cx, cy)) = window.cursor_bounds {
			interactive_cursor = Some((start_x + cx, start_y + cy));
		}

		let mut flex_spaces = 0;
		let mut static_width = 0;

		for frag in &window.fragments {
			if frag.is_flex {
				flex_spaces += 1;
			} else {
				static_width += frag.text.chars().count() as u16;
			}
		}

		let remaining_width = window.rect.width.saturating_sub(static_width);
		let flex_width = remaining_width.checked_div(flex_spaces).unwrap_or(0);

		let mut current_x = start_x;
		let mut current_y = start_y;
		
		screen.mov_to(current_x, current_y);

		for frag in &window.fragments {
			screen.set_bg(frag.bg);
			screen.set_fg(frag.fg);
			screen.set_bold(frag.is_bold);
			screen.italic = false;

			if frag.is_flex {
				for _ in 0..flex_width {
					if current_x >= start_x + window.rect.width {
						current_y += 1;
						current_x = start_x;
						screen.mov_to(current_x, current_y);
					}
					screen.put_char(' ');
					current_x += 1;
				}
			} else {
				for ch in frag.text.chars() {
					if current_x >= start_x + window.rect.width {
						current_y += 1;
						current_x = start_x;
						screen.mov_to(current_x, current_y);
					}
					screen.put_char(ch);
					current_x += 1;
				}
			}
		}
	}

	interactive_cursor
}

/// Split a help-line into fragments: hotkey tokens (`^X`, `Esc`, `⏎`) get
/// `hotkey_color`; surrounding text gets `instruction_color` before the
/// first hotkey and `text_color` after. Used to colorize the help bar.
fn parse_hotkeys(text: &str, bg: Color, text_color: Color, instruction_color: Option<Color>, hotkey_color: Color) -> Vec<UiFragment> {
	let mut fragments = Vec::new();
	let mut current_text = String::new();
	let chars: Vec<char> = text.chars().collect();
	let mut i = 0;
	let mut hit_first_hotkey = false;

	while i < chars.len() {
		if chars[i] == '^' && i + 1 < chars.len() && chars[i+1].is_uppercase() {
			if !current_text.is_empty() {
				fragments.push(UiFragment { bg, fg: if hit_first_hotkey { text_color } else { instruction_color.unwrap_or(text_color) }, text: current_text, is_flex: false, is_bold: false });
				current_text = String::new();
			}
			fragments.push(UiFragment { bg, fg: hotkey_color, text: format!("^{}", chars[i+1]), is_flex: false, is_bold: false });
			hit_first_hotkey = true;
			i += 2;
			continue;
		}

		if i + 2 < chars.len() && chars[i] == 'E' && chars[i+1] == 's' && chars[i+2] == 'c' {
			if !current_text.is_empty() {
				fragments.push(UiFragment { bg, fg: if hit_first_hotkey { text_color } else { instruction_color.unwrap_or(text_color) }, text: current_text, is_flex: false, is_bold: false });
				current_text = String::new();
			}
			fragments.push(UiFragment { bg, fg: hotkey_color, text: "Esc".to_string(), is_flex: false, is_bold: false });
			hit_first_hotkey = true;
			i += 3;
			continue;
		}

		if chars[i] == '⏎' {
			if !current_text.is_empty() {
				fragments.push(UiFragment { bg, fg: if hit_first_hotkey { text_color } else { instruction_color.unwrap_or(text_color) }, text: current_text, is_flex: false, is_bold: false });
				current_text = String::new();
			}
			fragments.push(UiFragment { bg, fg: hotkey_color, text: "⏎".to_string(), is_flex: false, is_bold: false });
			hit_first_hotkey = true;
			i += 1;
			continue;
		}

		current_text.push(chars[i]);
		i += 1;
	}

	if !current_text.is_empty() {
		fragments.push(UiFragment { bg, fg: if hit_first_hotkey { text_color } else { instruction_color.unwrap_or(text_color) }, text: current_text, is_flex: false, is_bold: false });
	}

	fragments
}

#[cfg(test)]
mod info_banner_tests {
	use super::*;
	use crate::editor::Editor;
	use crate::editor::InfoBanner;

	#[test]
	fn build_info_banner_none_when_pending_or_absent() {
		let mut e = Editor::new();
		assert!(build_info_banner(&e, 80, 10).is_empty());
		e.info_banner = Some(InfoBanner {
			expand_tab: true,
			tab_width: 4,
			pending: true,
		});
		assert!(build_info_banner(&e, 80, 10).is_empty());
	}

	#[test]
	fn build_info_banner_paints_full_width_row() {
		let mut e = Editor::new();
		e.info_banner = Some(InfoBanner {
			expand_tab: true,
			tab_width: 4,
			pending: false,
		});
		let wins = build_info_banner(&e, 80, 10);
		assert!(!wins.is_empty());
		assert_eq!(wins[0].rect.width, 80);
		let has_bold_info = wins.iter().any(|w| {
			w.fragments.iter().any(|f| {
				f.is_bold && f.text.contains("Info") && f.fg == e.theme.help_label
			})
		});
		assert!(has_bold_info);
	}
}

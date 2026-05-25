use crossterm::style::Color;

use super::Viewport;
use crate::editor::mode::Mode;
use crate::editor::Editor;



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
	use crate::palette::PaletteItem;

	let mut windows: Vec<Window> = Vec::new();

	let width = vw.saturating_sub(4).min(80);
	let max_height = vh.saturating_sub(4).min(20);
	if width < 30 || max_height < 6 {
		return windows; // too small — caller treats as no-op
	}

	// Layout (rows): top border | query | separator | results... | footer-sep | status | bottom
	// Fixed chrome rows = 6; remaining rows are for results.
	let palette_h = max_height;
	let palette_w = width;
	let visible_rows = (palette_h as usize).saturating_sub(6);
	let total = editor.palette.filtered.len();

	let theme = &editor.theme;
	let bg = theme.prompt_bg;
	let fg = theme.prompt_fg;
	let line = theme.toolbar_fg_dim; // box-drawing border colour (dark grey)
	let dim = theme.prompt_info;
	let accent = theme.accent;        // ▌ left tab + selected marker + "> " prompt
	let input_fg = theme.warning;     // query text — the eye-magnet
	let hint_fg = theme.hotkey;       // ⌃-keystroke hints
	let dirty_fg = theme.dirty_flag;  // post-name "●" on dirty buffers

	fn frag(text: String, fg: crossterm::style::Color, bg: crossterm::style::Color) -> UiFragment {
		UiFragment { text, fg, bg, is_flex: false, is_bold: false }
	}

	// Pad/truncate `s` so it occupies exactly `n` display columns (1 col per char).
	fn fit(s: &str, n: usize) -> String {
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

	// Left-truncate `s` to `n` display columns with a leading "…", preserving
	// the trailing portion (so the filename stays visible on long paths).
	// Pads with spaces if `s` is shorter than `n`.
	fn fit_left(s: &str, n: usize) -> String {
		let count = s.chars().count();
		if count <= n {
			let mut out = s.to_string();
			for _ in 0..(n - count) { out.push(' '); }
			out
		} else if n == 0 {
			String::new()
		} else {
			// Keep the rightmost (n-1) chars and prepend "…".
			let kept: String = s.chars().rev().take(n - 1).collect::<Vec<_>>()
				.into_iter().rev().collect();
			format!("…{}", kept)
		}
	}

	let make_row = |y: u16, frags: Vec<UiFragment>| -> Window {
		Window {
			rect: Rect { x: 0, y, width: palette_w, height: palette_h },
			gravity: Gravity::Center,
			z_index: 250,
			cursor_bounds: None,
			fragments: frags,
		}
	};

	let inner = palette_w as usize - 2; // chars between the side borders

	// Row 0: top border — left edge is the toolbar-style "▌" accent
	let top = format!("{}┐", "─".repeat(inner));
	windows.push(make_row(0, vec![
		frag("▌".to_string(), accent, bg),
		frag(top, line, bg),
	]));

	// Row 1: query bar:  ▌ > <query>           │
	// "▌ > " and trailing "│" total 4 chars; inner content area = inner - 3.
	let query_inner = inner.saturating_sub(3);
	let (query_str, query_color) = if editor.palette.query.is_empty() {
		("Search buffers, files, and commands…".to_string(), dim)
	} else {
		(editor.palette.query.clone(), input_fg)
	};
	let query_text = fit(&query_str, query_inner);
	// Place the visible terminal cursor inside the query field. cx is the
	// column index within this row; "▌ > " is 4 cells, then count display
	// columns of the query up to the byte cursor position (chars == cells
	// for the typical printable input).
	let cursor_col = editor.palette.query
		.get(..editor.palette.query_cursor)
		.map(|s| s.chars().count())
		.unwrap_or(0);
	let cursor_cx = (4u16 + cursor_col as u16).min(palette_w.saturating_sub(2));
	windows.push(Window {
		rect: Rect { x: 0, y: 1, width: palette_w, height: palette_h },
		gravity: Gravity::Center,
		z_index: 250,
		cursor_bounds: Some((cursor_cx, 0)),
		fragments: vec![
			frag("▌".to_string(), accent, bg),
			frag(" > ".to_string(), accent, bg),
			frag(query_text, query_color, bg),
			frag("│".to_string(), line, bg),
		],
	});

	// Row 2: separator
	let sep = format!("{}┤", "─".repeat(inner));
	windows.push(make_row(2, vec![
		frag("▌".to_string(), accent, bg),
		frag(sep, line, bg),
	]));

	// Result rows — or the dirty-buffer close prompt when close_prompt_idx is set.
	if editor.palette.close_prompt_idx.is_some() {
		// Prompt lines (up to visible_rows; we emit at most 5 meaningful lines
		// and fill the rest with blank rows to maintain the border height).
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
			let line_fg = if screen_idx == 0 { theme.dirty_flag } else { fg };
			windows.push(make_row(row_y, vec![
				frag("▌".to_string(), accent, bg),
				frag(fit(prompt_line, inner), line_fg, bg),
				frag("│".to_string(), line, bg),
			]));
		}
	} else {
		// Result rows are driven by one display-row model (items interleaved
		// with section dividers). `scroll` is a visual-row offset into that
		// sequence, so divider rows can no longer desync scroll from selection (1).
		let no_matches = total == 0 && !editor.palette.query.is_empty();
		let rows = editor.palette.display_rows();
		let mut screen_idx: u16 = 0;
		let mut row_cursor = editor.palette.scroll;
		while screen_idx < visible_rows as u16 {
			let row_y = 3 + screen_idx;
			// A divider occupies its own visible row.
			if let Some(crate::palette::PaletteRow::Divider) = rows.get(row_cursor) {
				let div = format!("{}┤", "─".repeat(inner));
				windows.push(make_row(row_y, vec![
					frag("▌".to_string(), accent, bg),
					frag(div, line, bg),
				]));
				screen_idx += 1;
				row_cursor += 1;
				continue;
			}
			let filtered_idx = match rows.get(row_cursor) {
				Some(&crate::palette::PaletteRow::Item(i)) => i,
				_ => {
					// Past the last result: "No matches" on the first row, else blank.
					if no_matches && screen_idx == 0 {
						let label = "No matches";
						let body_inner = inner.saturating_sub(4);
						let pad = body_inner.saturating_sub(label.chars().count());
						windows.push(make_row(row_y, vec![
							frag("▌".to_string(), accent, bg),
							frag("   ".to_string(), fg, bg),
							frag(label.to_string(), dim, bg),
							frag(" ".repeat(pad), fg, bg),
							frag(" ".to_string(), fg, bg),
							frag("│".to_string(), line, bg),
						]));
					} else {
						windows.push(make_row(row_y, vec![
							frag("▌".to_string(), accent, bg),
							frag(fit("", inner), fg, bg),
							frag("│".to_string(), line, bg),
						]));
					}
					screen_idx += 1;
					row_cursor += 1;
					continue;
				}
			};
			let (item_index_in_all, _score) = editor.palette.filtered[filtered_idx];
			let item = &editor.palette.all_items[item_index_in_all];
			let is_selected = filtered_idx == editor.palette.selection;
			let row_bg = if is_selected { theme.selection_bg } else { bg };
			// Inner row layout: leading pad(1) + marker(1) + sep(1) + body + trailing pad(1) = inner
			let body_inner = inner.saturating_sub(4);

			// On the cyan selection band, every fg snaps to selection_fg
			// (black) so the semantic colours (hotkey blue, dirty blue)
			// don't disappear into the background.
			let body_fg = if is_selected { theme.selection_fg } else { fg };
			let row_hint_fg = if is_selected { theme.selection_fg } else { hint_fg };
			let row_dirty_fg = if is_selected { Color::White } else { dirty_fg };

			// Per-row body content fragments. Each (text, fg) pair becomes one
			// fragment with `row_bg` as background. The chars must sum to
			// AT MOST `body_inner`; remaining cells are padded after.
			let body: Vec<(String, Color)> = match item {
				PaletteItem::Action { label, hint, .. } => {
					let hint_str = hint.as_deref().unwrap_or("");
					if hint_str.is_empty() {
						vec![(fit(label, body_inner), body_fg)]
					} else {
						let lcount = label.chars().count();
						let hcount = hint_str.chars().count();
						if lcount + 2 + hcount <= body_inner {
							let pad = body_inner - lcount - hcount;
							vec![
								(label.clone(), body_fg),
								(" ".repeat(pad), body_fg),
								(hint_str.to_string(), row_hint_fg),
							]
						} else {
							vec![(fit(label, body_inner), body_fg)]
						}
					}
				}
				PaletteItem::Buffer { path_display, dirty, .. } => {
					// Right-aligned type label (like an action's hint slot).
					let kind_label = "Buffer";
					let kind_w = kind_label.chars().count();
					let dirty_w = if *dirty { 2 } else { 0 }; // " ●"
					// Path takes the left side, then " ●" (if dirty), then enough
					// padding to push "Buffer" to the right edge.
					let path_room = body_inner.saturating_sub(dirty_w + kind_w + 2);
					let path_chars: Vec<char> = path_display.chars().collect();
					let path_str: String = if path_chars.len() <= path_room {
						path_display.to_string()
					} else if path_room == 0 {
						String::new()
					} else {
						let kept: String = path_chars.iter().rev().take(path_room - 1)
							.copied().collect::<Vec<_>>().into_iter().rev().collect();
						format!("…{}", kept)
					};
					let mut v: Vec<(String, Color)> = vec![(path_str.clone(), body_fg)];
					if *dirty {
						v.push((" ".to_string(), body_fg));
						v.push(("●".to_string(), row_dirty_fg));
					}
					let used = path_str.chars().count() + dirty_w;
					let pad = body_inner.saturating_sub(used + kind_w);
					if pad > 0 {
						v.push((" ".repeat(pad), body_fg));
					}
					// "Buffer" — dim grey when unselected, snaps to selection_fg
					// (black) on the cyan band so it stays readable.
					let kind_fg = if is_selected { theme.selection_fg } else { dim };
					v.push((kind_label.to_string(), kind_fg));
					v
				}
				PaletteItem::File { display, .. } => {
					vec![(fit_left(display, body_inner), body_fg)]
				}
			};

			let body_chars: usize = body.iter().map(|(s, _)| s.chars().count()).sum();
			let body_pad = body_inner.saturating_sub(body_chars);

			let marker_str = if is_selected { "→" } else { " " };
			let marker_fg = if is_selected { theme.selection_fg } else { fg };

			let mut row_frags: Vec<UiFragment> = Vec::with_capacity(7 + body.len());
			row_frags.push(frag("▌".to_string(), accent, bg));
			row_frags.push(frag(" ".to_string(), fg, row_bg));            // leading pad inside row_bg
			row_frags.push(frag(marker_str.to_string(), marker_fg, row_bg));
			row_frags.push(frag(" ".to_string(), fg, row_bg));            // sep
			for (s, color) in body {
				row_frags.push(frag(s, color, row_bg));
			}
			if body_pad > 0 {
				row_frags.push(frag(" ".repeat(body_pad), fg, row_bg));
			}
			row_frags.push(frag(" ".to_string(), fg, row_bg));            // trailing pad inside row_bg
			row_frags.push(frag("│".to_string(), line, bg));

			windows.push(make_row(row_y, row_frags));
			screen_idx += 1;
			row_cursor += 1;
		}
	}

	// Footer separator
	let footer_y = 3 + visible_rows as u16;
	let footer_sep = format!("{}┤", "─".repeat(inner));
	windows.push(make_row(footer_y, vec![
		frag("▌".to_string(), accent, bg),
		frag(footer_sep, line, bg),
	]));

	// Status row
	let total_all = editor.palette.all_items.len();
	let indexing = if editor.project_index_rx.is_some() { " · indexing…" } else { "" };
	let status_left = format!(" {} of {}{}", total, total_all, indexing);
	let status_inner = fit(&status_left, inner);
	windows.push(make_row(footer_y + 1, vec![
		frag("▌".to_string(), accent, bg),
		frag(status_inner, dim, bg),
		frag("│".to_string(), line, bg),
	]));

	// Bottom border
	let bot = format!("{}┘", "─".repeat(inner));
	windows.push(make_row(footer_y + 2, vec![
		frag("▌".to_string(), accent, bg),
		frag(bot, line, bg),
	]));

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

	if prompt.is_none() && editor.show_help && !editor.palette.open {
		windows.extend(build_help_bar(editor, vp.width, vp.height));
	}

	if let Some(p) = prompt {
		windows.extend(p);
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

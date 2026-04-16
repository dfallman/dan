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

	let c = editor.cursors.cursor();
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
		fg: editor.theme.toolbar_fg_dim,
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
		("^H".to_string(), editor.locale.translate(Message::HelpShortcutHelp)),
	]
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

			if !editor.search_matches.is_empty() {
				info_prefix = editor.locale.translate(Message::MatchFraction(editor.search_match_idx + 1, editor.search_matches.len()));
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
			
			if editor.search_matches.is_empty() {
				if editor.search_query.is_empty() {
					info_suffix = format!(" {} ", editor.locale.translate(Message::EscToClose));
				} else {
					info_prefix = editor.locale.translate(Message::ZeroMatches);
					info_suffix = editor.locale.translate(Message::SearchShortcuts);
				}
			} else {
				info_prefix = editor.locale.translate(Message::MatchFraction(editor.search_match_idx + 1, editor.search_matches.len()));
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

	if prompt.is_none() && editor.show_help {
		windows.extend(build_help_bar(editor, vp.width, vp.height));
	}

	if let Some(p) = prompt {
		windows.extend(p);
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
		let flex_width = if flex_spaces > 0 { remaining_width / flex_spaces } else { 0 };

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

/// Parses strings containing hotkey markers (`^X`, `Esc`, `⏎`) isolating them dynamically
/// structurally tracking distinct explicit foreground values natively separating interactive bounds seamlessly.
fn parse_hotkeys(text: &str, bg: Color, text_color: Color, instruction_color: Option<Color>, hotkey_color: Color) -> Vec<UiFragment> {
	let mut fragments = Vec::new();
	let mut current_text = String::new();
	let chars: Vec<char> = text.chars().collect();
	let mut i = 0;
	let mut hit_first_hotkey = false;

	while i < chars.len() {
		// Match "^X" hotkey identifiers gracefully
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

		// Match "Esc" conditionally dynamically natively
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

		// Match "⏎" implicitly dynamically mapping targets gracefully
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

use crossterm::style::Color;

use super::Viewport;
use crate::editor::mode::Mode;
use crate::editor::Editor;

/// Unified toolbar background color based on terminal theme.
pub fn toolbar_bg_color(editor: &Editor) -> Color {
	if editor.is_light_bg {
		Color::AnsiValue(254)
	} else {
		Color::AnsiValue(236)
	}
}

use crate::ui::layout::{Gravity, Rect, UiFragment, Window};
use crate::ui::overlay::{OverlayBlock, OverlayBuilder};
use crate::ui::i18n::Message;

/// Render the status bar.
pub fn build_status_bar(editor: &Editor, vp: &Viewport) -> Window {
	let mut fragments = Vec::new();

	fragments.push(UiFragment {
		text: editor.locale.translate(Message::ToolbarPrefix),
		fg: editor.theme.status_bg,
		bg: editor.theme.toolbar_bg,
		is_flex: false,
	});

	let mode_text = if editor.has_selection() {
		editor.locale.translate(Message::SelectionModeLabel)
	} else {
		editor.locale.translate(Message::ModeLabel(editor.mode.label().to_string()))
	};
	let mode_color = if editor.has_selection() {
		editor.theme.warning
	} else {
		editor.mode.color()
	};

	fragments.push(UiFragment {
		text: mode_text,
		fg: mode_color,
		bg: editor.theme.toolbar_bg,
		is_flex: false,
	});

	let name = editor.buffer().display_name();
	fragments.push(UiFragment {
		text: editor.locale.translate(Message::FilenameLabel(name)),
		fg: editor.theme.toolbar_fg,
		bg: editor.theme.toolbar_bg,
		is_flex: false,
	});

	if editor.buffer().dirty {
		fragments.push(UiFragment {
			text: editor.locale.translate(Message::DirtyFlag),
			fg: editor.theme.dirty_flag,
			bg: editor.theme.toolbar_bg,
			is_flex: false,
		});
	}

	if let Some(ref msg) = editor.status_msg {
		fragments.push(UiFragment {
			text: editor.locale.translate(Message::StatusMessage(msg.clone())),
			fg: editor.theme.dirty_flag,
			bg: editor.theme.toolbar_bg,
			is_flex: false,
		});
	}

	// Dynamic Flex padding pushing remaining elements cleanly to the right
	fragments.push(UiFragment {
		text: String::new(),
		fg: editor.theme.toolbar_bg,
		bg: editor.theme.toolbar_bg,
		is_flex: true,
	});

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
	
	fragments.push(UiFragment {
		text: right,
		fg: editor.theme.toolbar_fg_dim,
		bg: editor.theme.toolbar_bg,
		is_flex: false,
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
	
	let mut builder = OverlayBuilder::new(editor.theme.toolbar_bg, 0)
		.with_prefix(UiFragment {
			text: editor.locale.translate(Message::ToolbarPrefix),
			fg: editor.theme.status_bg,
			bg: editor.theme.toolbar_bg,
			is_flex: false,
		});

	builder.add_block(OverlayBlock {
		fragments: vec![UiFragment {
			text: editor.locale.translate(Message::HelpTitle),
			fg: editor.theme.help_label,
			bg: editor.theme.toolbar_bg,
			is_flex: false,
		}],
	});

	for (key, label) in &shortcuts {
		builder.add_block(OverlayBlock {
			fragments: vec![
				UiFragment {
					text: key.to_string(),
					fg: editor.theme.help_key,
					bg: editor.theme.toolbar_bg,
					is_flex: false,
				},
				UiFragment {
					text: format!(" {} ", label),
					fg: editor.theme.toolbar_fg,
					bg: editor.theme.toolbar_bg,
					is_flex: false,
				},
			],
		});
	}

	builder.add_block(OverlayBlock {
		fragments: vec![UiFragment {
			text: format!("  {}", editor.locale.translate(Message::Version(crate::VERSION.trim().to_string(), crate::GIT_HASH.to_string()))),
			fg: Color::DarkGrey,
			bg: editor.theme.toolbar_bg,
			is_flex: false,
		}],
	});

	builder.build(width, h.saturating_sub(2))
}

pub fn build_prompt(editor: &Editor, width: u16, h: u16) -> Option<Vec<Window>> {
	if width == 0 {
		return None;
	}

	let bg_col = toolbar_bg_color(editor);
	let mut builder = OverlayBuilder::new(bg_col, 10)
		.with_prefix(UiFragment {
			text: editor.locale.translate(Message::ToolbarPrefix),
			fg: editor.theme.status_bg,
			bg: bg_col,
			is_flex: false,
		});

	// Dynamic inputs require generic sliding viewport mapping 
	let label;
	let label_color;
	let query_text;
	let mut info_prefix = String::new();
	let mut info_color = Color::DarkGrey;
	let mut info_suffix = String::new();
	let text_cursor;

	match editor.mode {
		Mode::ReplacingSearch => {
			label = editor.locale.translate(Message::PromptReplaceTarget);
			label_color = Color::DarkMagenta;
			query_text = editor.replace_query.clone();
			text_cursor = editor.prompt_cursor;

			if editor.search_matches.is_empty() {
				if editor.replace_query.is_empty() {
					info_suffix = format!(" {} ", editor.locale.translate(Message::EscToClose));
				} else {
					info_prefix = editor.locale.translate(Message::ZeroMatches);
					info_color = Color::DarkYellow;
					info_suffix = editor.locale.translate(Message::ReplaceShortcuts);
				}
			} else {
				info_prefix = editor.locale.translate(Message::MatchFraction(editor.search_match_idx + 1, editor.search_matches.len()));
				info_color = Color::Yellow;
				info_suffix = editor.locale.translate(Message::ReplaceShortcuts);
			}
		}
		Mode::ReplacingWith => {
			label = editor.locale.translate(Message::PromptReplaceWith);
			label_color = Color::DarkMagenta;
			query_text = editor.replace_with.clone();
			text_cursor = editor.prompt_cursor;
		}
		Mode::GoToLine => {
			let total_lines = editor.buffers[editor.active_buffer].line_count();
			label = editor.locale.translate(Message::PromptGoToLine);
			label_color = Color::DarkCyan;
			query_text = editor.goto_line_input.clone();
			text_cursor = editor.prompt_cursor;
			info_suffix = editor.locale.translate(Message::PromptGoToLineHint(total_lines));
		}
		Mode::SaveAs | Mode::ConfirmOverwrite => {
			label = editor.locale.translate(Message::PromptSaveAs);
			label_color = Color::DarkGreen;
			query_text = editor.save_as_input.clone();
			text_cursor = editor.prompt_cursor;
			
			if editor.mode == Mode::ConfirmOverwrite {
				info_suffix = editor.locale.translate(Message::PromptConfirmOverwrite);
				info_color = Color::DarkRed;
			} else {
				info_suffix = editor.locale.translate(Message::PromptSaveAsShortcuts);
				info_color = Color::DarkGrey;
			}
		}
		Mode::Searching => {
			label = editor.locale.translate(Message::PromptSearch);
			label_color = Color::DarkYellow;
			query_text = editor.search_query.clone();
			text_cursor = editor.prompt_cursor;
			
			if editor.search_matches.is_empty() {
				if editor.search_query.is_empty() {
					info_suffix = format!(" {} ", editor.locale.translate(Message::EscToClose));
				} else {
					info_prefix = editor.locale.translate(Message::ZeroMatches);
					info_color = Color::DarkYellow;
					info_suffix = editor.locale.translate(Message::SearchShortcuts);
				}
			} else {
				info_prefix = editor.locale.translate(Message::MatchFraction(editor.search_match_idx + 1, editor.search_matches.len()));
				info_color = Color::Yellow;
				info_suffix = editor.locale.translate(Message::SearchReplaceShortcuts);
			}
		}
		Mode::ReplacingStep => {
			let label = editor.locale.translate(Message::PromptReplaceStep);
			builder.add_block(OverlayBlock {
				fragments: vec![
					UiFragment { bg: bg_col, fg: Color::DarkMagenta, text: label, is_flex: false },
				]
			});
			return Some(builder.build(width, h.saturating_sub(1)));
		}
		Mode::RecoverSwap => {
			let label = editor.locale.translate(Message::PromptRecoverTitle);
			let msg = editor.locale.translate(Message::PromptRecoverMsg);
			
			builder.add_block(OverlayBlock {
				fragments: vec![
					UiFragment { bg: bg_col, fg: Color::DarkRed, text: format!(" {} ", label), is_flex: false },
					UiFragment { bg: bg_col, fg: Color::DarkYellow, text: format!(" {} ", msg), is_flex: false },
				]
			});

			return Some(builder.build(width, h.saturating_sub(1)));
		}
		Mode::ConfirmQuit => {
			let label1 = editor.locale.translate(Message::PromptQuitWarning);
			let label2 = editor.locale.translate(Message::PromptQuitMsg);

			builder.add_block(OverlayBlock {
				fragments: vec![
					UiFragment { bg: bg_col, fg: Color::DarkRed, text: format!(" {} ", label1), is_flex: false },
					UiFragment { bg: bg_col, fg: Color::Blue, text: format!(" {} ", label2), is_flex: false },
				]
			});

			return Some(builder.build(width, h.saturating_sub(1)));
		}
		_ => return None,
	};

	let layout_width = (width as usize).saturating_sub(
		1 + label.chars().count() + info_prefix.chars().count() + info_suffix.chars().count() + 2 // padding
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

	let cursor_offset = label.chars().count() + 1 + screen_cursor_x + (if has_left { 1 } else { 0 });
	builder = builder.with_cursor(cursor_offset);

	let mut prompt_frags = vec![
		UiFragment { bg: bg_col, fg: label_color, text: label, is_flex: false },
		UiFragment { bg: bg_col, fg: Color::White, text: " ".to_string(), is_flex: false },
	];

	if has_left {
		prompt_frags.push(UiFragment { bg: bg_col, fg: editor.theme.toolbar_fg_dim, text: editor.locale.translate(Message::PromptClipLeft), is_flex: false });
	}

	prompt_frags.push(UiFragment { bg: bg_col, fg: Color::White, text: visible_slice, is_flex: false });
	
	if has_right {
		prompt_frags.push(UiFragment { bg: bg_col, fg: editor.theme.toolbar_fg_dim, text: editor.locale.translate(Message::PromptClipRight), is_flex: false });
	}
	
	prompt_frags.push(UiFragment { bg: bg_col, fg: Color::White, text: " ".to_string(), is_flex: false });

	if !info_prefix.is_empty() {
		prompt_frags.push(UiFragment { bg: bg_col, fg: info_color, text: info_prefix, is_flex: false });
	}
	if !info_suffix.is_empty() {
		prompt_frags.push(UiFragment { bg: bg_col, fg: Color::DarkGrey, text: info_suffix, is_flex: false });
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
			screen.bold = false;
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

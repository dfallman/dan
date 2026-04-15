import re

with open("src/render/chrome.rs", "r") as f:
    text = f.read()

start_idx = text.find("pub fn help_row_count")
end_idx = text.find("pub fn render_ui")

if start_idx == -1 or end_idx == -1:
    print("Could not find markers")
    exit(1)

new_code = """
use crate::ui::overlay::{OverlayBlock, OverlayBuilder};

pub fn build_help_bar(editor: &Editor, width: u16, h: u16) -> Vec<Window> {
	let shortcuts = help_shortcuts(editor);
	
	let mut builder = OverlayBuilder::new(editor.theme.toolbar_bg, 0)
		.with_prefix(UiFragment {
			text: editor.locale.translate(Message::ToolbarPrefix),
			fg: editor.theme.status_bg,
			bg: editor.theme.toolbar_bg,
			is_flex: false,
		})
		.with_trailing(UiFragment {
			text: editor.locale.translate(Message::Version(crate::VERSION.trim().to_string(), crate::GIT_HASH.to_string())),
			fg: editor.theme.text_main,
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

	match editor.mode {
		Mode::ReplacingSearch => {
			let label = " Replace find: ".to_string();
			let query_display = format!(" {} ", editor.replace_query);
			let (info_prefix, info_color, info_suffix) = if editor.search_matches.is_empty() {
				if editor.replace_query.is_empty() {
					(String::new(), Color::DarkGrey, " Esc to close ".to_string())
				} else {
					(" 0 matches".to_string(), Color::DarkYellow, ", ⏎ to replace, Esc to close ".to_string())
				}
			} else {
				(
					format!(" {}/{} matches", editor.search_match_idx + 1, editor.search_matches.len()),
					Color::Yellow,
					", ⏎ to replace, Esc to close ".to_string()
				)
			};

			let cursor_offset = label.chars().count() + 1 + editor.replace_query.chars().count();
			builder = builder.with_cursor(cursor_offset);

			builder.add_block(OverlayBlock {
				fragments: vec![
					UiFragment { bg: bg_col, fg: Color::DarkMagenta, text: label, is_flex: false },
					UiFragment { bg: bg_col, fg: Color::White, text: query_display, is_flex: false },
				]
			});

			if !info_prefix.is_empty() {
				builder.add_block(OverlayBlock {
					fragments: vec![
						UiFragment { bg: bg_col, fg: info_color, text: info_prefix, is_flex: false },
					]
				});
			}

			if !info_suffix.is_empty() {
				builder.add_block(OverlayBlock {
					fragments: vec![
						UiFragment { bg: bg_col, fg: Color::DarkGrey, text: info_suffix, is_flex: false },
					]
				});
			}

			Some(builder.build(width, h.saturating_sub(1)))
		}
		Mode::ReplacingWith => {
			let label = " Replace with: ".to_string();
			let query_display = format!(" {} ", editor.replace_with);
			let cursor_offset = label.chars().count() + 1 + editor.replace_with.chars().count();
			builder = builder.with_cursor(cursor_offset);

			builder.add_block(OverlayBlock {
				fragments: vec![
					UiFragment { bg: bg_col, fg: Color::DarkMagenta, text: label, is_flex: false },
					UiFragment { bg: bg_col, fg: Color::White, text: query_display, is_flex: false },
				]
			});

			Some(builder.build(width, h.saturating_sub(1)))
		}
		Mode::ReplacingStep => {
			let label = " Replace? (y)es, (n)o, (a)ll, (q)uit: ".to_string();
			builder.add_block(OverlayBlock {
				fragments: vec![
					UiFragment { bg: bg_col, fg: Color::DarkMagenta, text: label, is_flex: false },
				]
			});
			Some(builder.build(width, h.saturating_sub(1)))
		}
		Mode::Searching => {
			let label = " → ".to_string();
			let query_display = format!(" {} ", editor.search_query);
			let (info_prefix, info_color, info_suffix) = if editor.search_matches.is_empty() {
				if editor.search_query.is_empty() {
					(String::new(), Color::DarkGrey, " Esc to close ".to_string())
				} else {
					(" 0 matches".to_string(), Color::DarkYellow, ", ^G for next, ⏎ to select, Esc to close ".to_string())
				}
			} else {
				(
					format!(" {}/{} matches", editor.search_match_idx + 1, editor.search_matches.len()),
					Color::Yellow,
					", ^G for next, ^R to replace, ⏎ to select, Esc to close ".to_string()
				)
			};

			let cursor_offset = label.chars().count() + 1 + editor.search_query.chars().count();
			builder = builder.with_cursor(cursor_offset);

			builder.add_block(OverlayBlock {
				fragments: vec![
					UiFragment { bg: bg_col, fg: Color::DarkYellow, text: label, is_flex: false },
					UiFragment { bg: bg_col, fg: Color::White, text: query_display, is_flex: false },
				]
			});

			if !info_prefix.is_empty() {
				builder.add_block(OverlayBlock {
					fragments: vec![
						UiFragment { bg: bg_col, fg: info_color, text: info_prefix, is_flex: false },
					]
				});
			}

			if !info_suffix.is_empty() {
				builder.add_block(OverlayBlock {
					fragments: vec![
						UiFragment { bg: bg_col, fg: Color::DarkGrey, text: info_suffix, is_flex: false },
					]
				});
			}

			Some(builder.build(width, h.saturating_sub(1)))
		}
		Mode::GoToLine => {
			let label = " Go to line: ".to_string();
			let input_display = format!(" {} ", editor.goto_line_input);
			let total_lines = editor.buffer().line_count();
			let hint = format!(" (1-{}) ", total_lines);

			let cursor_offset = label.chars().count() + 1 + editor.goto_line_input.chars().count();
			builder = builder.with_cursor(cursor_offset);

			builder.add_block(OverlayBlock {
				fragments: vec![
					UiFragment { bg: bg_col, fg: Color::DarkCyan, text: label, is_flex: false },
					UiFragment { bg: bg_col, fg: Color::White, text: input_display, is_flex: false },
					UiFragment { bg: bg_col, fg: Color::Grey, text: hint, is_flex: false },
				]
			});

			Some(builder.build(width, h.saturating_sub(1)))
		}
		Mode::RecoverSwap => {
			let label = " Recovery ".to_string();
			let msg = " Swap file detected! Restore unsaved changes? (y)es, (n)o ".to_string();
			
			builder.add_block(OverlayBlock {
				fragments: vec![
					UiFragment { bg: bg_col, fg: Color::DarkRed, text: label, is_flex: false },
					UiFragment { bg: bg_col, fg: Color::DarkYellow, text: msg, is_flex: false },
				]
			});

			Some(builder.build(width, h.saturating_sub(1)))
		}
		Mode::SaveAs | Mode::ConfirmOverwrite => {
			let label = " Save as: ".to_string();
			let input_display = format!(" {} ", editor.save_as_input);
			let info = if editor.mode == Mode::ConfirmOverwrite {
				" File exists! ^O Overwrite, Esc Cancel ".to_string()
			} else {
				" type path, ⏎ Save, Esc Cancel ".to_string()
			};

			let cursor_offset = label.chars().count() + 1 + editor.save_as_cursor;
			builder = builder.with_cursor(cursor_offset);

			builder.add_block(OverlayBlock {
				fragments: vec![
					UiFragment { bg: bg_col, fg: Color::DarkGreen, text: label, is_flex: false },
					UiFragment { bg: bg_col, fg: Color::White, text: input_display, is_flex: false },
				]
			});

			if !info.is_empty() {
				builder.add_block(OverlayBlock {
					fragments: vec![
						UiFragment { bg: bg_col, fg: if editor.mode == Mode::ConfirmOverwrite { Color::DarkRed } else { Color::DarkGrey }, text: info, is_flex: false },
					]
				});
			}

			Some(builder.build(width, h.saturating_sub(1)))
		}
		Mode::ConfirmQuit => {
			let label1 = " Quit warning: ".to_string();
			let label2 = " Unsaved changes! (s)ave and quit, (f)orce quit, Esc to cancel ".to_string();

			builder.add_block(OverlayBlock {
				fragments: vec![
					UiFragment { bg: bg_col, fg: Color::DarkRed, text: label1, is_flex: false },
					UiFragment { bg: bg_col, fg: Color::Blue, text: label2, is_flex: false },
				]
			});

			Some(builder.build(width, h.saturating_sub(1)))
		}
		_ => None,
	}
}

"""

text = text[:start_idx] + new_code + text[end_idx:]

with open("src/render/chrome.rs", "w") as f:
    f.write(text)

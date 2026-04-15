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

/// Render the status bar.
pub fn render_status_bar(editor: &Editor, screen: &mut super::buffer::ScreenBuffer, vp: &Viewport) {
	let status_y = vp.height.saturating_sub(vp.chrome_rows);
	screen.mov_to(0, status_y);

	let width = vp.width as usize;
	let mut used: usize = 0;

	screen.reset_colors();
	screen.set_fg(Color::Blue);
	screen.put_str("▌");
	used += 1;

	// Mode indicator (derive visual mode from selection state)
	let (mode_color, mode_text) = if editor.has_selection() {
		(Color::DarkYellow, "Select")
	} else {
		(editor.mode.color(), editor.mode.label())
	};
	let mode_label = format!(" {} ", mode_text);
	screen.set_bg(toolbar_bg_color(editor));
	screen.set_fg(mode_color);
	screen.put_str(&mode_label);
	used += mode_label.chars().count();

	screen.set_bg(toolbar_bg_color(editor));
	screen.set_fg(Color::Black);

	// File name
	let name = editor.buffer().display_name();
	let name_part = format!(" {} ", name);
	
	screen.set_bg(toolbar_bg_color(editor));
	screen.set_fg(Color::White);
	screen.put_str(&name_part);
	used += name_part.chars().count();

	if editor.buffer().dirty {
		screen.set_fg(Color::DarkGrey);
		screen.put_str("+ ");
		screen.set_fg(Color::White);
		used += 2;
	}

	// Status message (if any)
	if let Some(ref msg) = editor.status_msg {
		let msg_part = format!(" {} ", msg);
		screen.set_bg(toolbar_bg_color(editor));
		screen.set_fg(Color::DarkGrey);
		screen.put_str(&msg_part);
		used += msg_part.chars().count();
	}

	// Right side: Language + Help toggle + cursor position
	let c = editor.cursors.cursor();
	let mut right_parts = Vec::new();

	if editor.config.show_help {
		right_parts.push("^H Help".to_string());
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
	right_parts.push(format!("Ln {:2}, Col {:2}", c.line + 1, c.col + 1));

	let right = format!(" {} ", right_parts.join("  "));
	let available = width.saturating_sub(used);
	
	if available > 0 {
		screen.set_fg(Color::DarkGrey);
		let right_len = right.chars().count();
		if available >= right_len {
			let padding = available - right_len;
			for _ in 0..padding {
				screen.put_char(' ');
			}
			screen.put_str(&right);
		} else {
			let truncated: String = right.chars().take(available).collect();
			screen.put_str(&truncated);
		}
		screen.set_fg(Color::White);
	}

	screen.reset_colors();
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
		("^F", "Find & replace"),
		("^G", "Goto"),
		("^D", "Duplicate"),
		("^K", "Delete"),
		("^W", "Wrap"),
		("^L", "Lint"),
		("^E", "Comment"),
		("^T", "Syntax highlight"),
		("^H", "Help"),
	]
}

/// Width of a single shortcut entry: key + " label ".
fn shortcut_width(key: &str, label: &str) -> usize {
	// key displayed, then " label " (space-label-space)
	key.chars().count() + 1 + label.chars().count() + 1
}

/// Calculate how many rows the help bar needs given a terminal width.
pub fn help_row_count(term_width: u16) -> u16 {
	let shortcuts = help_shortcuts();
	let width = term_width as usize;
	if width == 0 {
		return 1;
	}

	let help_label = " Help  ";

	// First row starts after the "▌" and " Help  " label.
	let mut rows: u16 = 1;
	let mut x = 1 + help_label.chars().count();
	let mut items_on_row = 0;

	for (key, label) in &shortcuts {
		let sw = shortcut_width(key, label);
		if x + sw > width && items_on_row > 0 {
			// Doesn't fit — start a new row.
			rows += 1;
			x = 1 + sw;
			items_on_row = 1;
		} else {
			x += sw;
			items_on_row += 1;
		}
	}
	rows
}

/// Render the pico-style help bar at the bottom of the screen.
/// Builds upward from the row above the status bar when it needs
/// multiple lines.
pub fn render_help_bar(editor: &Editor, screen: &mut super::buffer::ScreenBuffer, vp: &Viewport) {
	let shortcuts = help_shortcuts();
	let width = vp.width as usize;
	let num_rows = help_row_count(vp.width) as usize;

	// Layout shortcuts into rows.
	// Each row is a Vec of (key, label) pairs.
	let help_label = " Help  ";
	let mut rows: Vec<Vec<(&str, &str)>> = vec![Vec::new()];
	let mut x = 1 + help_label.chars().count(); // first row starts after "▌" and " Help  "

	for (key, label) in &shortcuts {
		let sw = shortcut_width(key, label);
		if x + sw > width && !rows.last().unwrap().is_empty() {
			rows.push(Vec::new());
			x = 1 + sw;
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
		screen.mov_to(0, y);

		let mut used: usize = 0;

		screen.reset_colors();
		screen.set_fg(Color::Blue);
		screen.put_str("▌");
		used += 1;

		if row_idx == 0 {
			// First row: " Help  " label in yellow (like mode labels)
			screen.set_bg(toolbar_bg_color(editor));
			screen.set_fg(Color::DarkYellow);
			screen.put_str(help_label);
			used += help_label.chars().count();
		}

		// Render shortcut items
		for (key, label) in row_items {
			screen.set_bg(toolbar_bg_color(editor));
			screen.set_fg(Color::DarkYellow);
			screen.put_str(key);
			screen.set_bg(toolbar_bg_color(editor));
			screen.set_fg(Color::White);
			let lbl = format!(" {} ", label);
			screen.put_str(&lbl);
			used += key.chars().count() + lbl.chars().count();
		}

		// On the last row, right-align the version string
		if row_idx == rows.len() - 1 {
			let version_str = format!("Dan v{} ({}) ", crate::VERSION.trim(), crate::GIT_HASH,);
			let version_str_len = version_str.chars().count();
			let available = width.saturating_sub(used);
			if available >= version_str_len {
				let remaining = available - version_str_len;
				if remaining > 0 {
					screen.set_bg(toolbar_bg_color(editor));
					for _ in 0..remaining {
						screen.put_char(' ');
					}
				}
				screen.set_bg(toolbar_bg_color(editor));
				screen.set_fg(Color::Black);
				screen.put_str(&version_str);
			} else {
				if available > 0 {
					screen.set_bg(toolbar_bg_color(editor));
					for _ in 0..available {
						screen.put_char(' ');
					}
				}
			}
		} else {
			// Pad remaining width with white background
			let remaining = width.saturating_sub(used);
			if remaining > 0 {
				screen.set_bg(toolbar_bg_color(editor));
				for _ in 0..remaining {
					screen.put_char(' ');
				}
			}
		}
	}

	screen.reset_colors();
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
	if width == 0 {
		return None;
	}
	let w = width as usize;

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

			let total =
				label.chars().count() + query_display.chars().count() + info_prefix.chars().count() + info_suffix.chars().count();
			let rows = ((total + w - 1) / w) as u16;
			let cursor_offset =
				(label.chars().count() + 1 + editor.replace_query.chars().count()) as u16;
			let current_len = label.chars().count() + query_display.chars().count();

			let mut blocks = vec![
				PromptBlock {
					bg: toolbar_bg_color(editor),
					fg: Color::DarkMagenta,
					text: label,
				},
				PromptBlock {
					bg: toolbar_bg_color(editor),
					fg: Color::White,
					text: query_display,
				},
			];
			if !info_prefix.is_empty() || !info_suffix.is_empty() {
				let eff_w = w.saturating_sub(1).max(1);
				let info_len = info_prefix.chars().count() + info_suffix.chars().count();
				let pad_len = eff_w.saturating_sub(current_len + info_len);
				if pad_len > 0 {
					blocks.push(PromptBlock {
						bg: toolbar_bg_color(editor),
						fg: Color::DarkGrey,
						text: " ".repeat(pad_len),
					});
				}
				if !info_prefix.is_empty() {
					blocks.push(PromptBlock {
						bg: toolbar_bg_color(editor),
						fg: info_color,
						text: info_prefix,
					});
				}
				if !info_suffix.is_empty() {
					blocks.push(PromptBlock {
						bg: toolbar_bg_color(editor),
						fg: Color::DarkGrey,
						text: info_suffix,
					});
				}
			}
			Some(PromptLayout {
				rows,
				cursor_offset,
				blocks,
			})
		}
		Mode::ReplacingWith => {
			let label = " Replace with: ".to_string();
			let query_display = format!(" {} ", editor.replace_with);

			let total = label.chars().count() + query_display.chars().count();
			let rows = ((total + w - 1) / w) as u16;
			let cursor_offset =
				(label.chars().count() + 1 + editor.replace_with.chars().count()) as u16;

			let blocks = vec![
				PromptBlock {
					bg: toolbar_bg_color(editor),
					fg: Color::DarkMagenta,
					text: label,
				},
				PromptBlock {
					bg: toolbar_bg_color(editor),
					fg: Color::White,
					text: query_display,
				},
			];
			Some(PromptLayout {
				rows,
				cursor_offset,
				blocks,
			})
		}
		Mode::ReplacingStep => {
			let label = " Replace? (y)es, (n)o, (a)ll, (q)uit: ".to_string();
			let rows = ((label.chars().count() + w - 1) / w) as u16;

			let blocks = vec![PromptBlock {
				bg: toolbar_bg_color(editor),
					fg: Color::DarkMagenta,
				text: label,
			}];
			Some(PromptLayout {
				rows,
				cursor_offset: 0,
				blocks,
			})
		}
		Mode::Searching => {
			let label = " → ".to_string();
			let query_display = format!(" {} ", editor.search_query);
			let (info_prefix, info_color, info_suffix) = if editor.search_matches.is_empty() {
				if editor.search_query.is_empty() {
					// String::new()
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

			let total =
				label.chars().count() + query_display.chars().count() + info_prefix.chars().count() + info_suffix.chars().count();
			let rows = ((total + w - 1) / w) as u16;
			let cursor_offset =
				(label.chars().count() + 1 + editor.search_query.chars().count()) as u16;
			let current_len = label.chars().count() + query_display.chars().count();

			let mut blocks = vec![
				PromptBlock {
					bg: toolbar_bg_color(editor),
					fg: Color::DarkYellow,
					text: label,
				},
				PromptBlock {
					bg: toolbar_bg_color(editor),
					fg: Color::White,
					text: query_display,
				},
			];
			if !info_prefix.is_empty() || !info_suffix.is_empty() {
				let eff_w = w.saturating_sub(1).max(1);
				let info_len = info_prefix.chars().count() + info_suffix.chars().count();
				let pad_len = eff_w.saturating_sub(current_len + info_len);
				if pad_len > 0 {
					blocks.push(PromptBlock {
						bg: toolbar_bg_color(editor),
						fg: Color::DarkGrey,
						text: " ".repeat(pad_len),
					});
				}
				if !info_prefix.is_empty() {
					blocks.push(PromptBlock {
						bg: toolbar_bg_color(editor),
						fg: info_color,
						text: info_prefix,
					});
				}
				if !info_suffix.is_empty() {
					blocks.push(PromptBlock {
						bg: toolbar_bg_color(editor),
						fg: Color::DarkGrey,
						text: info_suffix,
					});
				}
			}
			Some(PromptLayout {
				rows,
				cursor_offset,
				blocks,
			})
		}
		Mode::GoToLine => {
			let label = " Go to line: ".to_string();
			let input_display = format!(" {} ", editor.goto_line_input);
			let total_lines = editor.buffer().line_count();
			let hint = format!(" (1-{}) ", total_lines);

			let total =
				label.chars().count() + input_display.chars().count() + hint.chars().count();
			let rows = ((total + w - 1) / w) as u16;
			let cursor_offset =
				(label.chars().count() + 1 + editor.goto_line_input.chars().count()) as u16;

			let blocks = vec![
				PromptBlock {
					bg: toolbar_bg_color(editor),
					fg: Color::DarkCyan,
					text: label,
				},
				PromptBlock {
					bg: toolbar_bg_color(editor),
					fg: Color::White,
					text: input_display,
				},
				PromptBlock {
					bg: toolbar_bg_color(editor),
					fg: Color::Grey,
					text: hint,
				},
			];
			Some(PromptLayout {
				rows,
				cursor_offset,
				blocks,
			})
		}
		Mode::RecoverSwap => {
			let label = " Recovery ".to_string();
			let msg = " Swap file detected! Restore unsaved changes? (y)es, (n)o ".to_string();
			let total = label.chars().count() + msg.chars().count();
			let rows = ((total + w - 1) / w) as u16;
			let blocks = vec![
				PromptBlock {
					bg: toolbar_bg_color(editor),
					fg: Color::DarkRed,
					text: label,
				},
				PromptBlock {
					bg: toolbar_bg_color(editor),
					fg: Color::DarkYellow,
					text: msg,
				},
			];
			Some(PromptLayout {
				rows,
				cursor_offset: 0,
				blocks,
			})
		}
		Mode::SaveAs | Mode::ConfirmOverwrite => {
			let label = " Save as: ".to_string();
			let input_display = format!(" {} ", editor.save_as_input);
			let info = if editor.mode == Mode::ConfirmOverwrite {
				" File exists! ^O Overwrite, Esc Cancel ".to_string()
			} else {
				" type path, ⏎ Save, Esc Cancel ".to_string()
			};

			let current_len = label.chars().count() + input_display.chars().count();
			let total = current_len + info.chars().count();
			let rows = ((total + w - 1) / w) as u16;
			let cursor_offset = (label.chars().count() + 1 + editor.save_as_cursor) as u16;

			let mut blocks = vec![
				PromptBlock {
					bg: toolbar_bg_color(editor),
					fg: Color::DarkGreen,
					text: label,
				},
				PromptBlock {
					bg: toolbar_bg_color(editor),
					fg: Color::White,
					text: input_display,
				},
			];

			if !info.is_empty() {
				let eff_w = w.saturating_sub(1).max(1);
				let info_len = info.chars().count();
				let pad_len = eff_w.saturating_sub(current_len + info_len);
				if pad_len > 0 {
					blocks.push(PromptBlock {
						bg: toolbar_bg_color(editor),
						fg: Color::DarkGrey,
						text: " ".repeat(pad_len),
					});
				}
				blocks.push(PromptBlock {
					bg: toolbar_bg_color(editor),
					fg: if editor.mode == Mode::ConfirmOverwrite { Color::DarkRed } else { Color::DarkGrey },
					text: info,
				});
			}
			Some(PromptLayout {
				rows,
				cursor_offset,
				blocks,
			})
		}
		Mode::ConfirmQuit => {
			let label1 = " Quit warning: ".to_string();
			let label2 =
				" Unsaved changes! (s)ave and quit, (f)orce quit, Esc to cancel ".to_string();

			let total = label1.chars().count() + label2.chars().count();
			let rows = ((total + w - 1) / w) as u16;

			let blocks = vec![
				PromptBlock {
					bg: toolbar_bg_color(editor),
					fg: Color::DarkRed,
					text: label1,
				},
				PromptBlock {
					bg: toolbar_bg_color(editor),
					fg: Color::Blue,
					text: label2,
				},
			];
			Some(PromptLayout {
				rows,
				cursor_offset: 0,
				blocks,
			})
		}
		_ => None,
	}
}

/// Uniform renderer interpreting constructed runtime blocks natively.
pub fn render_prompt_overlay(
    screen: &mut super::buffer::ScreenBuffer,
    vp: &Viewport,
    layout: &PromptLayout,
) {
    if layout.rows == 0 {
        return;
    }
    let bar_y = vp.height.saturating_sub(1 + layout.rows);
    let width = vp.width as usize;

    let mut r: u16 = 0;
    screen.mov_to(0, bar_y + r);
    screen.reset_colors();
    screen.set_fg(Color::Blue);
    screen.put_str("▌");
    let mut x = 1;

    for block in &layout.blocks {
        screen.set_bg(block.bg);
        screen.set_fg(block.fg);
        for ch in block.text.chars() {
            if x >= width {
                r += 1;
                if r >= layout.rows { break; }
                screen.mov_to(0, bar_y + r);
                screen.reset_colors();
                screen.set_fg(Color::Blue);
                screen.put_str("▌");
                
                // restore colors for continuation
                screen.set_bg(block.bg);
                screen.set_fg(block.fg);
                x = 1;
            }
            screen.put_char(ch);
            x += 1;
        }
    }

    // Since prompt blocks now use toolbar_bg_color(editor), we can assume the whole remainder of the line 
    // should also be filled in with the prompt's background. We can peek at the first block's background,
    // which corresponds to the toolbar_bg_color correctly.
    let fill_bg = layout.blocks.first().map(|b| b.bg).unwrap_or(Color::DarkGrey);

    while r < layout.rows {
        screen.set_bg(fill_bg);
        while x < width {
            screen.put_char(' ');
            x += 1;
        }
        r += 1;
        if r < layout.rows {
            screen.mov_to(0, bar_y + r);
            screen.reset_colors();
            screen.set_fg(Color::Blue);
            screen.put_str("▌");
            x = 1;
        }
    }

    screen.reset_colors();
}

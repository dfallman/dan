//! Clipboard command handlers.
use crate::editor::commands::Command;
use crate::editor::Editor;

impl Editor {
	pub(crate) fn cmd_copy(&mut self) {
		if let Some(text) = self.get_selected_text() {
			if let Some(clip) = &mut self.sys_clipboard {
				let _ = clip.set_text(text.clone());
			}
			self.internal_clipboard = text;
			self.set_status("Copied");
		} else {
			// Copy current line if no selection
			let c = self.buffer().cursors.cursor();
			let text = self.buffer().text.line(c.line).to_string();
			if let Some(clip) = &mut self.sys_clipboard {
				let _ = clip.set_text(text.clone());
			}
			self.internal_clipboard = text;
			self.set_status("Copied line");
		}
	}

	pub(crate) fn cmd_cut(&mut self) {
		if self.has_selection() {
			if let Some(text) = self.get_selected_text() {
				if let Some(clip) = &mut self.sys_clipboard {
					let _ = clip.set_text(text.clone());
				}
				self.internal_clipboard = text;
			}
			self.delete_selection_if_active();
			self.set_status("Cut");
		} else {
			// Cut current line if no selection
			self.execute(Command::DeleteLine);
			self.set_status("Cut line");
		}
	}

	pub(crate) fn cmd_paste(&mut self) {
		// Skip if this was triggered by a Ctrl+V key event that
		// accompanied a bracketed paste we already handled.
		if self.suppress_next_paste {
			self.suppress_next_paste = false;
			return;
		}

		let mut text = String::new();
		if let Some(clip) = &mut self.sys_clipboard {
			if let Ok(sys_text) = clip.get_text() {
				if !sys_text.is_empty() {
					text = sys_text;
				}
			}
		}
		if text.is_empty() {
			text = self.internal_clipboard.clone();
		}

		if !text.is_empty() {
			self.paste_text(&text);
		}
	}

	/// Insert externally-sourced text into whichever field currently has
	/// focus (document buffer or a prompt). Always sanitizes at the paste
	/// boundary so clipboard content cannot inject terminal escapes.
	pub(crate) fn paste_text(&mut self, text: &str) {
		use crate::editor::mode::Mode;
		use crate::sanitize::sanitize_prompt_paste;

		match self.mode {
			Mode::Editing => {
				self.delete_selection_if_active();
				let pos = self.cursor_char_pos();
				let char_count = self.buffer_mut().insert_paste(pos, text);
				let new_pos = pos + char_count;
				let new_line = self.buffer().text.char_to_line(new_pos);
				let new_col = new_pos - self.buffer().text.line_to_char(new_line);
				self.buffer_mut().cursors.set_cursor(new_line, new_col);
				self.set_status("Pasted");
			}
			Mode::Searching => {
				let clean = sanitize_prompt_paste(text);
				if clean.is_empty() {
					return;
				}
				let mut chars: Vec<char> = self.search_query.chars().collect();
				for ch in clean.chars() {
					chars.insert(self.prompt_cursor, ch);
					self.prompt_cursor += 1;
				}
				self.search_query = chars.into_iter().collect();
				self.refresh_search_matches();
			}
			Mode::GoToLine => {
				let clean = sanitize_prompt_paste(text);
				for ch in clean.chars() {
					self.cmd_go_to_line_insert_char(ch);
				}
			}
			Mode::SaveAs => {
				let clean = sanitize_prompt_paste(text);
				if clean.is_empty() {
					return;
				}
				super::prompts::prompt_insert_str(
					&mut self.save_as_input,
					&mut self.prompt_cursor,
					&clean,
				);
			}
			Mode::ReplacingWith => {
				let clean = sanitize_prompt_paste(text);
				if clean.is_empty() {
					return;
				}
				super::prompts::prompt_insert_str(
					&mut self.replace_with,
					&mut self.prompt_cursor,
					&clean,
				);
			}
			Mode::Palette => {
				let clean = sanitize_prompt_paste(text);
				if clean.is_empty() {
					return;
				}
				self.ensure_project_indexer_started();
				self.palette.insert_str(&clean);
			}
			// Confirm / step dialogs: no text field — ignore.
			Mode::ConfirmQuit
			| Mode::ConfirmOverwrite
			| Mode::ReplacingStep
			| Mode::RecoverSwap => {}
		}
	}
}

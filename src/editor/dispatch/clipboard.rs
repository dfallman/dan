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
		self.delete_selection_if_active();

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
			let pos = self.cursor_char_pos();
			let char_count = self.buffer_mut().insert_paste(pos, &text);
			let new_pos = pos + char_count;
			let new_line = self.buffer().text.char_to_line(new_pos);
			let new_col = new_pos - self.buffer().text.line_to_char(new_line);
			self.buffer_mut().cursors.set_cursor(new_line, new_col);
			self.set_status("Pasted");
		}
	}
}

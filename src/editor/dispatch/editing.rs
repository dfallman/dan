//! Text editing, undo/redo command handlers.
use crate::editor::cursor::Cursor;
use crate::editor::Editor;

impl Editor {
	pub(crate) fn cmd_insert_char(&mut self, ch: char) {
		if self.config.auto_close && self.has_selection() {
			let pair = match ch {
				'{' => Some(('{', '}')),
				'[' => Some(('[', ']')),
				'(' => Some(('(', ')')),
				'"' => Some(('"', '"')),
				'\'' => Some(('\'', '\'')),
				'`' => Some(('`', '`')),
				_ => None,
			};
			if let Some((open, close)) = pair {
				// Wrap selection in the pair. Guard with Option even though
				// has_selection() was true — avoid panic if invariants regress.
				if let Some((start, end)) = self.selection_range() {
					let text = self.buffer().text.slice_to_string(start..end);
					self.buffer_mut().delete_range(start, end);

					let wrapped = format!("{}{}{}", open, text, close);
					self.buffer_mut().insert_str(start, &wrapped);

					// Char count, not byte length: `start` is a char offset.
					let new_end = start + wrapped.chars().count();
					let end_line = self.buffer().text.char_to_line(new_end);
					let end_col = new_end - self.buffer().text.line_to_char(end_line);

					let start_line = self.buffer().text.char_to_line(start);
					let start_col = start - self.buffer().text.line_to_char(start_line);

					self.buffer_mut().cursors.primary_mut().anchor = Cursor::new(start_line, start_col);
					self.buffer_mut().cursors.primary_mut().head = Cursor::new(end_line, end_col);
					return;
				}
			}
		}

		self.delete_selection_if_active();
		let pos = self.cursor_char_pos();

		let current_char = if pos < self.buffer().text.len_chars() {
			self.buffer().text.char_at(pos)
		} else {
			'\0'
		};

		// "Step over" existing closing punctuation instead of duplicating it
		if self.config.auto_close
			&& current_char != '\0'
			&& ch == current_char
			&& matches!(ch, '}' | ']' | ')' | '"' | '\'' | '`')
		{
			let line = self.buffer().text.char_to_line(pos + 1);
			let col = (pos + 1) - self.buffer().text.line_to_char(line);
			self.buffer_mut().cursors.set_cursor(line, col);
		} else {
			self.buffer_mut().insert_char(pos, ch);

			if self.config.auto_close {
				// Only insert quotes if followed by whitespace/closing-bracket/eof to avoid breaking valid inline syntax (like "don't")
				let should_close = match ch {
					'"' | '\'' | '`' => {
						current_char == '\0'
							|| current_char.is_whitespace()
							|| matches!(current_char, '}' | ']' | ')')
					}
					'{' | '[' | '(' => true,
					_ => false,
				};
				if should_close {
					let pair = match ch {
						'{' => '}',
						'[' => ']',
						'(' => ')',
						'"' => '"',
						'\'' => '\'',
						'`' => '`',
						_ => unreachable!(),
					};
					self.buffer_mut().insert_char(pos + 1, pair);
				}
			}

			let new_pos = pos + 1;
			let line = self.buffer().text.char_to_line(new_pos);
			let col = new_pos - self.buffer().text.line_to_char(line);
			self.buffer_mut().cursors.set_cursor(line, col);
		}
	}

	pub(crate) fn cmd_insert_string(&mut self, s: String) {
		self.delete_selection_if_active();
		if !s.is_empty() {
			let pos = self.cursor_char_pos();
			let char_count = self.buffer_mut().insert_paste(pos, &s);
			let new_pos = pos + char_count;
			let new_line = self.buffer().text.char_to_line(new_pos);
			let new_col = new_pos - self.buffer().text.line_to_char(new_line);
			self.buffer_mut().cursors.set_cursor(new_line, new_col);
		}
		// Suppress the Ctrl+V internal-paste that some terminals
		// send alongside the bracketed paste event.
		self.suppress_next_paste = true;
	}

	pub(crate) fn cmd_insert_newline(&mut self) {
		self.delete_selection_if_active();
		let c = self.buffer().cursors.cursor();
		let pos = self.cursor_char_pos();

		if self.config.auto_indent {
			// Collect leading whitespace from the current line, up to cursor col.
			let line_slice = self.buffer().text.line_slice(c.line);
			let mut indent = String::new();
			for (i, ch) in line_slice.chars().enumerate() {
				if i >= c.col {
					break;
				}
				if ch == ' ' || ch == '\t' {
					indent.push(ch);
				} else {
					break;
				}
			}
			// Insert "\n" + indent as a single operation (one undo step).
			let mut insertion = String::with_capacity(1 + indent.len());
			insertion.push('\n');
			insertion.push_str(&indent);
			self.buffer_mut().insert_str(pos, &insertion);
			self.buffer_mut().cursors.set_cursor(c.line + 1, indent.len());
		} else {
			self.buffer_mut().insert_char(pos, '\n');
			self.buffer_mut().cursors.set_cursor(c.line + 1, 0);
		}
	}

	pub(crate) fn cmd_insert_tab(&mut self) {
		let (start_c, end_c) = self.buffer_mut().cursors.primary().ordered();
		let mut end_line = end_c.line;

		// Standard IDE behavior: don't indent the last line if selection ends at column 0.
		if end_line > start_c.line && end_c.col == 0 {
			end_line -= 1;
		}

		if self.has_selection() {
			let tw = self.tab_width();
			let expand = self.expand_tab();
			let advance = if expand { tw } else { 1 };
			let spaces = " ".repeat(tw);
			let insert_str = if expand { &spaces } else { "\t" };

			for line_idx in (start_c.line..=end_line).rev() {
				let line_start = self.buffer().text.line_to_char(line_idx);
				self.buffer_mut().insert_str(line_start, insert_str);
			}
			self.buffer_mut().commit_edits();

			// Adjust selection columns
			let p = self.buffer_mut().cursors.primary_mut();
			if p.anchor.line >= start_c.line && p.anchor.line <= end_line {
				p.anchor.col += advance;
			}
			if p.head.line >= start_c.line && p.head.line <= end_line {
				p.head.col += advance;
			}
		} else {
			self.delete_selection_if_active();
			let pos = self.cursor_char_pos();
			let tw = self.tab_width();
			let advance = if self.expand_tab() {
				let spaces: String = " ".repeat(tw);
				self.buffer_mut().insert_str(pos, &spaces);
				tw
			} else {
				self.buffer_mut().insert_str(pos, "\t");
				1
			};
			let c = self.buffer().cursors.cursor();
			self.buffer_mut().cursors.set_cursor(c.line, c.col + advance);
		}
	}

	pub(crate) fn cmd_dedent(&mut self) {
		let (start_c, end_c) = self.buffer_mut().cursors.primary().ordered();
		let mut start_line = start_c.line;
		let mut end_line = end_c.line;

		if self.has_selection() && end_line > start_line && end_c.col == 0 {
			end_line -= 1;
		}

		if !self.has_selection() {
			start_line = self.buffer().cursors.cursor().line;
			end_line = start_line;
		}

		let tw = self.tab_width();
		let mut removals = Vec::new();

		for line_idx in (start_line..=end_line).rev() {
			let line_start = self.buffer().text.line_to_char(line_idx);
			let line_slice = self.buffer().text.line_slice(line_idx);

			let mut remove = 0usize;
			for ch in line_slice.chars() {
				if ch == '\t' && remove == 0 {
					remove = 1;
					break;
				} else if ch == ' ' && remove < tw {
					remove += 1;
				} else {
					break;
				}
			}

			if remove > 0 {
				self.buffer_mut()
					.delete_range(line_start, line_start + remove);
				removals.push((line_idx, remove));
			}
		}
		self.buffer_mut().commit_edits();

		// Adjust the selection's anchor/head columns to compensate
		// for the chars we just removed from each line.
		let p = self.buffer_mut().cursors.primary_mut();
		for (line_idx, remove) in removals {
			if p.anchor.line == line_idx {
				p.anchor.col = p.anchor.col.saturating_sub(remove);
			}
			if p.head.line == line_idx {
				p.head.col = p.head.col.saturating_sub(remove);
			}
		}
	}

	pub(crate) fn cmd_delete_backward(&mut self) {
		if self.has_selection() {
			self.delete_selection_if_active();
		} else {
			let c = self.buffer().cursors.cursor();
			let pos = self.cursor_char_pos();
			if pos > 0 {
				if c.col > 0 {
					// Auto-delete pairs mapping
					if self.config.auto_close && pos < self.buffer().text.len_chars() {
						let current_char = self.buffer().text.char_at(pos);
						let prev_char = self.buffer().text.char_at(pos - 1);
						let is_pair = match prev_char {
							'{' => current_char == '}',
							'[' => current_char == ']',
							'(' => current_char == ')',
							'"' => current_char == '"',
							'\'' => current_char == '\'',
							'`' => current_char == '`',
							_ => false,
						};
						if is_pair {
							// Auto-close pair: delete the closing char too.
							self.buffer_mut().delete_char(pos);
						}
					}

					// Deleting a char within the line — simple case
					self.buffer_mut().delete_char(pos - 1);
					self.buffer_mut().cursors.set_cursor(c.line, c.col - 1);
				} else if c.line > 0 {
					// At column 0: deleting the newline at end of previous line
					// to join lines. Capture prev line length BEFORE the delete.
					let prev_line = c.line - 1;
					let prev_len = self.line_len_no_newline(prev_line);
					self.buffer_mut().delete_char(pos - 1);
					self.buffer_mut().cursors.set_cursor(prev_line, prev_len);
				}
			}
		}
	}

	pub(crate) fn cmd_delete_forward(&mut self) {
		if self.has_selection() {
			self.delete_selection_if_active();
		} else {
			let pos = self.cursor_char_pos();
			if pos < self.buffer().text.len_chars() {
				self.buffer_mut().delete_char(pos);
			}
		}
	}

	pub(crate) fn cmd_delete_line(&mut self) {
		self.clear_selection();
		let c = self.buffer().cursors.cursor();
		let line_start = self.buffer().text.line_to_char(c.line);
		let line_end = if c.line + 1 < self.buffer().line_count() {
			self.buffer().text.line_to_char(c.line + 1)
		} else {
			self.buffer().text.len_chars()
		};
		if line_start < line_end {
			let deleted = self.buffer().text.slice_to_string(line_start..line_end);
			if let Some(clip) = &mut self.sys_clipboard {
				let _ = clip.set_text(deleted.clone());
			}
			self.internal_clipboard = deleted;
			self.buffer_mut().delete_range(line_start, line_end);
			let max_line = self.buffer().line_count().saturating_sub(1);
			let new_line = c.line.min(max_line);
			self.buffer_mut().cursors.set_cursor(new_line, 0);
			self.set_status("Line deleted");
		}
	}

	pub(crate) fn cmd_duplicate_line_or_selection(&mut self) {
		if let Some((start, end)) = self.selection_range() {
			if start < end {
				let text = self.buffer().text.slice_to_string(start..end);
				// Duplicate the selected text right after the selection.
				self.clear_selection();
				self.buffer_mut().insert_str(end, &text);
				// Place cursor at the end of the inserted duplicate.
				// Char count, not byte length: `end` is a char offset.
				let new_pos = end + text.chars().count();
				let line = self.buffer().text.char_to_line(new_pos);
				let line_start = self.buffer().text.line_to_char(line);
				let col = new_pos - line_start;
				self.buffer_mut().cursors.set_cursor(line, col);
				self.set_status("Selection duplicated");
				return;
			}
		}
		// No (non-empty) selection — duplicate the current line.
		let c = self.buffer().cursors.cursor();
		let line_start = self.buffer().text.line_to_char(c.line);
		let line_end = if c.line + 1 < self.buffer().line_count() {
			self.buffer().text.line_to_char(c.line + 1)
		} else {
			self.buffer().text.len_chars()
		};
		let line_text = self.buffer().text.slice_to_string(line_start..line_end);
		// If the line doesn't end with newline (last line), prepend one.
		let insert_text = if line_text.ends_with('\n') {
			line_text
		} else {
			format!("\n{}", line_text)
		};
		self.buffer_mut().insert_str(line_end, &insert_text);
		// Move cursor to the same column on the new duplicate line.
		self.buffer_mut().cursors.set_cursor(c.line + 1, c.col);
		self.set_status("Line duplicated");
	}

	pub(crate) fn cmd_undo(&mut self) {
		self.clear_selection();
		self.buffer_mut().undo();
		self.clamp_cursors();
	}

	pub(crate) fn cmd_redo(&mut self) {
		self.clear_selection();
		self.buffer_mut().redo();
		self.clamp_cursors();
	}
}

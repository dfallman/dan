//! Command dispatch — `Editor::execute` and the giant match on `Command`.
//!
//! Extracted from `editor/mod.rs` so that the dispatch logic and the editor
//! state definition stay in separate files (M8). Pure code move; no
//! behavior change. Imports mirror what the match arms reference.

use super::Editor;
use super::commands::Command;
use super::cursor::Cursor;
use super::mode::Mode;

impl Editor {
	/// Execute a command.
	pub fn execute(&mut self, cmd: Command) {
		let action = match &cmd {
			Command::InsertChar(ch) if ch.is_whitespace() || ch.is_ascii_punctuation() => crate::editor::commands::EditAction::Whitespace,
			Command::InsertChar(_) | Command::InsertTab | Command::Paste | Command::Dedent | Command::InsertString(_) => crate::editor::commands::EditAction::Insert,
			Command::InsertNewline => crate::editor::commands::EditAction::Whitespace,
			Command::DeleteBackward | Command::DeleteForward | Command::DeleteLine | Command::Cut => crate::editor::commands::EditAction::Delete,
			_ => crate::editor::commands::EditAction::Other,
		};

		if action != self.last_edit_action || action == crate::editor::commands::EditAction::Other {
			self.buffer_mut().commit_edits();
		}
		self.last_edit_action = action;

		match cmd {
			// -- Motion (clears selection) --
			Command::MoveLeft => {
				self.move_cursor_horizontal(-1);
				self.clear_selection();
			}
			Command::MoveRight => {
				self.move_cursor_horizontal(1);
				self.clear_selection();
			}
			Command::MoveUp => {
				self.move_cursor_vertical(-1);
				self.clear_selection();
			}
			Command::MoveDown => {
				self.move_cursor_vertical(1);
				self.clear_selection();
			}
			Command::MoveLineStart => {
				self.buffer_mut().cursors.primary_mut().head.set_col(0);
				self.clear_selection();
			}
			Command::MoveLineEnd => {
				let c = self.buffer().cursors.cursor();
				let len = self.line_len_no_newline(c.line);
				self.buffer_mut().cursors.primary_mut().head.set_col(len);
				self.buffer_mut().cursors.primary_mut().head.desired_vcol =
					crate::editor::visual_col::visual_col_at(
						self.buffer().text.line_slice(c.line).chars(),
						len,
						self.tab_width(),
					);
				self.clear_selection();
			}
			Command::MoveWordForward => {
				self.move_word_forward();
				self.clear_selection();
			}
			Command::MoveWordBackward => {
				self.move_word_backward();
				self.clear_selection();
			}
			Command::SwapLineUp => {
				if self.has_selection() {
					self.move_lines_up();
				} else {
					self.swap_line_up();
					self.clear_selection();
				}
			}
			Command::SwapLineDown => {
				if self.has_selection() {
					self.move_lines_down();
				} else {
					self.swap_line_down();
					self.clear_selection();
				}
			}
			Command::MoveBufferTop => {
				self.buffer_mut().cursors.primary_mut().head.line = 0;
				self.buffer_mut().cursors.primary_mut().head.set_col(0);
				self.clear_selection();
			}
			Command::MoveBufferBottom => {
				let last_line = self.buffer().line_count().saturating_sub(1);
				self.buffer_mut().cursors.primary_mut().head.line = last_line;
				self.buffer_mut().cursors.primary_mut().head.set_col(0);
				self.clear_selection();
			}
			Command::PageUp => {
				// Scroll by visible text area height (terminal height minus status + command bars)
				let page = (self.terminal_height as usize).saturating_sub(2).max(1);
				for _ in 0..page {
					self.move_cursor_vertical(-1);
				}
				self.clear_selection();
			}
			Command::PageDown => {
				let page = (self.terminal_height as usize).saturating_sub(2).max(1);
				for _ in 0..page {
					self.move_cursor_vertical(1);
				}
				self.clear_selection();
			}
			Command::ScrollViewportUp => {
				let new_scroll_y = self.buffer().scroll_y.saturating_sub(1);
				self.buffer_mut().scroll_y = new_scroll_y;
				let visible_height = self.terminal_height.saturating_sub(2) as usize;
				let cursor_line = self.buffer().cursors.cursor().line;
				// Maintain VSCode-style viewport tether: push cursor back up if it would fall out of the bottom bound
				if cursor_line
					>= self.buffer_mut().scroll_y + visible_height.saturating_sub(self.config.scroll_off)
				{
					self.move_cursor_vertical(-1);
				}
				self.clear_selection();
			}
			Command::ScrollViewportDown => {
				self.buffer_mut().scroll_y += 1;
				let cursor_line = self.buffer().cursors.cursor().line;
				// Maintain VSCode-style viewport tether: pull cursor down if it would fall out of the top bound
				if cursor_line < self.buffer_mut().scroll_y + self.config.scroll_off {
					self.move_cursor_vertical(1);
				}
				self.clear_selection();
			}
			Command::MoveFastUp => {
				for _ in 0..self.config.fast_scroll_steps {
					self.move_cursor_vertical(-1);
				}
				self.clear_selection();
			}
			Command::MoveFastDown => {
				for _ in 0..self.config.fast_scroll_steps {
					self.move_cursor_vertical(1);
				}
				self.clear_selection();
			}

			// -- Selection (shift+arrows) --
			Command::SelectLeft => {
				self.begin_selection_if_needed();
				self.move_cursor_horizontal(-1);
			}
			Command::SelectRight => {
				self.begin_selection_if_needed();
				self.move_cursor_horizontal(1);
			}
			Command::SelectUp => {
				self.begin_selection_if_needed();
				self.move_cursor_vertical(-1);
			}
			Command::SelectDown => {
				self.begin_selection_if_needed();
				self.move_cursor_vertical(1);
			}
			Command::SelectWordForward => {
				self.begin_selection_if_needed();
				self.move_word_forward();
			}
			Command::SelectWordBackward => {
				self.begin_selection_if_needed();
				self.move_word_backward();
			}
			Command::SelectLineStart => {
				self.begin_selection_if_needed();
				self.buffer_mut().cursors.primary_mut().head.set_col(0);
			}
			Command::SelectLineEnd => {
				self.begin_selection_if_needed();
				let c = self.buffer().cursors.cursor();
				let len = self.line_len_no_newline(c.line);
				let tab_w = self.tab_width();
				let vcol = crate::editor::visual_col::visual_col_at(
					self.buffer().text.line_slice(c.line).chars(),
					len,
					tab_w,
				);
				self.buffer_mut().cursors.primary_mut().head.set_col(len);
				self.buffer_mut().cursors.primary_mut().head.desired_vcol = vcol;
			}
			Command::SelectAll => {
				let last_line = self.buffer().line_count().saturating_sub(1);
				let last_col = self.line_len_no_newline(last_line);
				// Set anchor at start of buffer, head at end.
				use crate::editor::cursor::Cursor;
				self.buffer_mut().cursors.primary_mut().anchor = Cursor::new(0, 0);
				self.buffer_mut().cursors.primary_mut().head = Cursor::new(last_line, last_col);
			}

			// -- Editing --
			Command::InsertChar(ch) => {
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
			Command::InsertString(ref s) => {
				self.delete_selection_if_active();
				if !s.is_empty() {
					let pos = self.cursor_char_pos();
					let char_count = self.buffer_mut().insert_paste(pos, s);
					let new_pos = pos + char_count;
					let new_line = self.buffer().text.char_to_line(new_pos);
					let new_col = new_pos - self.buffer().text.line_to_char(new_line);
					self.buffer_mut().cursors.set_cursor(new_line, new_col);
				}
				// Suppress the Ctrl+V internal-paste that some terminals
				// send alongside the bracketed paste event.
				self.suppress_next_paste = true;
			}
			Command::InsertNewline => {
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
			Command::InsertTab => {
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
			Command::Dedent => {
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
			Command::DeleteBackward => {
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
			Command::DeleteForward => {
				if self.has_selection() {
					self.delete_selection_if_active();
				} else {
					let pos = self.cursor_char_pos();
					if pos < self.buffer().text.len_chars() {
						self.buffer_mut().delete_char(pos);
					}
				}
			}
			Command::DeleteLine => {
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
			Command::DuplicateLineOrSelection => {
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

			// -- Undo / Redo --
			Command::Undo => {
				self.clear_selection();
				self.buffer_mut().undo();
				self.clamp_cursors();
			}
			Command::Redo => {
				self.clear_selection();
				self.buffer_mut().redo();
				self.clamp_cursors();
			}

			// -- Clipboard (GUI-style) --
			Command::Copy => {
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
			Command::Cut => {
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
			Command::Paste => {
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

			// -- Global Replace --
			Command::ReplaceInsertChar(ch) => {
				if self.mode == Mode::ReplacingWith {
					prompt_insert_char(&mut self.replace_with, self.prompt_cursor, ch);
					self.prompt_cursor += 1;
				}
			}
			Command::ReplaceDeleteChar => {
				if self.mode == Mode::ReplacingWith
					&& self.prompt_cursor > 0 {
						self.prompt_cursor -= 1;
						prompt_remove_char(&mut self.replace_with, self.prompt_cursor);
					}
			}
			Command::ReplaceWithConfirm => {
				if self.buffer().search_matches.is_empty() {
					self.mode = Mode::Editing;
					self.search_query.clear();
					self.clear_status();
				} else {
					self.mode = Mode::ReplacingStep;
					self.jump_to_search_match();
				}
			}
			Command::ReplaceActionYes => {
				let current = {
					let buf = self.buffer();
					buf.search_matches.get(buf.search_match_idx).copied()
				};
				if let Some((start, end)) = current {
					let replacement = self.replace_with.clone();
					self.buffer_mut().commit_edits(); // wrap
					self.buffer_mut().delete_range(start, end);
					self.buffer_mut().insert_str(start, &replacement);
					self.buffer_mut().commit_edits();

					// Char count, not byte length: `start` is a char offset.
					let new_pos = start + replacement.chars().count();
					let line = self.buffer().text.char_to_line(new_pos);
					let col = new_pos - self.buffer().text.line_to_char(line);
					self.buffer_mut().search_saved_cursor = Some((line, col));
					self.buffer_mut().cursors.set_cursor(line, col);
					self.refresh_search_matches();

					if self.buffer().search_matches.is_empty() {
						self.mode = Mode::Editing;
						self.search_query.clear();
						self.clear_status();
					} else {
						// match idx is implicitly resync'd via refresh geometry bounding to the nearest next item naturally
						self.jump_to_search_match();
					}
				} else {
					self.mode = Mode::Editing;
				}
			}
			Command::ReplaceActionNo => {
				if !self.buffer().search_matches.is_empty() {
					let len = self.buffer().search_matches.len();
					self.buffer_mut().search_match_idx = (self.buffer().search_match_idx + 1) % len;
					self.jump_to_search_match();
				} else {
					self.mode = Mode::Editing;
				}
			}
			Command::ReplaceActionAll => {
				self.buffer_mut().commit_edits(); // Explicit history block grouping
				let replacement = self.replace_with.clone();

				// Iterate end-to-start so each replace leaves earlier match
				// offsets intact. Start from search_match_idx so already-skipped
				// (`n`-answered) matches stay untouched.
				let pending_matches = {
					let buf = self.buffer();
					buf.search_matches[buf.search_match_idx..].to_vec()
				};
				for &(start, end) in pending_matches.iter().rev() {
					self.buffer_mut().delete_range(start, end);
					self.buffer_mut().insert_str(start, &replacement);
				}

				self.buffer_mut().commit_edits();
				self.clamp_cursors();
				self.mode = Mode::Editing;
				self.search_query.clear();
				self.buffer_mut().search_matches.clear();
				self.buffer_mut().search_match_idx = 0;
				self.clear_status();
			}
			Command::ReplaceCancel => {
				if !self.replace_query.is_empty() {
					let q = self.replace_query.clone();
					self.buffer_mut().last_search_query = q;
				}
				if let Some((line, col)) = self.buffer_mut().search_saved_cursor.take() {
					self.buffer_mut().cursors.set_cursor(line, col);
				}
				self.search_query.clear();
				self.replace_query.clear();
				self.replace_with.clear();
				self.buffer_mut().search_matches.clear();
				self.buffer_mut().search_match_idx = 0;
				self.mode = Mode::Editing;
				self.clear_status();
			}

			// -- Search --
			Command::SearchForward => {
				// Enter search mode — save the current cursor position.
				self.clear_selection();
				let c = self.buffer().cursors.cursor();
				self.buffer_mut().search_saved_cursor = Some((c.line, c.col));
				// Pre-fill with the last search query so re-opening search
				// immediately shows previous results.
				self.search_query = self.buffer().last_search_query.clone();
				self.buffer_mut().search_matches.clear();
				self.buffer_mut().search_match_idx = 0;
				self.mode = Mode::Searching;
				self.prompt_cursor = self.search_query.chars().count();
				self.prompt_view_start.set(0);
				// If we have a previous query, run the search immediately.
				if !self.search_query.is_empty() {
					self.refresh_search_matches();
				}
			}
			Command::SearchInsertChar(ch) => {
				// Insert by char index (not byte index) so multibyte input works.
				let mut chars: Vec<char> = self.search_query.chars().collect();
				chars.insert(self.prompt_cursor, ch);
				self.search_query = chars.into_iter().collect();
				self.prompt_cursor += 1;

				self.refresh_search_matches();
			}
			Command::SearchDeleteChar => {
				if self.prompt_cursor > 0 {
					self.prompt_cursor -= 1;
					let mut chars: Vec<char> = self.search_query.chars().collect();
					chars.remove(self.prompt_cursor);
					self.search_query = chars.into_iter().collect();
					self.refresh_search_matches();
				}
			}
			Command::SearchConfirm => {
				// Accept the current match — exit search, select matched text.
				if !self.search_query.is_empty() {
					let q = self.search_query.clone();
					self.buffer_mut().last_search_query = q;
				}
				let current = {
					let buf = self.buffer();
					buf.search_matches.get(buf.search_match_idx).copied()
				};
				if let Some((start, end)) = current {
					// Set anchor at the end of the match, head at the start
					// so the matched text is selected.
					use crate::editor::cursor::Cursor;
					let line = self.buffer().text.char_to_line(start);
					let col = start - self.buffer().text.line_to_char(line);
					let end_line = self.buffer().text.char_to_line(end);
					let end_col = end - self.buffer().text.line_to_char(end_line);
					self.buffer_mut().cursors.primary_mut().anchor = Cursor::new(end_line, end_col);
					self.buffer_mut().cursors.primary_mut().head = Cursor::new(line, col);
				}
				self.mode = Mode::Editing;
				self.search_query.clear();
				self.buffer_mut().search_matches.clear();
				self.buffer_mut().search_match_idx = 0;
				self.buffer_mut().search_saved_cursor = None;
				self.clear_status();
			}
			Command::SearchCancel => {
				// Restore cursor to its pre-search position.
				if !self.search_query.is_empty() {
					let q = self.search_query.clone();
					self.buffer_mut().last_search_query = q;
				}
				if let Some((line, col)) = self.buffer_mut().search_saved_cursor.take() {
					self.buffer_mut().cursors.set_cursor(line, col);
				}
				self.search_query.clear();
				self.buffer_mut().search_matches.clear();
				self.buffer_mut().search_match_idx = 0;
				self.mode = Mode::Editing;
				self.clear_status();
			}
			Command::SearchConvertToReplace => {
				if !self.buffer().search_matches.is_empty() {
					self.replace_query = self.search_query.clone();
					let q = self.search_query.clone();
					self.buffer_mut().last_search_query = q;
					self.mode = Mode::ReplacingWith;
					self.prompt_cursor = self.replace_with.chars().count();
					self.prompt_view_start.set(0);
				}
			}
			Command::SearchNext => {
				if !self.buffer().search_matches.is_empty() {
					let len = self.buffer().search_matches.len();
					self.buffer_mut().search_match_idx = (self.buffer().search_match_idx + 1) % len;
					self.jump_to_search_match();
				}
			}
			Command::SearchPrev => {
				if !self.buffer().search_matches.is_empty() {
					let idx = self.buffer().search_match_idx;
					let len = self.buffer().search_matches.len();
					self.buffer_mut().search_match_idx = if idx == 0 { len - 1 } else { idx - 1 };
					self.jump_to_search_match();
				}
			}

			// -- Go-to-line --
			Command::GoToLineOpen => {
				self.clear_selection();
				self.goto_line_input.clear();
				self.prompt_cursor = 0;
				self.mode = Mode::GoToLine;
			}
			Command::GoToLineInsertChar(ch) => {
				if ch.is_ascii_digit() {
					prompt_insert_char(&mut self.goto_line_input, self.prompt_cursor, ch);
					self.prompt_cursor += 1;
				}
			}
			Command::GoToLineDeleteChar => {
				if self.prompt_cursor > 0 {
					self.prompt_cursor -= 1;
					prompt_remove_char(&mut self.goto_line_input, self.prompt_cursor);
				}
			}
			Command::GoToLineConfirm => {
				if let Ok(n) = self.goto_line_input.parse::<usize>() {
					let target = if n == 0 { 0 } else { n - 1 }; // 1-indexed to 0-indexed
					let max_line = self.buffer().line_count().saturating_sub(1);
					let line = target.min(max_line);
					self.buffer_mut().cursors.set_cursor(line, 0);
					self.set_status(format!("Jumped to line {}", line + 1));
				}
				self.goto_line_input.clear();
				self.prompt_cursor = 0;
				self.mode = Mode::Editing;
			}
			Command::GoToLineCancel => {
				self.goto_line_input.clear();
				self.prompt_cursor = 0;
				self.mode = Mode::Editing;
				self.clear_status();
			}

			// -- Save As --
			Command::SaveAsOpen => {
				// Pre-populate with current file path if one exists.
				self.save_as_input = self
					.buffer()
					.file_path
					.as_ref()
					.map(|p| p.to_string_lossy().to_string())
					.unwrap_or_default();
				self.prompt_cursor = self.save_as_input.chars().count();
				self.mode = Mode::SaveAs;
			}
			Command::SaveAsInsertChar(ch) => {
				prompt_insert_char(&mut self.save_as_input, self.prompt_cursor, ch);
				self.prompt_cursor += 1;
			}
			Command::SaveAsDeleteChar => {
				if self.prompt_cursor > 0 {
					self.prompt_cursor -= 1;
					prompt_remove_char(&mut self.save_as_input, self.prompt_cursor);
				}
			}
			Command::PromptCursorLeft => {
				if self.prompt_cursor > 0 {
					self.prompt_cursor -= 1;
				}
			}
			Command::PromptCursorRight => {
				let max_len = match self.mode {
					Mode::Searching => self.search_query.chars().count(),
					Mode::ReplacingWith => self.replace_with.chars().count(),
					Mode::GoToLine => self.goto_line_input.chars().count(),
					Mode::SaveAs | Mode::ConfirmOverwrite => self.save_as_input.chars().count(),
					_ => 0,
				};
				if self.prompt_cursor < max_len {
					self.prompt_cursor += 1;
				}
			}
			Command::SaveAsConfirm => {
				let path_str = self.save_as_input.clone();
				if path_str.is_empty() {
					self.save_as_input.clear();
					self.prompt_cursor = 0;
					self.mode = Mode::Editing;
					self.set_status("Save as cancelled: no path given");
				} else {
					let path = std::path::Path::new(&path_str);
					// Check if parent directory exists.
					if let Some(parent) = path.parent() {
						if !parent.as_os_str().is_empty() && !parent.exists() {
							self.set_status(format!(
								"Directory does not exist: {}",
								parent.display()
							));
							return;
						}
					}
					// Check if file already exists — ask for overwrite confirmation.
					if path.exists() {
						self.save_as_pending_path = Some(path_str);
						self.mode = Mode::ConfirmOverwrite;
					} else {
						// New file — save directly.
						self.save_as_input.clear();
						self.prompt_cursor = 0;
						self.buffer_mut().commit_edits();
						let cfg = self.config.clone();
						match self.buffer_mut().save_to(path, &cfg) {
							Ok(()) => {
								self.set_status(format!("✓ Saved as {}", path.display()));
								if self.quit_cycle_idx.is_some() {
									self.advance_quit_cycle();
								} else {
									self.mode = Mode::Editing;
								}
							}
							Err(e) => {
								// Drop any in-flight quit cycle: the save failed,
								// so we are not advancing through dirty buffers.
								self.quit_cycle_idx = None;
								self.mode = Mode::Editing;
								self.set_status(format!("Save failed: {}", e));
							}
						}
					}
				}
			}
			Command::SaveAsCancel => {
				self.save_as_input.clear();
				self.prompt_cursor = 0;
				if self.quit_cycle_idx.is_some() {
					self.quit_cycle_idx = None;
				}
				self.mode = Mode::Editing;
				self.clear_status();
			}

			// -- Overwrite confirmation --
			Command::ConfirmOverwrite => {
				if let Some(path_str) = self.save_as_pending_path.take() {
					let path = std::path::Path::new(&path_str);
					self.save_as_input.clear();
					self.prompt_cursor = 0;
					self.buffer_mut().commit_edits();
					let cfg = self.config.clone();
					match self.buffer_mut().save_to(path, &cfg) {
						Ok(()) => {
							self.set_status(format!("✓ Saved as {}", path_str));
							if self.quit_cycle_idx.is_some() {
								self.advance_quit_cycle();
							} else {
								self.mode = Mode::Editing;
							}
						}
						Err(e) => {
							// Same as SaveAsConfirm error path — clear the cycle.
							self.quit_cycle_idx = None;
							self.mode = Mode::Editing;
							self.set_status(format!("Save failed: {}", e));
						}
					}
				}
			}
			Command::CancelOverwrite => {
				self.save_as_pending_path = None;
				self.mode = Mode::SaveAs;
			}

			// -- File --
			Command::Save => {
				if self.buffer().file_path.is_none() {
					self.execute(Command::SaveAsOpen);
				} else {
					self.buffer_mut().commit_edits();
					let cfg = self.config.clone();
					match self.buffer_mut().save(&cfg) {
						Ok(()) => self.set_status("✓ Saved"),
						Err(e) => self.set_status(format!("Save failed: {}", e)),
					}
				}
			}
			Command::Quit => {
				// Find the first dirty buffer and prompt for it; if none, exit.
				let dirty_idx = self.buffers.iter().position(|b| b.dirty);
				match dirty_idx {
					None => self.should_quit = true,
					Some(i) => {
						self.active_buffer = i;
						self.quit_cycle_idx = Some(i);
						self.mode = Mode::ConfirmQuit;
					}
				}
			}
			Command::ForceQuit => {
				// Discard the active buffer's dirty state and advance the cycle.
				// Remove its swap too so the discarded content isn't offered for
				// recovery on the next open (P4-M).
				if let Some(swp) = self.buffer().swp_path.clone() {
					crate::recovery::cleanup_swap(&swp);
				}
				self.buffer_mut().dirty = false;
				self.advance_quit_cycle();
			}
			Command::ForceQuitAll => {
				// Unconditional exit — no questions, no cycle, no save.
				self.should_quit = true;
			}
			Command::SaveAndQuit => {
				if self.buffer().file_path.is_none() {
					self.execute(Command::SaveAsOpen);
				} else {
					self.buffer_mut().commit_edits();
					let cfg = self.config.clone();
					match self.buffer_mut().save(&cfg) {
						Ok(()) => self.advance_quit_cycle(),
						Err(e) => {
							// Leave mode as ConfirmQuit so the user can retry or cancel.
							self.set_status(format!("Save failed: {}", e));
						}
					}
				}
			}
			Command::CancelQuit => {
				self.quit_cycle_idx = None;
				self.mode = Mode::Editing;
				self.clear_status();
			}

			Command::ToggleHelp => {
				self.show_help = !self.show_help;
			}

			Command::ToggleWrap => {
				self.config.wrap_lines = !self.config.wrap_lines;
				self.buffer_mut().scroll_vrow = 0;
			}

			Command::ToggleWhitespace => {
				self.config.show_whitespace = !self.config.show_whitespace;
				let status = if self.config.show_whitespace {
					"Whitespace markers on"
				} else {
					"Whitespace markers off"
				};
				self.set_status(status);
			}

			Command::FormatDocument => {
				// Don't stack formatter runs: a second spawn would orphan the
				// first worker + child and overwrite fmt_rx (P3-J).
				if self.buffer().is_formatting {
					return;
				}
				let ext_str = self
					.buffer()
					.file_path
					.as_ref()
					.and_then(|p| p.extension())
					.and_then(|s| s.to_str())
					.unwrap_or("js")
					.to_string();

				let content = self.buffer().text.to_string_full();
				let (tx, rx) = std::sync::mpsc::channel();
				let child_pid = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
				crate::editor::formatter::spawn_formatter(
					ext_str,
					content,
					tx,
					std::sync::Arc::clone(&child_pid),
				);

				let baseline = self.buffer().version;
				let buf = self.buffer_mut();
				buf.fmt_rx = Some(rx);
				buf.fmt_child_pid = Some(child_pid);
				buf.fmt_baseline_version = Some(baseline);
				buf.is_formatting = true;
				self.set_status("Formatting...");
			}
			Command::RecoverSwapAccept => {
				if let Some(swp) = self.buffer().swp_path.clone() {
					if let Some(payload) = crate::recovery::check_recovery(&swp) {
						let len = self.buffer().text.len_chars();
						self.buffer_mut().delete_range(0, len);
						self.buffer_mut().insert_str(0, &payload);
						self.buffer_mut().mark_mutated();
					}
					crate::recovery::cleanup_swap(&swp);
				}
				self.mode = Mode::Editing;
				self.clear_status();
			}
			Command::RecoverSwapDecline => {
				if let Some(swp) = self.buffer().swp_path.clone() {
					crate::recovery::cleanup_swap(&swp);
				}
				self.mode = Mode::Editing;
				self.clear_status();
			}
			Command::ToggleComment => {
				self.toggle_comment();
			}
			Command::ToggleSyntax => {
				self.config.syntax_highlight = !self.config.syntax_highlight;
				let status = if self.config.syntax_highlight {
					"Syntax highlighting enabled"
				} else {
					"Syntax highlighting disabled"
				};
				self.set_status(status);
			}
			// -- Palette --
			Command::PaletteOpen => {
				// Only open from plain Editing — don't clobber another prompt.
				if matches!(self.mode, Mode::Editing) {
					self.mode = Mode::Palette;
					self.open_palette();
				}
			}
			Command::PaletteCancel => {
				if self.palette.close_prompt_idx.is_some() {
					self.palette.close_prompt_idx = None;
				} else {
					self.palette.close();
					self.mode = Mode::Editing;
				}
			}
			Command::PaletteInsertChar(ch) => {
				self.ensure_project_indexer_started();
				self.palette.insert_char(ch);
			}
			Command::PaletteDeleteChar => self.palette.delete_char(),
			Command::PaletteUp => self.palette.move_up(),
			Command::PaletteDown => {
				let rows = palette_visible_rows(self);
				self.palette.move_down(rows);
			}
			Command::PalettePageUp => {
				let rows = palette_visible_rows(self);
				self.palette.page_up(rows);
			}
			Command::PalettePageDown => {
				let rows = palette_visible_rows(self);
				self.palette.page_down(rows);
			}
			Command::PaletteConfirm => {
				// Clone the selected item before dispatching so we don't hold
				// an immutable borrow of `self.palette` while mutating self.
				let item = self.palette.selected_item().cloned();
				match item {
					Some(crate::palette::PaletteItem::Action { id, .. }) => {
						self.palette.close();
						self.mode = Mode::Editing;
						let cmd = crate::palette::action_to_command(id);
						self.execute(cmd);
					}
					Some(crate::palette::PaletteItem::Buffer { idx, .. }) => {
						self.palette.close();
						self.mode = Mode::Editing;
						if idx < self.buffers.len() {
							self.active_buffer = idx;
						}
					}
					Some(crate::palette::PaletteItem::File { path, .. }) => {
						self.palette.close();
						self.mode = Mode::Editing;
						match self.open_file(&path) {
							Ok(()) => {
								// open_file already records the recent entry on a
								// fresh load. Calling push_recent_file again here
								// also promotes already-open files to the top.
								self.push_recent_file(&path);
							}
							Err(e) => {
								self.set_status(format!(
									"Could not open {}: {}",
									path.display(),
									e
								));
							}
						}
					}
					None => {
						self.palette.close();
						self.mode = Mode::Editing;
					}
				}
			}
			Command::PaletteCloseBuffer => {
				if let Some(idx) = self.palette.close_prompt_idx {
					// Already in sub-mode → Ctrl-D means Discard
					self.buffers[idx].dirty = false;
					let _ = self.close_buffer(idx);
					self.palette.close_prompt_idx = None;
					self.open_palette();
				} else {
					let item = self.palette.selected_item().cloned();
					match item {
						Some(crate::palette::PaletteItem::Buffer { idx, dirty, .. }) => {
							if dirty {
								self.palette.close_prompt_idx = Some(idx);
							} else {
								let _ = self.close_buffer(idx);
								self.open_palette(); // rebuild items
							}
						}
						Some(crate::palette::PaletteItem::File { path, .. }) => {
							self.recent_files.retain(|r| r.path != path);
							self.recent_files_dirty = true;
							self.open_palette();
						}
						_ => { /* Action selection: silent no-op */ }
					}
				}
			}
			Command::PaletteClosePromptSave => {
				if let Some(idx) = self.palette.close_prompt_idx {
					self.active_buffer = idx;
					let cfg = self.config.clone();
					if let Err(e) = self.buffer_mut().save(&cfg) {
						self.set_status(format!("Save failed: {}", e));
						return;
					}
					let _ = self.close_buffer(idx);
					self.palette.close_prompt_idx = None;
					self.open_palette();
				}
			}
			#[allow(dead_code)]
			Command::PaletteClosePromptDiscard => {
				// Triggered if user maps a separate key — kept for completeness.
				if let Some(idx) = self.palette.close_prompt_idx {
					self.buffers[idx].dirty = false;
					let _ = self.close_buffer(idx);
					self.palette.close_prompt_idx = None;
					self.open_palette();
				}
			}
			Command::PaletteClosePromptCancel => {
				self.palette.close_prompt_idx = None;
			}

			Command::NewBuffer => {
				// If the active buffer is already an empty, clean, unpathed
				// scratch, just stay on it — no point in two empty scratches.
				let active = &self.buffers[self.active_buffer];
				if active.file_path.is_none()
					&& !active.dirty
					&& active.text.len_chars() == 0
				{
					let name = active.display_name();
					self.set_status(format!("{} is already empty", name));
					return;
				}
				// Otherwise, drop the auto-created startup scratch (if it's
				// still pristine somewhere else) so the user doesn't end up
				// with an [Untitled] they didn't ask for, then push a new one.
				self.maybe_dispose_startup_scratch();
				self.push_new_untitled();
				let name = self.buffer().display_name();
				self.set_status(format!("Created {}", name));
			}
			Command::OpenFilePicker => {
				// Same as PaletteOpen but pre-filtered? For now, just open the palette.
				self.mode = crate::editor::mode::Mode::Palette;
				self.open_palette();
			}
			Command::ReloadBuffer => {
				let path = match self.buffer().file_path.clone() {
					Some(p) => p,
					None => { self.set_status("Cannot reload [Untitled]"); return; }
				};
				if self.buffer().dirty {
					self.set_status("Buffer has unsaved changes — save or force-reload (not implemented)");
					return;
				}
				match crate::buffer::Buffer::from_file(&path) {
					Ok((mut new_buf, _et, _tw)) => {
						// Preserve crash-recovery coverage across reload (P0-1):
						// from_file leaves swp_path None.
						new_buf.swp_path = Some(crate::recovery::get_swap_path(&path));
						self.buffers[self.active_buffer] = new_buf;
						self.set_status(format!("Reloaded {}", path.display()));
					}
					Err(e) => self.set_status(format!("Reload failed: {}", e)),
				}
			}
			Command::CloseBuffer => {
				let idx = self.active_buffer;
				if self.buffer().dirty {
					self.set_status("Buffer has unsaved changes — save first or use palette Ctrl-D for prompt");
					return;
				}
				let _ = self.close_buffer(idx);
			}
			Command::CloseOthers => {
				// Check dirty status BEFORE mutating the vector — the previous
				// swap_remove-then-restore approach silently reordered buffers
				// on the abort path (the keeper got push()-ed back at the end
				// rather than restored to its original index).
				let active = self.active_buffer;
				if self.buffers.iter().enumerate().any(|(i, b)| i != active && b.dirty) {
					self.set_status("Other buffers have unsaved changes; aborting");
					return;
				}
				let keeper = self.buffers.swap_remove(active);
				self.buffers = vec![keeper];
				self.active_buffer = 0;
				self.set_status("Closed other buffers");
			}
			Command::CloseAll => {
				for b in &self.buffers {
					if b.dirty {
						self.set_status("Some buffers have unsaved changes; save first or use Quit");
						return;
					}
				}
				self.buffers.clear();
				let mut scratch = crate::buffer::Buffer::new();
				scratch.untitled_seq = Some(1);
				scratch.swp_path = Some(crate::recovery::untitled_swap_path(1));
				self.buffers.push(scratch);
				self.active_buffer = 0;
			}
			Command::SaveAll => {
				let mut ok = 0; let mut fail = 0; let mut last_err = String::new();
				let cfg = self.config.clone();
				for i in 0..self.buffers.len() {
					if !self.buffers[i].dirty { continue; }
					if self.buffers[i].file_path.is_none() { continue; }
					// Save by temporarily switching active.
					let prev = self.active_buffer;
					self.active_buffer = i;
					match self.buffer_mut().save(&cfg) {
						Ok(_) => ok += 1,
						Err(e) => { fail += 1; last_err = e.to_string(); }
					}
					self.active_buffer = prev;
				}
				if fail == 0 {
					self.set_status(format!("Saved {} buffer(s)", ok));
				} else {
					self.set_status(format!("Saved {}; {} failed: {}", ok, fail, last_err));
				}
			}
			// -- Path / Metadata --
			Command::CopyPathAbs => {
				match self.buffer().file_path.clone() {
					Some(p) => {
						let s = std::fs::canonicalize(&p).unwrap_or(p).display().to_string();
						if let Ok(mut cb) = arboard::Clipboard::new() {
							let _ = cb.set_text(s.clone());
							self.set_status(format!("Copied: {}", s));
						} else {
							self.set_status("Clipboard unavailable");
						}
					}
					None => self.set_status("No file path to copy"),
				}
			}
			Command::CopyPathRel => {
				match self.buffer().file_path.clone() {
					Some(p) => {
						let s = p.strip_prefix(&self.project_root).unwrap_or(&p).display().to_string();
						if let Ok(mut cb) = arboard::Clipboard::new() {
							let _ = cb.set_text(s.clone());
							self.set_status(format!("Copied: {}", s));
						} else {
							self.set_status("Clipboard unavailable");
						}
					}
					None => self.set_status("No file path to copy"),
				}
			}
			Command::RevealInFinder => {
				let Some(p) = self.buffer().file_path.clone() else {
					self.set_status("No file to reveal"); return;
				};
				let result = if cfg!(target_os = "macos") {
					std::process::Command::new("open").args(["-R", &p.display().to_string()]).spawn()
				} else if cfg!(target_os = "linux") {
					let parent = p.parent().unwrap_or(std::path::Path::new("."));
					std::process::Command::new("xdg-open").arg(parent).spawn()
				} else if cfg!(target_os = "windows") {
					std::process::Command::new("explorer").args(["/select,", &p.display().to_string()]).spawn()
				} else {
					self.set_status("Reveal not supported on this platform"); return;
				};
				match result {
					Ok(_) => self.set_status("Revealed in file manager"),
					Err(e) => self.set_status(format!("Reveal failed: {}", e)),
				}
			}
			Command::OpenContainingFolder => {
				let Some(p) = self.buffer().file_path.clone() else {
					self.set_status("No file to open folder for"); return;
				};
				let parent = p.parent().unwrap_or(std::path::Path::new(".")).to_owned();
				let cmd = if cfg!(target_os = "macos") { "open" }
				          else if cfg!(target_os = "windows") { "explorer" }
				          else { "xdg-open" };
				match std::process::Command::new(cmd).arg(&parent).spawn() {
					Ok(_) => self.set_status(format!("Opened {}", parent.display())),
					Err(e) => self.set_status(format!("Open failed: {}", e)),
				}
			}
			Command::ShowBufferInfo => {
				let path = self.buffer().file_path.as_ref()
					.map(|p| p.display().to_string())
					.unwrap_or_else(|| "[Untitled]".into());
				let lines = self.buffer().text.len_lines();
				let bytes = self.buffer().text.to_string_full().len();
				let enc = self.buffer().encoding.name();
				let eol = self.config.end_of_line.as_deref().unwrap_or("auto");
				self.set_status(format!("{} · {} lines · {} bytes · {} · EOL: {}", path, lines, bytes, enc, eol));
			}

			// -- Format / Encoding --
			Command::IndentSpaces => {
				self.config.expand_tab = true;
				self.set_status("Indent: spaces");
			}
			Command::IndentTabs => {
				self.config.expand_tab = false;
				self.set_status("Indent: tabs");
			}
			Command::TabWidth(w) => {
				self.config.tab_width = w;
				self.set_status(format!("Tab width: {}", w));
			}
			Command::LineEndingsLF => {
				self.config.end_of_line = Some("lf".into());
				self.buffer_mut().dirty = true;
				self.set_status("Line endings: LF");
			}
			Command::LineEndingsCRLF => {
				self.config.end_of_line = Some("crlf".into());
				self.buffer_mut().dirty = true;
				self.set_status("Line endings: CRLF");
			}
			Command::TrimTrailingWhitespaceNow => {
				let text = self.buffer().text.to_string_full();
				let trimmed: String = text
					.lines()
					.map(|l| l.trim_end_matches([' ', '\t']))
					.collect::<Vec<_>>()
					.join("\n");
				let final_text = if text.ends_with('\n') {
					trimmed + "\n"
				} else {
					trimmed
				};
				let len = self.buffer().text.len_chars();
				self.buffer_mut().delete_range(0, len);
				self.buffer_mut().insert_str(0, &final_text);
				self.buffer_mut().commit_edits();
				self.clamp_cursors();
				self.set_status("Trimmed trailing whitespace");
			}
			Command::ConvertTabsToSpaces => {
				// Convert ONLY leading-whitespace tabs — touching mid-line tabs
				// (e.g. column-aligned data, doc-comments) would silently
				// corrupt formatting.
				let tw = self.tab_width();
				let spaces = " ".repeat(tw);
				let text = self.buffer().text.to_string_full();
				let out: String = text
					.split('\n')
					.map(|line| {
						let leading_end = line
							.find(|c: char| c != '\t' && c != ' ')
							.unwrap_or(line.len());
						let leading: String = line[..leading_end]
							.chars()
							.map(|c| if c == '\t' { spaces.clone() } else { c.to_string() })
							.collect();
						format!("{}{}", leading, &line[leading_end..])
					})
					.collect::<Vec<_>>()
					.join("\n");
				let len = self.buffer().text.len_chars();
				self.buffer_mut().delete_range(0, len);
				self.buffer_mut().insert_str(0, &out);
				self.buffer_mut().commit_edits();
				self.clamp_cursors();
				self.set_status(format!("Converted leading tabs to {} spaces", tw));
			}
			Command::ConvertSpacesToTabs => {
				// Same constraint as the inverse: only collapse leading-whitespace
				// space-runs, never anything mid-line.
				let tw = self.tab_width();
				let spaces = " ".repeat(tw);
				let text = self.buffer().text.to_string_full();
				let out: String = text
					.split('\n')
					.map(|line| {
						let leading_end = line
							.find(|c: char| c != ' ' && c != '\t')
							.unwrap_or(line.len());
						let leading_spaces = &line[..leading_end];
						let tabs = leading_spaces.replace(&spaces, "\t");
						format!("{}{}", tabs, &line[leading_end..])
					})
					.collect::<Vec<_>>()
					.join("\n");
				let len = self.buffer().text.len_chars();
				self.buffer_mut().delete_range(0, len);
				self.buffer_mut().insert_str(0, &out);
				self.buffer_mut().commit_edits();
				self.clamp_cursors();
				self.set_status("Converted leading spaces to tabs");
			}

			Command::SortLinesAsc => sort_lines(self, false),
			Command::SortLinesDesc => sort_lines(self, true),
			Command::DedupAdjacent => {
				let text = self.buffer().text.to_string_full();
				let mut last: Option<String> = None;
				let mut out = String::with_capacity(text.len());
				for line in text.split_inclusive('\n') {
					let trimmed = line.trim_end_matches('\n').to_string();
					if Some(&trimmed) != last.as_ref() {
						out.push_str(line);
						last = Some(trimmed);
					}
				}
				let len = self.buffer().text.len_chars();
				self.buffer_mut().delete_range(0, len);
				self.buffer_mut().insert_str(0, &out);
				self.buffer_mut().commit_edits();
				self.clamp_cursors();
				self.set_status("Deduplicated adjacent lines");
			}
			Command::ConvertUpper => transform_text(self, |s| s.to_uppercase()),
			Command::ConvertLower => transform_text(self, |s| s.to_lowercase()),
			Command::ConvertTitle => transform_text(self, |s| {
				s.split_whitespace()
					.map(|w| {
						let mut c = w.chars();
						match c.next() {
							None => String::new(),
							Some(f) => {
								f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()
							}
						}
					})
					.collect::<Vec<_>>()
					.join(" ")
			}),
			Command::ReverseSelection => {
				let range = self.selection_range();
				let (s, e) = match range {
					Some(r) => r,
					None => {
						let line = self.buffer().cursors.cursor().line;
						let line_start = self.buffer().text.line_to_char(line);
						let line_len = self.buffer().text.line_len_chars(line);
						(line_start, line_start + line_len.saturating_sub(1))
					}
				};
				let span = self.buffer().text.slice_to_string(s..e);
				let reversed: String = span.chars().rev().collect();
				self.buffer_mut().delete_range(s, e);
				self.buffer_mut().insert_str(s, &reversed);
				self.buffer_mut().commit_edits();
				self.clamp_cursors();
				self.set_status("Reversed");
			}

			Command::ToggleLineNumbers => {
				self.config.line_numbers = !self.config.line_numbers;
				self.set_status(if self.config.line_numbers { "Line numbers on" } else { "Line numbers off" });
			}
			Command::ReloadConfiguration => {
				self.config = crate::config::Config::load();
				if let Some(p) = self.buffer().file_path.clone() {
					self.config.apply_editorconfig(&p);
				}
				self.set_status("Configuration reloaded");
			}
			Command::ShowRecentFiles => {
				self.mode = crate::editor::mode::Mode::Palette;
				self.open_palette();
				// Override the items: just buffers + recent, no actions.
				let items: Vec<crate::palette::PaletteItem> = self.palette.all_items.iter()
					.filter(|i| matches!(i, crate::palette::PaletteItem::Buffer { .. } | crate::palette::PaletteItem::File { .. }))
					.cloned().collect();
				self.palette.all_items = items;
				self.palette.refilter();
			}
			Command::ShowVersion => {
				self.set_status(format!("dan {} ({})", crate::VERSION.trim(), crate::GIT_HASH));
			}

			Command::Noop => {}
		}
	}
}

/// Number of result rows the palette modal shows at once — divider rows
/// included. Must match `render::chrome::build_palette_window`'s `visible_rows`
/// (`min(terminal_height - 4, 20) - 6`) so the scroll math keeps the selected
/// item on screen; the previous hardcoded `14` over-reported on terminals
/// shorter than 24 rows.
fn palette_visible_rows(editor: &crate::editor::Editor) -> usize {
	let max_height = editor.terminal_height.saturating_sub(4).min(20);
	(max_height as usize).saturating_sub(6).max(1)
}

/// Insert `ch` into `s` at **char** index `char_idx` (0..=char_count). The
/// editor's `prompt_cursor` is a char position; `String::insert` takes a byte
/// index, and using the char index directly panics on a non-boundary
/// (`is_char_boundary`) the moment any multibyte char precedes the cursor
/// (P1-B / P1-C). This translates char→byte first.
fn prompt_insert_char(s: &mut String, char_idx: usize, ch: char) {
	let byte_idx = s
		.char_indices()
		.nth(char_idx)
		.map(|(b, _)| b)
		.unwrap_or(s.len());
	s.insert(byte_idx, ch);
}

/// Remove the char at **char** index `char_idx` from `s`. No-op if out of
/// range. Char-index counterpart to `String::remove` (which takes bytes).
fn prompt_remove_char(s: &mut String, char_idx: usize) {
	if let Some((b, _)) = s.char_indices().nth(char_idx) {
		s.remove(b);
	}
}

fn sort_lines(editor: &mut crate::editor::Editor, descending: bool) {
	let text = editor.buffer().text.to_string_full();
	let mut lines: Vec<&str> = text.split('\n').collect();
	let trailing_nl = text.ends_with('\n');
	if trailing_nl {
		lines.pop();
	}
	if descending {
		lines.sort_by(|a, b| b.cmp(a));
	} else {
		lines.sort();
	}
	let mut out = lines.join("\n");
	if trailing_nl {
		out.push('\n');
	}
	let len = editor.buffer().text.len_chars();
	editor.buffer_mut().delete_range(0, len);
	editor.buffer_mut().insert_str(0, &out);
	editor.buffer_mut().commit_edits();
	editor.clamp_cursors();
	editor.set_status(if descending {
		"Sorted lines descending"
	} else {
		"Sorted lines ascending"
	});
}

fn transform_text(editor: &mut crate::editor::Editor, f: impl Fn(&str) -> String) {
	let (s, e) = match editor.selection_range() {
		Some(r) => r,
		None => (0, editor.buffer().text.len_chars()),
	};
	let span = editor.buffer().text.slice_to_string(s..e);
	let new = f(&span);
	editor.buffer_mut().delete_range(s, e);
	editor.buffer_mut().insert_str(s, &new);
	editor.buffer_mut().commit_edits();
	editor.clamp_cursors();
}

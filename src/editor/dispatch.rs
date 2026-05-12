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
				self.cursors.primary_mut().head.set_col(0);
				self.clear_selection();
			}
			Command::MoveLineEnd => {
				let c = self.cursors.cursor();
				let len = self.line_len_no_newline(c.line);
				self.cursors.primary_mut().head.set_col(len);
				self.cursors.primary_mut().head.desired_vcol =
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
				self.cursors.primary_mut().head.line = 0;
				self.cursors.primary_mut().head.set_col(0);
				self.clear_selection();
			}
			Command::MoveBufferBottom => {
				let last_line = self.buffer().line_count().saturating_sub(1);
				self.cursors.primary_mut().head.line = last_line;
				self.cursors.primary_mut().head.set_col(0);
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
				self.scroll_y = self.scroll_y.saturating_sub(1);
				let visible_height = self.terminal_height.saturating_sub(2) as usize;
				let cursor_line = self.cursors.cursor().line;
				// Maintain VSCode-style viewport tether: push cursor back up if it would fall out of the bottom bound
				if cursor_line
					>= self.scroll_y + visible_height.saturating_sub(self.config.scroll_off)
				{
					self.move_cursor_vertical(-1);
				}
				self.clear_selection();
			}
			Command::ScrollViewportDown => {
				self.scroll_y += 1;
				let cursor_line = self.cursors.cursor().line;
				// Maintain VSCode-style viewport tether: pull cursor down if it would fall out of the top bound
				if cursor_line < self.scroll_y + self.config.scroll_off {
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
				self.cursors.primary_mut().head.set_col(0);
			}
			Command::SelectLineEnd => {
				self.begin_selection_if_needed();
				let c = self.cursors.cursor();
				let len = self.line_len_no_newline(c.line);
				let tab_w = self.tab_width();
				let vcol = crate::editor::visual_col::visual_col_at(
					self.buffer().text.line_slice(c.line).chars(),
					len,
					tab_w,
				);
				self.cursors.primary_mut().head.set_col(len);
				self.cursors.primary_mut().head.desired_vcol = vcol;
			}
			Command::SelectAll => {
				let last_line = self.buffer().line_count().saturating_sub(1);
				let last_col = self.line_len_no_newline(last_line);
				// Set anchor at start of buffer, head at end.
				use crate::editor::cursor::Cursor;
				self.cursors.primary_mut().anchor = Cursor::new(0, 0);
				self.cursors.primary_mut().head = Cursor::new(last_line, last_col);
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
						// Functional wrapper logic: extract selection, delete it symmetrically, then substitute wrapped
						let (start, end) = self.selection_range().unwrap();
						let text = self.buffer().text.slice_to_string(start..end);
						self.buffer_mut().delete_range(start, end);

						let wrapped = format!("{}{}{}", open, text, close);
						self.buffer_mut().insert_str(start, &wrapped);

						let new_end = start + wrapped.len();
						let end_line = self.buffer().text.char_to_line(new_end);
						let end_col = new_end - self.buffer().text.line_to_char(end_line);

						let start_line = self.buffer().text.char_to_line(start);
						let start_col = start - self.buffer().text.line_to_char(start_line);

						self.cursors.primary_mut().anchor = Cursor::new(start_line, start_col);
						self.cursors.primary_mut().head = Cursor::new(end_line, end_col);
						return;
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
					self.cursors.set_cursor(line, col);
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
					self.cursors.set_cursor(line, col);
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
					self.cursors.set_cursor(new_line, new_col);
				}
				// Suppress the Ctrl+V internal-paste that some terminals
				// send alongside the bracketed paste event.
				self.suppress_next_paste = true;
			}
			Command::InsertNewline => {
				self.delete_selection_if_active();
				let c = self.cursors.cursor();
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
					self.cursors.set_cursor(c.line + 1, indent.len());
				} else {
					self.buffer_mut().insert_char(pos, '\n');
					self.cursors.set_cursor(c.line + 1, 0);
				}
			}
			Command::InsertTab => {
				let (start_c, end_c) = self.cursors.primary().ordered();
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
					let p = self.cursors.primary_mut();
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
					let c = self.cursors.cursor();
					self.cursors.set_cursor(c.line, c.col + advance);
				}
			}
			Command::Dedent => {
				let (start_c, end_c) = self.cursors.primary().ordered();
				let mut start_line = start_c.line;
				let mut end_line = end_c.line;

				if self.has_selection() && end_line > start_line && end_c.col == 0 {
					end_line -= 1;
				}

				if !self.has_selection() {
					start_line = self.cursors.cursor().line;
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
				let p = self.cursors.primary_mut();
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
					let c = self.cursors.cursor();
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
							self.cursors.set_cursor(c.line, c.col - 1);
						} else if c.line > 0 {
							// At column 0: deleting the newline at end of previous line
							// to join lines. Capture prev line length BEFORE the delete.
							let prev_line = c.line - 1;
							let prev_len = self.line_len_no_newline(prev_line);
							self.buffer_mut().delete_char(pos - 1);
							self.cursors.set_cursor(prev_line, prev_len);
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
				let c = self.cursors.cursor();
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
					self.cursors.set_cursor(new_line, 0);
					self.set_status("Line deleted");
				}
			}
			Command::DuplicateLineOrSelection => {
				if let Some(text) = self.get_selected_text() {
					// Duplicate the selected text right after the selection.
					let (_, end) = self.selection_range().unwrap();
					self.clear_selection();
					self.buffer_mut().insert_str(end, &text);
					// Place cursor at the end of the inserted duplicate.
					let new_pos = end + text.len();
					let line = self.buffer().text.char_to_line(new_pos);
					let line_start = self.buffer().text.line_to_char(line);
					let col = new_pos - line_start;
					self.cursors.set_cursor(line, col);
					self.set_status("Selection duplicated");
				} else {
					// No selection — duplicate the current line.
					let c = self.cursors.cursor();
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
					self.cursors.set_cursor(c.line + 1, c.col);
					self.set_status("Line duplicated");
				}
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
					let c = self.cursors.cursor();
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
					self.cursors.set_cursor(new_line, new_col);
					self.set_status("Pasted");
				}
			}

			// -- Global Replace --
			Command::ReplaceInsertChar(ch) => {
				if self.mode == Mode::ReplacingWith {
					self.replace_with.insert(self.prompt_cursor, ch);
					self.prompt_cursor += 1;
				}
			}
			Command::ReplaceDeleteChar => {
				if self.mode == Mode::ReplacingWith
					&& self.prompt_cursor > 0 {
						self.prompt_cursor -= 1;
						self.replace_with.remove(self.prompt_cursor);
					}
			}
			Command::ReplaceWithConfirm => {
				if self.search_matches.is_empty() {
					self.mode = Mode::Editing;
					self.search_query.clear();
					self.clear_status();
				} else {
					self.mode = Mode::ReplacingStep;
					self.jump_to_search_match();
				}
			}
			Command::ReplaceActionYes => {
				if let Some(&(start, end)) = self.search_matches.get(self.search_match_idx) {
					let replacement = self.replace_with.clone();
					self.buffer_mut().commit_edits(); // wrap
					self.buffer_mut().delete_range(start, end);
					self.buffer_mut().insert_str(start, &replacement);
					self.buffer_mut().commit_edits();

					let new_pos = start + replacement.len();
					let line = self.buffer().text.char_to_line(new_pos);
					let col = new_pos - self.buffer().text.line_to_char(line);
					self.search_saved_cursor = Some((line, col));
					self.cursors.set_cursor(line, col);
					self.refresh_search_matches();

					if self.search_matches.is_empty() {
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
				if !self.search_matches.is_empty() {
					self.search_match_idx = (self.search_match_idx + 1) % self.search_matches.len();
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
				let pending_matches = self.search_matches[self.search_match_idx..].to_vec();
				for &(start, end) in pending_matches.iter().rev() {
					self.buffer_mut().delete_range(start, end);
					self.buffer_mut().insert_str(start, &replacement);
				}

				self.buffer_mut().commit_edits();
				self.clamp_cursors();
				self.mode = Mode::Editing;
				self.search_query.clear();
				self.search_matches.clear();
				self.search_match_idx = 0;
				self.clear_status();
			}
			Command::ReplaceCancel => {
				if !self.replace_query.is_empty() {
					self.last_search_query = self.replace_query.clone();
				}
				if let Some((line, col)) = self.search_saved_cursor.take() {
					self.cursors.set_cursor(line, col);
				}
				self.search_query.clear();
				self.replace_query.clear();
				self.replace_with.clear();
				self.search_matches.clear();
				self.search_match_idx = 0;
				self.mode = Mode::Editing;
				self.clear_status();
			}

			// -- Search --
			Command::SearchForward => {
				// Enter search mode — save the current cursor position.
				self.clear_selection();
				let c = self.cursors.cursor();
				self.search_saved_cursor = Some((c.line, c.col));
				// Pre-fill with the last search query so re-opening search
				// immediately shows previous results.
				self.search_query = self.last_search_query.clone();
				self.search_matches.clear();
				self.search_match_idx = 0;
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
					self.last_search_query = self.search_query.clone();
				}
				if let Some(&(start, end)) = self.search_matches.get(self.search_match_idx) {
					// Set anchor at the end of the match, head at the start
					// so the matched text is selected.
					use crate::editor::cursor::Cursor;
					let line = self.buffer().text.char_to_line(start);
					let col = start - self.buffer().text.line_to_char(line);
					let end_line = self.buffer().text.char_to_line(end);
					let end_col = end - self.buffer().text.line_to_char(end_line);
					self.cursors.primary_mut().anchor = Cursor::new(end_line, end_col);
					self.cursors.primary_mut().head = Cursor::new(line, col);
				}
				self.mode = Mode::Editing;
				self.search_query.clear();
				self.search_matches.clear();
				self.search_match_idx = 0;
				self.search_saved_cursor = None;
				self.clear_status();
			}
			Command::SearchCancel => {
				// Restore cursor to its pre-search position.
				if !self.search_query.is_empty() {
					self.last_search_query = self.search_query.clone();
				}
				if let Some((line, col)) = self.search_saved_cursor.take() {
					self.cursors.set_cursor(line, col);
				}
				self.search_query.clear();
				self.search_matches.clear();
				self.search_match_idx = 0;
				self.mode = Mode::Editing;
				self.clear_status();
			}
			Command::SearchConvertToReplace => {
				if !self.search_matches.is_empty() {
					self.replace_query = self.search_query.clone();
					self.last_search_query = self.search_query.clone();
					self.mode = Mode::ReplacingWith;
					self.prompt_cursor = self.replace_with.chars().count();
					self.prompt_view_start.set(0);
				}
			}
			Command::SearchNext => {
				if !self.search_matches.is_empty() {
					self.search_match_idx = (self.search_match_idx + 1) % self.search_matches.len();
					self.jump_to_search_match();
				}
			}
			Command::SearchPrev => {
				if !self.search_matches.is_empty() {
					if self.search_match_idx == 0 {
						self.search_match_idx = self.search_matches.len() - 1;
					} else {
						self.search_match_idx -= 1;
					}
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
					self.goto_line_input.insert(self.prompt_cursor, ch);
					self.prompt_cursor += 1;
				}
			}
			Command::GoToLineDeleteChar => {
				if self.prompt_cursor > 0 {
					self.prompt_cursor -= 1;
					self.goto_line_input.remove(self.prompt_cursor);
				}
			}
			Command::GoToLineConfirm => {
				if let Ok(n) = self.goto_line_input.parse::<usize>() {
					let target = if n == 0 { 0 } else { n - 1 }; // 1-indexed to 0-indexed
					let max_line = self.buffer().line_count().saturating_sub(1);
					let line = target.min(max_line);
					self.cursors.set_cursor(line, 0);
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
				self.save_as_input.insert(self.prompt_cursor, ch);
				self.prompt_cursor += 1;
			}
			Command::SaveAsDeleteChar => {
				if self.prompt_cursor > 0 {
					self.prompt_cursor -= 1;
					self.save_as_input.remove(self.prompt_cursor);
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
						self.mode = Mode::Editing;
						self.buffer_mut().commit_edits();
						let cfg = self.config.clone();
						match self.buffer_mut().save_to(path, &cfg) {
							Ok(()) => self.set_status(format!("✓ Saved as {}", path.display())),
							Err(e) => self.set_status(format!("Save failed: {}", e)),
						}
					}
				}
			}
			Command::SaveAsCancel => {
				self.save_as_input.clear();
				self.prompt_cursor = 0;
				self.mode = Mode::Editing;
				self.clear_status();
			}

			// -- Overwrite confirmation --
			Command::ConfirmOverwrite => {
				if let Some(path_str) = self.save_as_pending_path.take() {
					let path = std::path::Path::new(&path_str);
					self.save_as_input.clear();
					self.prompt_cursor = 0;
					self.mode = Mode::Editing;
					self.buffer_mut().commit_edits();
					let cfg = self.config.clone();
					match self.buffer_mut().save_to(path, &cfg) {
						Ok(()) => self.set_status(format!("✓ Saved as {}", path_str)),
						Err(e) => self.set_status(format!("Save failed: {}", e)),
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
				if self.buffer().dirty {
					self.mode = Mode::ConfirmQuit;
				} else {
					self.should_quit = true;
				}
			}
			Command::ForceQuit => {
				self.should_quit = true;
			}
			Command::SaveAndQuit => {
				if self.buffer().file_path.is_none() {
					self.execute(Command::SaveAsOpen);
				} else {
					self.buffer_mut().commit_edits();
					let cfg = self.config.clone();
					match self.buffer_mut().save(&cfg) {
						Ok(()) => self.should_quit = true,
						Err(e) => {
							self.mode = Mode::Editing;
							self.set_status(format!("Save failed: {}", e));
						}
					}
				}
			}
			Command::CancelQuit => {
				self.mode = Mode::Editing;
				self.clear_status();
			}

			Command::ToggleHelp => {
				self.show_help = !self.show_help;
			}

			Command::ToggleWrap => {
				self.config.wrap_lines = !self.config.wrap_lines;
				self.scroll_vrow = 0;
			}

			Command::FormatDocument => {
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
				crate::editor::formatter::spawn_formatter(ext_str, content, tx);

				self.fmt_rx = Some(rx);
				self.fmt_baseline_version = Some(self.buffer().version);
				self.is_formatting = true;
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
			Command::Noop => {}
		}
	}
}

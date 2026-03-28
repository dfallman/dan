pub mod commands;
pub mod cursor;
mod editing;
pub mod mode;
mod navigation;
mod search;
mod selection;
pub(crate) mod viewport;
pub(crate) mod visual_col;

pub(crate) use viewport::visual_rows_for;

use crate::buffer::Buffer;
use crate::config::Config;
use crate::editor::commands::Command;
use crate::editor::cursor::CursorSet;
use crate::editor::mode::Mode;
use crate::syntax::Highlighter;

use crossterm::terminal;


/// Core editor state — pico-style modeless editor.
pub struct Editor {
	/// Loaded configuration.
	pub config: Config,
	/// All open buffers.
	pub buffers: Vec<Buffer>,
	/// Index of the active buffer.
	pub active_buffer: usize,
	/// Current mode (Editing or Searching).
	pub mode: Mode,
	/// Cursors / selections for the active buffer.
	pub cursors: CursorSet,
	/// Status message displayed in the status bar.
	pub status_msg: Option<String>,
	/// Whether the editor should quit.
	pub should_quit: bool,
	/// Viewport scroll offset (top visible line).
	pub scroll_y: usize,
	/// Viewport visual row scroll offset (for wrap mode sub-line scrolling).
	pub scroll_vrow: usize,
	/// Horizontal scroll offset (first visible column, used when wrap_lines=false).
	pub scroll_x: usize,
	/// OS system clipboard.
	pub sys_clipboard: Option<arboard::Clipboard>,
	/// Internal fallback clipboard content.
	pub internal_clipboard: String,
	/// Current terminal width (updated on resize).
	pub terminal_width: u16,
	/// Current terminal height (updated on resize).
	pub terminal_height: u16,
	/// Suppresses the next internal Paste command after a bracketed
	/// paste event, preventing double-insert on terminals that send
	/// both Event::Paste and Event::Key(Ctrl+V).
	suppress_next_paste: bool,
	/// Whether the help legend bar is visible (toggled with ^H).
	pub show_help: bool,
	/// Current search query string (populated during search mode).
	pub search_query: String,
	/// Target query string globally tracked for replace execution limits.
	pub replace_query: String,
	/// Target format payload tracking during step bindings natively.
	pub replace_with: String,
	/// All current matches as (start_char, end_char) pairs.
	pub search_matches: Vec<(usize, usize)>,
	/// Index of the currently-highlighted match.
	pub search_match_idx: usize,
	/// Saved cursor position before entering search (so Esc can restore).
	pub search_saved_cursor: Option<(usize, usize)>,
	/// Last completed search query (persists across search sessions).
	last_search_query: String,
	/// Syntax highlighter (shared across buffers).
	pub highlighter: Highlighter,
	/// Current input text in the go-to-line prompt.
	pub goto_line_input: String,
	/// Current input text in the save-as prompt.
	pub save_as_input: String,
	/// Cursor position within the save-as input.
	pub save_as_cursor: usize,
	/// Path pending overwrite confirmation.
	pub save_as_pending_path: Option<String>,
}

impl Editor {
	pub fn new() -> Self {
		let (tw, th) = terminal::size().unwrap_or((80, 24));
		let config = Config::load();
		let highlighter = Highlighter::new(&config.theme);
		Self {
			config,
			buffers: vec![Buffer::new()],
			active_buffer: 0,
			mode: Mode::Editing,
			cursors: CursorSet::new(),
			status_msg: None,
			should_quit: false,
			scroll_y: 0,
			scroll_vrow: 0,
			scroll_x: 0,
			sys_clipboard: arboard::Clipboard::new().ok(),
			internal_clipboard: String::new(),
			terminal_width: tw,
			terminal_height: th,
			suppress_next_paste: false,
			show_help: false,
			search_query: String::new(),
			replace_query: String::new(),
			replace_with: String::new(),
			search_matches: Vec::new(),
			search_match_idx: 0,
			search_saved_cursor: None,
			last_search_query: String::new(),
			highlighter,
			goto_line_input: String::new(),
			save_as_input: String::new(),
			save_as_cursor: 0,
			save_as_pending_path: None,
		}
	}

	/// Open a file into a new buffer and switch to it.
	pub fn open_file(&mut self, path: &std::path::Path) -> std::io::Result<()> {
		let buffer = Buffer::from_file(path)?;
		self.buffers.push(buffer);
		self.active_buffer = self.buffers.len() - 1;
		self.cursors = CursorSet::new();
		self.scroll_y = 0;
		self.scroll_vrow = 0;
		Ok(())
	}

	/// Get a reference to the active buffer.
	pub fn buffer(&self) -> &Buffer {
		&self.buffers[self.active_buffer]
	}

	/// Get a mutable reference to the active buffer.
	pub fn buffer_mut(&mut self) -> &mut Buffer {
		&mut self.buffers[self.active_buffer]
	}

	/// Returns the effective tab width for the active buffer.
	pub fn tab_width(&self) -> usize {
		self.buffer().tab_width.unwrap_or(self.config.tab_width)
	}

	/// Returns the effective expand_tab setting for the active buffer.
	pub fn expand_tab(&self) -> bool {
		self.buffer().expand_tab.unwrap_or(self.config.expand_tab)
	}

	/// Set a status message.
	pub fn set_status(&mut self, msg: impl Into<String>) {
		self.status_msg = Some(msg.into());
	}

	/// Clear the status message.
	pub fn clear_status(&mut self) {
		self.status_msg = None;
	}

	/// Execute a command.
	pub fn execute(&mut self, cmd: Command) {
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
				let line_text: String = self.buffer().text.line_slice(c.line).chars().collect();
				self.cursors.primary_mut().head.set_col(len);
				self.cursors.primary_mut().head.desired_vcol = crate::editor::visual_col::visual_col_at(&line_text, len, self.tab_width());
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
				let _len = self.line_len_no_newline(c.line);
				let line_text = self.buffer().text.line(c.line);
				let len = line_text.len().saturating_sub(1);
				self.cursors.primary_mut().head.desired_vcol = crate::editor::visual_col::visual_col_at(&line_text, len, self.tab_width());
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
				self.delete_selection_if_active();
				let pos = self.cursor_char_pos();
				self.buffer_mut().insert_char(pos, ch);
				let c = self.cursors.cursor();
				// Move cursor forward and collapse selection (anchor = head)
				self.cursors.set_cursor(c.line, c.col + 1);
			}
			Command::InsertString(ref s) => {
				self.delete_selection_if_active();
				if !s.is_empty() {
					let clean = Self::sanitize_paste(s);
					let pos = self.cursor_char_pos();
					let char_count = clean.chars().count();
					self.buffer_mut().insert_str(pos, &clean);
					// Move cursor to end of inserted text
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
						if i >= c.col { break; }
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
			Command::Dedent => {
				let c = self.cursors.cursor();
				let line_start = self.buffer().text.line_to_char(c.line);
				let line_slice = self.buffer().text.line_slice(c.line);
				let tw = self.tab_width();

				// Count leading whitespace to remove:
				// - a single '\t', or
				// - up to `tab_width` leading spaces
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
					self.buffer_mut().delete_range(line_start, line_start + remove);
					let new_col = c.col.saturating_sub(remove);
					self.cursors.set_cursor(c.line, new_col);
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
					let clean = Self::sanitize_paste(&text);
					let char_count = clean.chars().count();
					self.buffer_mut().insert_str(pos, &clean);
					// Move cursor to end of pasted text
					let new_pos = pos + char_count;
					let new_line = self.buffer().text.char_to_line(new_pos);
					let new_col = new_pos - self.buffer().text.line_to_char(new_line);
					self.cursors.set_cursor(new_line, new_col);
					self.set_status("Pasted");
				}
			}

			// -- Global Replace --
			Command::ReplaceOpen => {
				self.clear_selection();
				self.replace_query = self.last_search_query.clone();
				self.search_query.clear();
				self.replace_with.clear();
				self.search_matches.clear();
				self.search_match_idx = 0;
				let c = self.cursors.cursor();
				self.search_saved_cursor = Some((c.line, c.col));
				self.mode = Mode::ReplacingSearch;
			}
			Command::ReplaceInsertChar(ch) => {
				if self.mode == Mode::ReplacingSearch {
					self.replace_query.push(ch);
					self.search_query = self.replace_query.clone();
					self.refresh_search_matches();
				} else if self.mode == Mode::ReplacingWith {
					self.replace_with.push(ch);
				}
			}
			Command::ReplaceDeleteChar => {
				if self.mode == Mode::ReplacingSearch {
					self.replace_query.pop();
					self.search_query = self.replace_query.clone();
					self.refresh_search_matches();
				} else if self.mode == Mode::ReplacingWith {
					self.replace_with.pop();
				}
			}
			Command::ReplaceSearchConfirm => {
				if !self.replace_query.is_empty() {
					self.last_search_query = self.replace_query.clone();
					self.mode = Mode::ReplacingWith;
				} else {
					self.mode = Mode::Editing;
					self.search_query.clear();
					self.clear_status();
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
				
				// Execute backwards to trivially retain string indexing locations dynamically
				// Slicing from current index ensures we never retro-actively mangle skipped `(n)o` instances
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
				// If we have a previous query, run the search immediately.
				if !self.search_query.is_empty() {
					self.refresh_search_matches();
				}
			}
			Command::SearchInsertChar(ch) => {
				self.search_query.push(ch);
				self.refresh_search_matches();
			}
			Command::SearchDeleteChar => {
				self.search_query.pop();
				self.refresh_search_matches();
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
			Command::SearchNext => {
				if !self.search_matches.is_empty() {
					self.search_match_idx =
						(self.search_match_idx + 1) % self.search_matches.len();
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
				self.mode = Mode::GoToLine;
			}
			Command::GoToLineInsertChar(ch) => {
				if ch.is_ascii_digit() {
					self.goto_line_input.push(ch);
				}
			}
			Command::GoToLineDeleteChar => {
				self.goto_line_input.pop();
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
				self.mode = Mode::Editing;
			}
			Command::GoToLineCancel => {
				self.goto_line_input.clear();
				self.mode = Mode::Editing;
				self.clear_status();
			}

			// -- Save As --
			Command::SaveAsOpen => {
				// Pre-populate with current file path if one exists.
				self.save_as_input = self.buffer()
					.file_path
					.as_ref()
					.map(|p| p.to_string_lossy().to_string())
					.unwrap_or_default();
				self.save_as_cursor = self.save_as_input.len();
				self.mode = Mode::SaveAs;
				self.set_status("Save as: type path, ⏎ Save, Esc Cancel");
			}
			Command::SaveAsInsertChar(ch) => {
				self.save_as_input.insert(self.save_as_cursor, ch);
				self.save_as_cursor += 1;
			}
			Command::SaveAsDeleteChar => {
				if self.save_as_cursor > 0 {
					self.save_as_cursor -= 1;
					self.save_as_input.remove(self.save_as_cursor);
				}
			}
			Command::SaveAsCursorLeft => {
				if self.save_as_cursor > 0 {
					self.save_as_cursor -= 1;
				}
			}
			Command::SaveAsCursorRight => {
				if self.save_as_cursor < self.save_as_input.len() {
					self.save_as_cursor += 1;
				}
			}
			Command::SaveAsConfirm => {
				let path_str = self.save_as_input.clone();
				if path_str.is_empty() {
					self.save_as_input.clear();
					self.save_as_cursor = 0;
					self.mode = Mode::Editing;
					self.set_status("Save as cancelled: no path given");
				} else {
					let path = std::path::Path::new(&path_str);
					// Check if parent directory exists.
					if let Some(parent) = path.parent() {
						if !parent.as_os_str().is_empty() && !parent.exists() {
							self.set_status(format!("Directory does not exist: {}", parent.display()));
							return;
						}
					}
					// Check if file already exists — ask for overwrite confirmation.
					if path.exists() {
						self.save_as_pending_path = Some(path_str);
						self.mode = Mode::ConfirmOverwrite;
						self.set_status("File exists! ^O Overwrite, Esc Cancel");
					} else {
						// New file — save directly.
						self.save_as_input.clear();
						self.save_as_cursor = 0;
						self.mode = Mode::Editing;
						self.buffer_mut().commit_edits();
						match self.buffer_mut().save_to(path) {
							Ok(()) => self.set_status(format!("Saved as {}", path.display())),
							Err(e) => self.set_status(format!("Save failed: {}", e)),
						}
					}
				}
			}
			Command::SaveAsCancel => {
				self.save_as_input.clear();
				self.save_as_cursor = 0;
				self.mode = Mode::Editing;
				self.clear_status();
			}

			// -- Overwrite confirmation --
			Command::ConfirmOverwrite => {
				if let Some(path_str) = self.save_as_pending_path.take() {
					let path = std::path::Path::new(&path_str);
					self.save_as_input.clear();
					self.save_as_cursor = 0;
					self.mode = Mode::Editing;
					self.buffer_mut().commit_edits();
					match self.buffer_mut().save_to(path) {
						Ok(()) => self.set_status(format!("Saved as {}", path_str)),
						Err(e) => self.set_status(format!("Save failed: {}", e)),
					}
				}
			}
			Command::CancelOverwrite => {
				self.save_as_pending_path = None;
				self.mode = Mode::SaveAs;
				self.set_status("Save as: Save as: type path, ⏎ Save, Esc Cancel");
			}

			// -- File --
			Command::Save => {
				self.buffer_mut().commit_edits();
				match self.buffer_mut().save() {
					Ok(()) => self.set_status("Saved"),
					Err(e) => self.set_status(format!("Save failed: {}", e)),
				}
			}
			Command::Quit => {
				if self.buffer().dirty {
					self.mode = Mode::ConfirmQuit;
					self.set_status(
						"Unsaved changes! ^S Save and quit, ^Y Force quit, Esc Cancel"
					);
				} else {
					self.should_quit = true;
				}
			}
			Command::ForceQuit => {
				self.should_quit = true;
			}
			Command::SaveAndQuit => {
				self.buffer_mut().commit_edits();
				match self.buffer_mut().save() {
					Ok(()) => self.should_quit = true,
					Err(e) => {
						self.mode = Mode::Editing;
						self.set_status(format!("Save failed: {}", e));
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

			Command::ToggleSyntax => {
				self.config.syntax_highlight = !self.config.syntax_highlight;
			}

			Command::Noop => {}
		}
	}

	/// Update stored terminal dimensions (called on resize events).
	pub fn handle_resize(&mut self, width: u16, height: u16) {
		self.terminal_width = width;
		self.terminal_height = height;
	}
}

impl Default for Editor {
	fn default() -> Self {
		Self::new()
	}
}

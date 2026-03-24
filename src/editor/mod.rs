pub mod commands;
pub mod cursor;
pub mod mode;

use crate::buffer::Buffer;
use crate::config::Config;
use crate::editor::commands::Command;
use crate::editor::cursor::CursorSet;
use crate::editor::mode::Mode;

use crossterm::terminal;
use unicode_segmentation::UnicodeSegmentation;

/// Core editor state — pico-style modeless editor.
pub struct Editor {
    /// Loaded configuration.
    pub config: Config,
    /// All open buffers.
    pub buffers: Vec<Buffer>,
    /// Index of the active buffer.
    pub active_buffer: usize,
    /// Current mode (Editing or Selecting).
    pub mode: Mode,
    /// Cursors / selections for the active buffer.
    pub cursors: CursorSet,
    /// Status message displayed in the status bar.
    pub status_msg: Option<String>,
    /// Whether the editor should quit.
    pub should_quit: bool,
    /// Viewport scroll offset (top visible line).
    pub scroll_y: usize,
    /// Horizontal scroll offset (first visible column, used when wrap_lines=false).
    pub scroll_x: usize,
    /// Clipboard content.
    pub clipboard: String,
    /// Selection anchor (line, col) — set when shift-selecting begins.
    pub select_anchor: Option<(usize, usize)>,
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
    /// All current matches as (start_char, end_char) pairs.
    pub search_matches: Vec<(usize, usize)>,
    /// Index of the currently-highlighted match.
    pub search_match_idx: usize,
    /// Saved cursor position before entering search (so Esc can restore).
    pub search_saved_cursor: Option<(usize, usize)>,
    /// Last completed search query (persists across search sessions).
    last_search_query: String,
}

impl Editor {
    pub fn new() -> Self {
        let (tw, th) = terminal::size().unwrap_or((80, 24));
        Self {
            config: Config::load(),
            buffers: vec![Buffer::new()],
            active_buffer: 0,
            mode: Mode::Editing,
            cursors: CursorSet::new(),
            status_msg: None,
            should_quit: false,
            scroll_y: 0,
            scroll_x: 0,
            clipboard: String::new(),
            select_anchor: None,
            terminal_width: tw,
            terminal_height: th,
            suppress_next_paste: false,
            show_help: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_match_idx: 0,
            search_saved_cursor: None,
            last_search_query: String::new(),
        }
    }

    /// Open a file into a new buffer and switch to it.
    pub fn open_file(&mut self, path: &std::path::Path) -> std::io::Result<()> {
        let buffer = Buffer::from_file(path)?;
        self.buffers.push(buffer);
        self.active_buffer = self.buffers.len() - 1;
        self.cursors = CursorSet::new();
        self.scroll_y = 0;
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
                self.clear_selection();
                self.move_cursor_horizontal(-1);
            }
            Command::MoveRight => {
                self.clear_selection();
                self.move_cursor_horizontal(1);
            }
            Command::MoveUp => {
                self.clear_selection();
                self.move_cursor_vertical(-1);
            }
            Command::MoveDown => {
                self.clear_selection();
                self.move_cursor_vertical(1);
            }
            Command::MoveLineStart => {
                self.clear_selection();
                self.cursors.primary_mut().head.set_col(0);
            }
            Command::MoveLineEnd => {
                self.clear_selection();
                let c = self.cursors.cursor();
                let len = self.line_len_no_newline(c.line);
                self.cursors.primary_mut().head.set_col(len);
            }
            Command::MoveWordForward => {
                self.clear_selection();
                self.move_word_forward();
            }
            Command::MoveWordBackward => {
                self.clear_selection();
                self.move_word_backward();
            }
            Command::SwapLineUp => {
                self.clear_selection();
                self.swap_line_up();
            }
            Command::SwapLineDown => {
                self.clear_selection();
                self.swap_line_down();
            }
            Command::MoveBufferTop => {
                self.clear_selection();
                self.cursors.set_cursor(0, 0);
            }
            Command::MoveBufferBottom => {
                self.clear_selection();
                let last_line = self.buffer().line_count().saturating_sub(1);
                self.cursors.set_cursor(last_line, 0);
            }
            Command::PageUp => {
                self.clear_selection();
                for _ in 0..20 {
                    self.move_cursor_vertical(-1);
                }
            }
            Command::PageDown => {
                self.clear_selection();
                for _ in 0..20 {
                    self.move_cursor_vertical(1);
                }
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
                self.cursors.primary_mut().head.set_col(len);
            }
            Command::SelectAll => {
                let last_line = self.buffer().line_count().saturating_sub(1);
                let last_col = self.line_len_no_newline(last_line);
                self.select_anchor = Some((0, 0));
                self.cursors.set_cursor(last_line, last_col);
                self.mode = Mode::Selecting;
            }

            // -- Editing --
            Command::InsertChar(ch) => {
                self.delete_selection_if_active();
                let pos = self.cursor_char_pos();
                self.buffer_mut().insert_char(pos, ch);
                let c = self.cursors.cursor();
                self.cursors.primary_mut().head.set_col(c.col + 1);
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
                let pos = self.cursor_char_pos();
                self.buffer_mut().insert_char(pos, '\n');
                let c = self.cursors.cursor();
                self.cursors.set_cursor(c.line + 1, 0);
            }
            Command::InsertTab => {
                self.delete_selection_if_active();
                let pos = self.cursor_char_pos();
                let tw = self.config.tab_width;
                let advance = if self.config.expand_tab {
                    let spaces: String = " ".repeat(tw);
                    self.buffer_mut().insert_str(pos, &spaces);
                    tw
                } else {
                    self.buffer_mut().insert_str(pos, "\t");
                    1
                };
                let c = self.cursors.cursor();
                self.cursors.primary_mut().head.set_col(c.col + advance);
            }
            Command::Dedent => {
                let c = self.cursors.cursor();
                let line_start = self.buffer().text.line_to_char(c.line);
                let line_slice = self.buffer().text.line_slice(c.line);
                let tw = self.config.tab_width;

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
                    self.cursors.primary_mut().head.set_col(new_col);
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
                            self.cursors.primary_mut().head.set_col(c.col - 1);
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
                    self.clipboard = deleted;
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
            }
            Command::Redo => {
                self.clear_selection();
                self.buffer_mut().redo();
            }

            // -- Clipboard (GUI-style) --
            Command::Copy => {
                if let Some(text) = self.get_selected_text() {
                    self.clipboard = text;
                    self.set_status("Copied");
                } else {
                    // Copy current line if no selection
                    let c = self.cursors.cursor();
                    self.clipboard = self.buffer().text.line(c.line).to_string();
                    self.set_status("Copied line");
                }
            }
            Command::Cut => {
                if self.has_selection() {
                    if let Some(text) = self.get_selected_text() {
                        self.clipboard = text;
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
                if !self.clipboard.is_empty() {
                    let pos = self.cursor_char_pos();
                    let text = self.clipboard.clone();
                    let char_count = text.chars().count();
                    self.buffer_mut().insert_str(pos, &text);
                    // Move cursor to end of pasted text
                    let new_pos = pos + char_count;
                    let new_line = self.buffer().text.char_to_line(new_pos);
                    let new_col = new_pos - self.buffer().text.line_to_char(new_line);
                    self.cursors.set_cursor(new_line, new_col);
                    self.set_status("Pasted");
                }
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
                    // Position cursor at the start of the match.
                    let line = self.buffer().text.char_to_line(start);
                    let col = start - self.buffer().text.line_to_char(line);
                    self.cursors.set_cursor(line, col);
                    // Set selection anchor at end of match so the text is selected.
                    let end_line = self.buffer().text.char_to_line(end);
                    let end_col = end - self.buffer().text.line_to_char(end_line);
                    self.select_anchor = Some((end_line, end_col));
                    self.mode = Mode::Selecting;
                } else {
                    self.mode = Mode::Editing;
                }
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
                    self.set_status("Unsaved changes! Save first (Ctrl+S) or use Ctrl+Q again");
                    // Second Ctrl+Q force quits — we flip a soft flag
                } else {
                    self.should_quit = true;
                }
            }
            Command::ForceQuit => {
                self.should_quit = true;
            }

            Command::ToggleHelp => {
                self.show_help = !self.show_help;
            }

            Command::ToggleWrap => {
                self.config.wrap_lines = !self.config.wrap_lines;
            }

            Command::Noop => {}
        }
    }

    // -- Selection helpers --

    /// Returns true if there is an active selection.
    pub fn has_selection(&self) -> bool {
        self.select_anchor.is_some()
    }

    /// Begin tracking a selection from the current cursor position.
    fn begin_selection_if_needed(&mut self) {
        if self.select_anchor.is_none() {
            let c = self.cursors.cursor();
            self.select_anchor = Some((c.line, c.col));
            self.mode = Mode::Selecting;
        }
    }

    /// Clear the active selection (does NOT change the editor mode).
    fn clear_selection(&mut self) {
        self.select_anchor = None;
        if self.mode == Mode::Selecting {
            self.mode = Mode::Editing;
        }
    }

    /// Get the selected text range as (start_pos, end_pos) char offsets.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let (anchor_line, anchor_col) = self.select_anchor?;
        let anchor_pos = self.buffer().text.line_to_char(anchor_line) + anchor_col;
        let head_pos = self.cursor_char_pos();
        if anchor_pos <= head_pos {
            Some((anchor_pos, head_pos))
        } else {
            Some((head_pos, anchor_pos))
        }
    }

    /// Get the selected text string.
    fn get_selected_text(&self) -> Option<String> {
        let (start, end) = self.selection_range()?;
        if start == end {
            return None;
        }
        Some(self.buffer().text.slice_to_string(start..end))
    }

    /// Delete the selected text and clear the selection.
    fn delete_selection_if_active(&mut self) {
        if let Some((start, end)) = self.selection_range() {
            if start < end {
                self.buffer_mut().delete_range(start, end);
                let new_line = self.buffer().text.char_to_line(start);
                let new_col = start - self.buffer().text.line_to_char(new_line);
                self.cursors.set_cursor(new_line, new_col);
            }
        }
        self.clear_selection();
    }

    // -- Internal helpers --

    /// Re-run the search against the buffer and jump to the nearest match.
    fn refresh_search_matches(&mut self) {
        self.search_matches = self.buffer().text.find_all(&self.search_query);
        if self.search_matches.is_empty() {
            if self.search_query.is_empty() {
                self.clear_status();
            } else {
                self.set_status(format!("No matches for \"{}\"", self.search_query));
            }
            return;
        }
        // Find the match nearest (at or after) the saved cursor position.
        let anchor_pos = if let Some((line, col)) = self.search_saved_cursor {
            self.buffer().text.line_to_char(line) + col
        } else {
            0
        };
        self.search_match_idx = self
            .search_matches
            .iter()
            .position(|&(start, _)| start >= anchor_pos)
            .unwrap_or(0);
        self.jump_to_search_match();
    }

    /// Jump the cursor to the currently-highlighted search match.
    fn jump_to_search_match(&mut self) {
        if let Some(&(start, _end)) = self.search_matches.get(self.search_match_idx) {
            let line = self.buffer().text.char_to_line(start);
            let col = start - self.buffer().text.line_to_char(line);
            self.cursors.set_cursor(line, col);
            self.set_status(format!(
                "{}/{} matches",
                self.search_match_idx + 1,
                self.search_matches.len()
            ));
        }
    }

    /// Get the char position in the buffer for the current cursor.
    fn cursor_char_pos(&self) -> usize {
        let c = self.cursors.cursor();
        let line_start = self.buffer().text.line_to_char(c.line);
        let line_len = self.buffer().text.line_len_chars(c.line);
        line_start + c.col.min(line_len)
    }

    /// Get the length of a line excluding the trailing newline.
    fn line_len_no_newline(&self, line: usize) -> usize {
        let len = self.buffer().text.line_len_chars(line);
        if len > 0 {
            let line_start = self.buffer().text.line_to_char(line);
            let last_char = self.buffer().text.char_at(line_start + len - 1);
            if last_char == '\n' {
                len - 1
            } else {
                len
            }
        } else {
            0
        }
    }

    /// Move cursor horizontally by `delta` chars (-1 = left, 1 = right).
    fn move_cursor_horizontal(&mut self, delta: i32) {
        let c = self.cursors.cursor();
        if delta < 0 {
            if c.col > 0 {
                self.cursors.primary_mut().head.set_col(c.col - 1);
            } else if c.line > 0 {
                let prev_len = self.line_len_no_newline(c.line - 1);
                self.cursors.primary_mut().head.line = c.line - 1;
                self.cursors.primary_mut().head.set_col(prev_len);
            }
        } else {
            let line_len = self.line_len_no_newline(c.line);
            if c.col < line_len {
                self.cursors.primary_mut().head.set_col(c.col + 1);
            } else if c.line + 1 < self.buffer().line_count() {
                self.cursors.primary_mut().head.line = c.line + 1;
                self.cursors.primary_mut().head.set_col(0);
            }
        }
    }

    /// Move cursor vertically by `delta` lines.
    fn move_cursor_vertical(&mut self, delta: i32) {
        if !self.config.wrap_lines {
            // No-wrap mode: move by buffer lines as before.
            let c = self.cursors.cursor();
            let new_line = if delta < 0 {
                c.line.saturating_sub((-delta) as usize)
            } else {
                let max_line = self.buffer().line_count().saturating_sub(1);
                (c.line + delta as usize).min(max_line)
            };

            if new_line != c.line {
                let line_len = self.line_len_no_newline(new_line);
                let new_col = c.desired_col.min(line_len);
                self.cursors.primary_mut().head.line = new_line;
                self.cursors.primary_mut().head.set_col_clamped(new_col);
            }
            return;
        }

        // -- Wrap mode: move by visual (screen) rows --
        let text_area_width = self.text_area_width();
        if text_area_width == 0 {
            return;
        }
        let tab_w = self.config.tab_width;
        let c = self.cursors.cursor();
        let line_count = self.buffer().line_count();

        // Collect current line text so we release the borrow on self.
        let cur_line_text: String = self.buffer().text.line_slice(c.line).chars().collect();
        let cur_line_len = self.line_len_no_newline(c.line);

        let rows = visual_rows_for(&cur_line_text, tab_w, text_area_width);

        // Which visual row is the cursor on within its buffer line?
        let mut cur_vrow: usize = 0;
        for (i, &(start, end)) in rows.iter().enumerate() {
            if c.col >= start && (c.col < end || i == rows.len() - 1) {
                cur_vrow = i;
                break;
            }
        }

        if delta > 0 {
            // Moving down
            if cur_vrow + 1 < rows.len() {
                // Stay on same buffer line, move to next visual row.
                let next_row = rows[cur_vrow + 1];
                let new_col = col_in_visual_row_from_text(&cur_line_text, cur_line_len, next_row.0, next_row.1, c.desired_col, tab_w);
                self.cursors.primary_mut().head.set_col_clamped(new_col);
            } else {
                // Move to next buffer line (first visual row).
                let next_line = c.line + 1;
                if next_line < line_count {
                    let next_text: String = self.buffer().text.line_slice(next_line).chars().collect();
                    let next_len = self.line_len_no_newline(next_line);
                    let next_rows = visual_rows_for(&next_text, tab_w, text_area_width);
                    let first = next_rows[0];
                    let new_col = col_in_visual_row_from_text(&next_text, next_len, first.0, first.1, c.desired_col, tab_w);
                    self.cursors.primary_mut().head.line = next_line;
                    self.cursors.primary_mut().head.set_col_clamped(new_col);
                }
            }
        } else {
            // Moving up
            if cur_vrow > 0 {
                // Stay on same buffer line, move to previous visual row.
                let prev_row = rows[cur_vrow - 1];
                let new_col = col_in_visual_row_from_text(&cur_line_text, cur_line_len, prev_row.0, prev_row.1, c.desired_col, tab_w);
                self.cursors.primary_mut().head.set_col_clamped(new_col);
            } else {
                // Move to previous buffer line (last visual row).
                if c.line > 0 {
                    let prev_line = c.line - 1;
                    let prev_text: String = self.buffer().text.line_slice(prev_line).chars().collect();
                    let prev_len = self.line_len_no_newline(prev_line);
                    let prev_rows = visual_rows_for(&prev_text, tab_w, text_area_width);
                    let last = prev_rows[prev_rows.len() - 1];
                    let new_col = col_in_visual_row_from_text(&prev_text, prev_len, last.0, last.1, c.desired_col, tab_w);
                    self.cursors.primary_mut().head.line = prev_line;
                    self.cursors.primary_mut().head.set_col_clamped(new_col);
                }
            }
        }
    }

    /// Compute the text-area width (terminal width minus gutter).
    fn text_area_width(&self) -> usize {
        let line_count = self.buffer().line_count();
        let gutter_width = if self.config.line_numbers {
            if line_count == 0 { 1 } else { (line_count as f64).log10().floor() as usize + 1 }
        } else {
            0
        };
        (self.terminal_width as usize).saturating_sub(gutter_width + 1)
    }

    /// Move cursor forward one word using UAX #29 word boundaries.
    fn move_word_forward(&mut self) {
        let c = self.cursors.cursor();
        let line_count = self.buffer().line_count();
        let mut line = c.line;
        let mut col = c.col;

        // Walk forward across lines until we find the next word boundary
        while line < line_count {
            let line_text: String = self.buffer().text.line_slice(line).chars().collect();
            // Collect word-boundary byte offsets, convert to char offsets
            let boundaries: Vec<usize> = line_text
                .split_word_bound_indices()
                .map(|(byte_off, _)| line_text[..byte_off].chars().count())
                .collect();
            // Also add end-of-content boundary (excluding trailing newline)
            let content_len = line_text.trim_end_matches('\n').chars().count();

            // Find the first boundary strictly after our column
            for &b in &boundaries {
                if b > col && b <= content_len {
                    self.cursors.set_cursor(line, b);
                    return;
                }
            }
            // If content_len is past us, go there
            if content_len > col {
                self.cursors.set_cursor(line, content_len);
                return;
            }
            // Move to next line, col 0
            line += 1;
            col = 0;
        }
    }

    /// Move cursor backward one word using UAX #29 word boundaries.
    fn move_word_backward(&mut self) {
        let c = self.cursors.cursor();
        let mut line = c.line;
        let mut col = c.col;

        loop {
            let line_text: String = self.buffer().text.line_slice(line).chars().collect();
            let boundaries: Vec<usize> = line_text
                .split_word_bound_indices()
                .map(|(byte_off, _)| line_text[..byte_off].chars().count())
                .collect();

            // Find the last boundary strictly before our column
            for &b in boundaries.iter().rev() {
                if b < col {
                    self.cursors.set_cursor(line, b);
                    return;
                }
            }
            // No boundary found before us on this line — go to end of previous line
            if line == 0 {
                self.cursors.set_cursor(0, 0);
                return;
            }
            line -= 1;
            let prev_line_text: String = self.buffer().text.line_slice(line).chars().collect();
            col = prev_line_text.trim_end_matches('\n').chars().count();
        }
    }

    /// Swap the current line with the line above it. Cursor follows.
    fn swap_line_up(&mut self) {
        let line = self.cursors.cursor().line;
        if line == 0 {
            return;
        }
        let col = self.cursors.cursor().col;
        let total = self.buffer().line_count();

        let cur_start = self.buffer().text.line_to_char(line);
        let cur_end = if line + 1 < total {
            self.buffer().text.line_to_char(line + 1)
        } else {
            self.buffer().text.len_chars()
        };
        let above_start = self.buffer().text.line_to_char(line - 1);

        let cur_text = self.buffer().text.slice_to_string(cur_start..cur_end);
        let above_text = self.buffer().text.slice_to_string(above_start..cur_start);

        // Strip exactly one trailing newline from each (preserves the line
        // content including any empty-line identity).
        let cur_body = cur_text.strip_suffix('\n').unwrap_or(&cur_text);
        let above_body = above_text.strip_suffix('\n').unwrap_or(&above_text);

        // Rebuild: cur_body first (it moves up), then above_body.
        // Every line except possibly the very last gets a trailing \n.
        let is_last = line + 1 >= total;
        let mut new_text = String::with_capacity(cur_text.len() + above_text.len());
        new_text.push_str(cur_body);
        new_text.push('\n');
        new_text.push_str(above_body);
        if !is_last {
            new_text.push('\n');
        }

        self.buffer_mut().text.remove(above_start..cur_end);
        self.buffer_mut().text.insert_str(above_start, &new_text);
        self.buffer_mut().dirty = true;

        self.cursors.set_cursor(line - 1, col);
    }

    /// Swap the current line with the line below it. Cursor follows.
    fn swap_line_down(&mut self) {
        let line = self.cursors.cursor().line;
        let total = self.buffer().line_count();
        if line + 1 >= total {
            return;
        }
        let col = self.cursors.cursor().col;

        let cur_start = self.buffer().text.line_to_char(line);
        let cur_end = self.buffer().text.line_to_char(line + 1);
        let below_end = if line + 2 < total {
            self.buffer().text.line_to_char(line + 2)
        } else {
            self.buffer().text.len_chars()
        };

        let cur_text = self.buffer().text.slice_to_string(cur_start..cur_end);
        let below_text = self.buffer().text.slice_to_string(cur_end..below_end);

        // Strip exactly one trailing newline from each.
        let cur_body = cur_text.strip_suffix('\n').unwrap_or(&cur_text);
        let below_body = below_text.strip_suffix('\n').unwrap_or(&below_text);

        // Rebuild: below_body first (it moves up), then cur_body.
        let is_last = line + 2 >= total;
        let mut new_text = String::with_capacity(cur_text.len() + below_text.len());
        new_text.push_str(below_body);
        new_text.push('\n');
        new_text.push_str(cur_body);
        if !is_last {
            new_text.push('\n');
        }

        self.buffer_mut().text.remove(cur_start..below_end);
        self.buffer_mut().text.insert_str(cur_start, &new_text);
        self.buffer_mut().dirty = true;

        self.cursors.set_cursor(line + 1, col);
    }

    /// Compute the gutter width (line numbers) for the current buffer.
    fn gutter_width(&self) -> usize {
        if !self.config.line_numbers {
            return 0;
        }
        let lc = self.buffer().line_count();
        if lc == 0 { 1 } else { (lc as f64).log10().floor() as usize + 1 }
    }

    /// Available text columns (terminal width minus gutter and separator).
    fn text_columns(&self) -> usize {
        (self.terminal_width as usize).saturating_sub(self.gutter_width() + 1)
    }

    /// Sanitize pasted / externally-sourced text.
    ///
    /// Strips:
    ///  - HTML / XML tags  (e.g. `<span style="...">`, `</p>`, `<br/>`)
    ///  - Carriage returns  (`\r`) — we normalise to Unix `\n`.
    ///  - Zero-width chars  (U+200B .. U+200F, U+FEFF BOM, U+2060 WJ).
    fn sanitize_paste(text: &str) -> String {
        // 1. Normalise line endings.
        let text = text.replace("\r\n", "\n").replace('\r', "\n");

        // 2. Strip HTML/XML tags with a simple state machine.
        //    This handles `<tag attr="...">`, `</tag>`, `<br/>`,
        //    and also strips `&nbsp;` → space.
        let mut out = String::with_capacity(text.len());
        let mut in_tag = false;
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '<' {
                in_tag = true;
                continue;
            }
            if ch == '>' && in_tag {
                in_tag = false;
                continue;
            }
            if in_tag {
                continue;
            }

            // Decode common HTML entities.
            if ch == '&' {
                let entity: String = chars
                    .by_ref()
                    .take_while(|c| *c != ';')
                    .collect();
                match entity.as_str() {
                    "nbsp" => out.push(' '),
                    "amp"  => out.push('&'),
                    "lt"   => out.push('<'),
                    "gt"   => out.push('>'),
                    "quot" => out.push('"'),
                    "apos" => out.push('\''),
                    "tab"  => out.push('\t'),
                    _ => {
                        // Unknown entity — preserve as-is.
                        out.push('&');
                        out.push_str(&entity);
                        out.push(';');
                    }
                }
                continue;
            }

            // 3. Strip zero-width / invisible unicode characters.
            match ch {
                '\u{200B}' // zero-width space
                | '\u{200C}' // zero-width non-joiner
                | '\u{200D}' // zero-width joiner
                | '\u{200E}' // LTR mark
                | '\u{200F}' // RTL mark
                | '\u{FEFF}' // BOM / zero-width no-break space
                | '\u{2060}' // word joiner
                | '\u{00AD}' // soft hyphen
                => continue,
                _ => out.push(ch),
            }
        }

        out
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

/// Build visual row breaks for a line of text.
/// Returns a Vec of (start_char_idx, end_char_idx) for each visual row.
pub(crate) fn visual_rows_for(line_text: &str, tab_w: usize, text_area_width: usize) -> Vec<(usize, usize)> {
    let mut rows: Vec<(usize, usize)> = Vec::new();
    let mut row_start: usize = 0;
    let mut screen_col: usize = 0;
    let mut char_idx: usize = 0;

    for ch in line_text.chars() {
        if ch == '\n' || ch == '\r' {
            char_idx += 1;
            continue;
        }
        let ch_w = if ch == '\t' {
            tab_w - (screen_col % tab_w)
        } else {
            crate::utils::char_width(ch, tab_w)
        };

        if screen_col + ch_w > text_area_width && screen_col > 0 {
            rows.push((row_start, char_idx));
            row_start = char_idx;
            screen_col = 0;
        }
        screen_col += ch_w;
        char_idx += 1;
    }
    rows.push((row_start, char_idx));
    rows
}

/// Given a visual row spanning char indices [row_start..row_end) within `line_text`,
/// find the best char index matching `desired_col`.
fn col_in_visual_row_from_text(
    line_text: &str,
    line_len: usize,
    row_start: usize,
    row_end: usize,
    desired_col: usize,
    tab_w: usize,
) -> usize {
    let mut vcol: usize = 0;
    let mut best_col = row_start;

    for (i, ch) in line_text.chars().enumerate() {
        if i < row_start {
            continue;
        }
        if i >= row_end {
            break;
        }
        if ch == '\n' || ch == '\r' {
            break;
        }

        if vcol >= desired_col {
            return i.min(line_len);
        }

        let ch_w = if ch == '\t' {
            tab_w - (vcol % tab_w)
        } else {
            crate::utils::char_width(ch, tab_w)
        };
        vcol += ch_w;
        best_col = i + 1;
    }

    best_col.min(line_len)
}

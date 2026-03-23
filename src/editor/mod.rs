pub mod commands;
pub mod cursor;
pub mod mode;

use crate::buffer::Buffer;
use crate::config::Config;
use crate::editor::commands::Command;
use crate::editor::cursor::CursorSet;
use crate::editor::mode::Mode;

use crossterm::terminal;

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
            clipboard: String::new(),
            select_anchor: None,
            terminal_width: tw,
            terminal_height: th,
            suppress_next_paste: false,
            show_help: false,
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

            // -- Search (stubs) --
            Command::SearchForward => {
                self.set_status("Search: not yet implemented (Ctrl+F)");
            }
            Command::SearchNext | Command::SearchPrev => {}

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

    /// Clear the active selection.
    fn clear_selection(&mut self) {
        self.select_anchor = None;
        self.mode = Mode::Editing;
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
    }

    /// Move cursor forward one word.
    fn move_word_forward(&mut self) {
        let pos = self.cursor_char_pos();
        let total = self.buffer().text.len_chars();
        if pos >= total {
            return;
        }
        let mut i = pos;
        while i < total && is_word_char(self.buffer().text.char_at(i)) {
            i += 1;
        }
        while i < total && !is_word_char(self.buffer().text.char_at(i)) {
            i += 1;
        }
        let new_line = self.buffer().text.char_to_line(i);
        let new_col = i - self.buffer().text.line_to_char(new_line);
        self.cursors.set_cursor(new_line, new_col);
    }

    /// Move cursor backward one word.
    fn move_word_backward(&mut self) {
        let pos = self.cursor_char_pos();
        if pos == 0 {
            return;
        }
        let mut i = pos - 1;
        while i > 0 && !is_word_char(self.buffer().text.char_at(i)) {
            i -= 1;
        }
        while i > 0 && is_word_char(self.buffer().text.char_at(i - 1)) {
            i -= 1;
        }
        let new_line = self.buffer().text.char_to_line(i);
        let new_col = i - self.buffer().text.line_to_char(new_line);
        self.cursors.set_cursor(new_line, new_col);
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

/// Simple word-character check (alphanumeric + underscore).
fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

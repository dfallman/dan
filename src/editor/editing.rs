use crate::editor::Editor;

impl Editor {
    /// Get the char position in the buffer for the current cursor.
    pub(crate) fn cursor_char_pos(&self) -> usize {
        let c = self.cursors.cursor();
        let line_start = self.buffer().text.line_to_char(c.line);
        let line_len = self.buffer().text.line_len_chars(c.line);
        line_start + c.col.min(line_len)
    }

    /// Get the length of a line excluding the trailing newline.
    pub(crate) fn line_len_no_newline(&self, line: usize) -> usize {
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

    /// Swap the current line with the line above it. Cursor follows.
    pub(crate) fn swap_line_up(&mut self) {
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
    pub(crate) fn swap_line_down(&mut self) {
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

    /// Sanitize pasted / externally-sourced text.
    ///
    /// Strips:
    ///  - HTML / XML tags  (e.g. `<span style="...">`, `</p>`, `<br/>`)
    ///  - Carriage returns  (`\r`) — we normalise to Unix `\n`.
    ///  - Zero-width chars  (U+200B .. U+200F, U+FEFF BOM, U+2060 WJ).
    pub(crate) fn sanitize_paste(text: &str) -> String {
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
}

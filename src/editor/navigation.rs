use crate::editor::viewport::{col_in_visual_row_from_text, visual_rows_for};
use crate::editor::Editor;
use unicode_segmentation::UnicodeSegmentation;

impl Editor {
    /// Move cursor horizontally by `delta` chars (-1 = left, 1 = right).
    pub(crate) fn move_cursor_horizontal(&mut self, delta: i32) {
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
    pub(crate) fn move_cursor_vertical(&mut self, delta: i32) {
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

    /// Move cursor forward one word using UAX #29 word boundaries.
    pub(crate) fn move_word_forward(&mut self) {
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
                    self.cursors.primary_mut().head.line = line;
                    self.cursors.primary_mut().head.set_col(b);
                    return;
                }
            }
            // If content_len is past us, go there
            if content_len > col {
                self.cursors.primary_mut().head.line = line;
                self.cursors.primary_mut().head.set_col(content_len);
                return;
            }
            // Move to next line, col 0
            line += 1;
            col = 0;
        }
    }

    /// Move cursor backward one word using UAX #29 word boundaries.
    pub(crate) fn move_word_backward(&mut self) {
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
                    self.cursors.primary_mut().head.line = line;
                    self.cursors.primary_mut().head.set_col(b);
                    return;
                }
            }
            // No boundary found before us on this line — go to end of previous line
            if line == 0 {
                self.cursors.primary_mut().head.line = 0;
                self.cursors.primary_mut().head.set_col(0);
                return;
            }
            line -= 1;
            let prev_line_text: String = self.buffer().text.line_slice(line).chars().collect();
            col = prev_line_text.trim_end_matches('\n').chars().count();
        }
    }
}

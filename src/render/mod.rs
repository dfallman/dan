use crossterm::{
    cursor,
    style::{self, Color, SetBackgroundColor, SetForegroundColor},
    terminal,
    QueueableCommand,
};
use std::io::{self, Write};

use crate::editor::Editor;
use crate::editor::mode::Mode;
use crate::editor::visual_rows_for;
use crate::utils::char_width;

/// Viewport dimensions, cached from the Editor.
pub struct Viewport {
    pub width: u16,
    pub height: u16,
    /// How many chrome rows at the bottom (status bar + optional help bar).
    pub chrome_rows: u16,
}

impl Viewport {
    /// Query the actual terminal size and sync the editor's cached values.
    pub fn from_editor(editor: &mut Editor) -> Self {
        let (w, h) = terminal::size().unwrap_or((editor.terminal_width, editor.terminal_height));
        editor.terminal_width = w;
        editor.terminal_height = h;
        // 1 row for the status bar, plus 1 more if help legend is shown,
        // plus 1 more if the search prompt is active.
        let mut chrome: u16 = 1;
        if editor.show_help {
            chrome += 1;
        }
        if editor.mode == Mode::Searching {
            chrome += 1;
        }
        Self {
            width: w,
            height: h,
            chrome_rows: chrome,
        }
    }

    /// Height available for text (total height minus chrome rows).
    pub fn text_height(&self) -> u16 {
        self.height.saturating_sub(self.chrome_rows)
    }
}

/// A reusable buffer of spaces for padding lines — avoids allocating
/// a new `String` every time we need to pad.
const PAD_CHUNK: &str = "                                                                                                                                                                                                                                                                ";

/// Write `n` space characters by repeatedly printing from PAD_CHUNK.
fn write_spaces<W: Write>(w: &mut W, n: usize) -> io::Result<()> {
    let mut remaining = n;
    while remaining > 0 {
        let chunk = remaining.min(PAD_CHUNK.len());
        w.queue(style::Print(&PAD_CHUNK[..chunk]))?;
        remaining -= chunk;
    }
    Ok(())
}

/// Render the full editor frame to the terminal.
pub fn render<W: Write>(editor: &mut Editor, w: &mut W) -> io::Result<()> {
    let vp = Viewport::from_editor(editor);
    let text_height = vp.text_height() as usize;

    // Adjust scroll to keep cursor visible (with scroll_off padding)
    let cursor_line = editor.cursors.cursor().line;
    let scroll_off = editor.config.scroll_off;
    if editor.config.wrap_lines {
        // Wrap mode: scroll must account for visual rows, not just buffer lines.
        // Re-derive text_area_width for the helper (gutter not computed yet, use temp)
        let line_count_tmp = editor.buffer().line_count();
        let gw_tmp = if editor.config.line_numbers {
            line_number_width(line_count_tmp)
        } else { 0 };
        let taw_tmp = (vp.width as usize).saturating_sub(gw_tmp + 1);
        let tab_w = editor.config.tab_width;
        if taw_tmp > 0 {
            // Find which visual row the cursor is on within its buffer line.
            let cur_text: String = editor.buffer().text.line_slice(cursor_line).chars().collect();
            let cur_vrows = visual_rows_for(&cur_text, tab_w, taw_tmp);
            let cursor_col = editor.cursors.cursor().col;
            let mut cur_vrow_idx = cur_vrows.len() - 1;
            for (i, &(start, end)) in cur_vrows.iter().enumerate() {
                if cursor_col >= start && (cursor_col < end || i == cur_vrows.len() - 1) {
                    cur_vrow_idx = i;
                    break;
                }
            }

            // --- Scroll UP: ensure scroll_off visual rows above the cursor ---
            // First, clamp scroll_y so it never goes past the cursor line.
            if editor.scroll_y > cursor_line {
                editor.scroll_y = cursor_line;
            }
            // Count visual rows from scroll_y to the cursor's visual row.
            // If it's less than scroll_off, scroll up.
            loop {
                if editor.scroll_y == 0 { break; }
                let mut rows_above: usize = 0;
                for bl in editor.scroll_y..cursor_line {
                    let lt: String = editor.buffer().text.line_slice(bl).chars().collect();
                    rows_above += visual_rows_for(&lt, tab_w, taw_tmp).len();
                }
                rows_above += cur_vrow_idx; // cursor's sub-row within its line
                if rows_above >= scroll_off { break; }
                editor.scroll_y -= 1;
            }

            // --- Scroll DOWN: ensure scroll_off visual rows below the cursor ---
            // The cursor's visual row (from top of viewport) must be at most
            // text_height - 1 - scroll_off.
            let max_row = text_height.saturating_sub(1 + scroll_off);
            loop {
                let mut vrow_from_top: usize = 0;
                for bl in editor.scroll_y..cursor_line {
                    let lt: String = editor.buffer().text.line_slice(bl).chars().collect();
                    vrow_from_top += visual_rows_for(&lt, tab_w, taw_tmp).len();
                }
                vrow_from_top += cur_vrow_idx;
                if vrow_from_top <= max_row {
                    break;
                }
                editor.scroll_y += 1;
                if editor.scroll_y > cursor_line {
                    editor.scroll_y = cursor_line;
                    break;
                }
            }
        }
    } else {
        if cursor_line < editor.scroll_y + scroll_off {
            editor.scroll_y = cursor_line.saturating_sub(scroll_off);
        }
        if cursor_line + scroll_off >= editor.scroll_y + text_height {
            editor.scroll_y = (cursor_line + scroll_off).saturating_sub(text_height) + 1;
        }
    }

    // -- Horizontal scroll adjustment (only when wrap_lines = false) --
    let line_count = editor.buffer().line_count();
    let show_line_numbers = editor.config.line_numbers;
    let gutter_width = if show_line_numbers {
        line_number_width(line_count)
    } else {
        0
    };
    let text_area_width = (vp.width as usize).saturating_sub(gutter_width + 1);

    if !editor.config.wrap_lines {
        // Compute the cursor's visual column so we can center scroll_x on it.
        let cursor_pos = editor.cursors.cursor();
        let tab_w = editor.config.tab_width;
        let cursor_vcol = if cursor_pos.line < line_count {
            let lsl = editor.buffer().text.line_slice(cursor_pos.line);
            let mut vc: usize = 0;
            for (i, ch) in lsl.chars().enumerate() {
                if i >= cursor_pos.col { break; }
                if ch == '\t' { vc += tab_w - (vc % tab_w); }
                else { vc += char_width(ch, tab_w); }
            }
            vc
        } else {
            cursor_pos.col
        };
        let h_margin: usize = 5;
        if cursor_vcol < editor.scroll_x + h_margin {
            editor.scroll_x = cursor_vcol.saturating_sub(h_margin);
        }
        if cursor_vcol >= editor.scroll_x + text_area_width.saturating_sub(h_margin) {
            editor.scroll_x = cursor_vcol.saturating_sub(text_area_width.saturating_sub(h_margin + 1));
        }
    } else {
        editor.scroll_x = 0;
    }

    w.queue(cursor::Hide)?;
    w.queue(cursor::MoveTo(0, 0))?;

    // -- Render text lines --
    // Get selection range for highlighting
    let sel_range = editor.selection_range();

    if editor.config.wrap_lines {
        // ---- Soft-wrap mode ----
        // Each buffer line may occupy multiple screen rows.
        let mut screen_row: usize = 0;
        let mut buf_line = editor.scroll_y;

        while screen_row < text_height && buf_line < line_count {
            // -- First screen row of this buffer line: draw the real line number --
            w.queue(cursor::MoveTo(0, screen_row as u16))?;
            w.queue(SetForegroundColor(Color::Reset))?;
            w.queue(SetBackgroundColor(Color::Reset))?;

            if show_line_numbers {
                let line_num = format!("{:>width$} ", buf_line + 1, width = gutter_width);
                if buf_line == cursor_line {
                    w.queue(SetForegroundColor(Color::Blue))?;
                } else {
                    w.queue(SetForegroundColor(Color::White))?;
                }
                w.queue(style::Print(&line_num))?;
                w.queue(SetForegroundColor(Color::Reset))?;
            }

            let line_slice = editor.buffer().text.line_slice(buf_line);
            let line_start_pos = editor.buffer().text.line_to_char(buf_line);
            let tab_w = editor.config.tab_width;

            let mut batch = String::new();
            let mut in_sel = false;
            let mut prev_in_search = false;
            let mut prev_is_current = false;
            let mut vcol: usize = 0;
            let mut char_idx: usize = 0;
            let mut screen_col: usize = 0; // columns written on current screen row

            for ch in line_slice.chars() {
                if ch == '\n' || ch == '\r' {
                    char_idx += 1;
                    continue;
                }

                // Compute the width this char will take
                let ch_w = if ch == '\t' { tab_w - (vcol % tab_w) } else { char_width(ch, tab_w) };

                // If this char would overflow the current screen row, wrap.
                if screen_col + ch_w > text_area_width {
                    // Flush batch
                    if !batch.is_empty() {
                        w.queue(style::Print(&batch))?;
                        batch.clear();
                    }
                    if in_sel || prev_in_search {
                        w.queue(SetBackgroundColor(Color::Reset))?;
                        w.queue(SetForegroundColor(Color::Reset))?;
                    }
                    // Pad rest of this row
                    let remaining = text_area_width.saturating_sub(screen_col);
                    if remaining > 0 {
                        write_spaces(w, remaining)?;
                    }
                    screen_row += 1;
                    if screen_row >= text_height { break; }

                    // Start new screen row — continuation with ↳ gutter
                    w.queue(cursor::MoveTo(0, screen_row as u16))?;
                    w.queue(SetForegroundColor(Color::Reset))?;
                    w.queue(SetBackgroundColor(Color::Reset))?;
                    if show_line_numbers {
                        // Right-align ↳ within the gutter width, followed by separator space
                        let wrap_gutter = format!("{:>width$} ", "↳", width = gutter_width);
                        if buf_line == cursor_line {
                            w.queue(SetForegroundColor(Color::Blue))?;
                        } else {
                            w.queue(SetForegroundColor(Color::DarkGrey))?;
                        }
                        w.queue(style::Print(&wrap_gutter))?;
                        w.queue(SetForegroundColor(Color::Reset))?;
                    }
                    screen_col = 0;
                    in_sel = false;
                    prev_in_search = false;
                    prev_is_current = false;
                }

                // Highlight logic
                let char_pos = line_start_pos + char_idx;
                let want_sel = if let Some((sel_start, sel_end)) = sel_range {
                    char_pos >= sel_start && char_pos < sel_end
                } else {
                    false
                };
                let search_hit = editor.search_matches.iter().enumerate().find(
                    |(_i, &(ms, me))| char_pos >= ms && char_pos < me,
                );
                let is_current_match = search_hit.as_ref().map(|(i, _)| *i == editor.search_match_idx).unwrap_or(false);
                let in_search = search_hit.is_some();

                let want_state = (want_sel, in_search, is_current_match);
                let prev_state = (in_sel, prev_in_search, prev_is_current);
                if want_state != prev_state {
                    if !batch.is_empty() {
                        w.queue(style::Print(&batch))?;
                        batch.clear();
                    }
                    if want_sel {
                        w.queue(SetBackgroundColor(Color::Rgb { r: 70, g: 130, b: 180 }))?;
                        w.queue(SetForegroundColor(Color::White))?;
                    } else if is_current_match {
                        w.queue(SetBackgroundColor(Color::Rgb { r: 255, g: 165, b: 0 }))?;
                        w.queue(SetForegroundColor(Color::Black))?;
                    } else if in_search {
                        w.queue(SetBackgroundColor(Color::Rgb { r: 180, g: 180, b: 60 }))?;
                        w.queue(SetForegroundColor(Color::Black))?;
                    } else {
                        w.queue(SetBackgroundColor(Color::Reset))?;
                        w.queue(SetForegroundColor(Color::Reset))?;
                    }
                    in_sel = want_sel;
                    prev_in_search = in_search;
                    prev_is_current = is_current_match;
                }

                if ch == '\t' {
                    let spaces = ch_w;
                    for _ in 0..spaces {
                        batch.push(' ');
                    }
                } else {
                    batch.push(ch);
                }
                vcol += ch_w;
                screen_col += ch_w;
                char_idx += 1;
            }

            // Flush remaining batch for this buffer line
            if !batch.is_empty() {
                w.queue(style::Print(&batch))?;
            }
            if in_sel || prev_in_search {
                w.queue(SetBackgroundColor(Color::Reset))?;
                w.queue(SetForegroundColor(Color::Reset))?;
            }

            // Pad rest of the last screen row for this buffer line
            let cols_used = gutter_width + 1 + screen_col;
            let remaining = (vp.width as usize).saturating_sub(cols_used);
            if remaining > 0 {
                write_spaces(w, remaining)?;
            }

            screen_row += 1;
            buf_line += 1;
        }

        // Fill remaining screen rows past EOF with tilde gutters
        while screen_row < text_height {
            w.queue(cursor::MoveTo(0, screen_row as u16))?;
            w.queue(SetForegroundColor(Color::Reset))?;
            w.queue(SetBackgroundColor(Color::Reset))?;
            let mut cols_written: usize = 0;
            if show_line_numbers {
                let tilde_gutter = format!("{:>width$} ", "~", width = gutter_width);
                w.queue(SetForegroundColor(Color::DarkGrey))?;
                w.queue(style::Print(&tilde_gutter))?;
                w.queue(SetForegroundColor(Color::Reset))?;
                cols_written = gutter_width + 1;
            }
            let remaining = (vp.width as usize).saturating_sub(cols_written);
            if remaining > 0 {
                write_spaces(w, remaining)?;
            }
            screen_row += 1;
        }
    } else {
        // ---- No-wrap mode (horizontal scroll) ----
        let sx = editor.scroll_x;

        for row in 0..text_height {
            let line_idx = editor.scroll_y + row;
            w.queue(cursor::MoveTo(0, row as u16))?;
            w.queue(SetForegroundColor(Color::Reset))?;
            w.queue(SetBackgroundColor(Color::Reset))?;

            let mut cols_written: usize = 0;

            if line_idx < line_count {
                if show_line_numbers {
                    let line_num = format!("{:>width$} ", line_idx + 1, width = gutter_width);
                    cols_written += line_num.len();
                    if line_idx == cursor_line {
                        w.queue(SetForegroundColor(Color::Blue))?;
                    } else {
                        w.queue(SetForegroundColor(Color::White))?;
                    }
                    w.queue(style::Print(&line_num))?;
                    w.queue(SetForegroundColor(Color::Reset))?;
                }

                let line_slice = editor.buffer().text.line_slice(line_idx);
                let line_start_pos = editor.buffer().text.line_to_char(line_idx);
                let tab_w = editor.config.tab_width;

                let mut batch = String::new();
                let mut in_sel = false;
                let mut prev_in_search = false;
                let mut prev_is_current = false;
                let mut vcol: usize = 0;
                let mut visible_written: usize = 0;
                let mut char_idx: usize = 0;

                for ch in line_slice.chars() {
                    if visible_written >= text_area_width {
                        break;
                    }
                    if ch == '\n' || ch == '\r' {
                        char_idx += 1;
                        continue;
                    }

                    let ch_w = if ch == '\t' { tab_w - (vcol % tab_w) } else { char_width(ch, tab_w) };
                    let vcol_end = vcol + ch_w;

                    // Skip chars entirely before scroll_x
                    if vcol_end <= sx {
                        vcol = vcol_end;
                        char_idx += 1;
                        continue;
                    }

                    // Highlight logic
                    let char_pos = line_start_pos + char_idx;
                    let want_sel = if let Some((sel_start, sel_end)) = sel_range {
                        char_pos >= sel_start && char_pos < sel_end
                    } else {
                        false
                    };
                    let search_hit = editor.search_matches.iter().enumerate().find(
                        |(_i, &(ms, me))| char_pos >= ms && char_pos < me,
                    );
                    let is_current_match = search_hit.as_ref().map(|(i, _)| *i == editor.search_match_idx).unwrap_or(false);
                    let in_search = search_hit.is_some();

                    let want_state = (want_sel, in_search, is_current_match);
                    let prev_state = (in_sel, prev_in_search, prev_is_current);
                    if want_state != prev_state {
                        if !batch.is_empty() {
                            w.queue(style::Print(&batch))?;
                            batch.clear();
                        }
                        if want_sel {
                            w.queue(SetBackgroundColor(Color::Rgb { r: 70, g: 130, b: 180 }))?;
                            w.queue(SetForegroundColor(Color::White))?;
                        } else if is_current_match {
                            w.queue(SetBackgroundColor(Color::Rgb { r: 255, g: 165, b: 0 }))?;
                            w.queue(SetForegroundColor(Color::Black))?;
                        } else if in_search {
                            w.queue(SetBackgroundColor(Color::Rgb { r: 180, g: 180, b: 60 }))?;
                            w.queue(SetForegroundColor(Color::Black))?;
                        } else {
                            w.queue(SetBackgroundColor(Color::Reset))?;
                            w.queue(SetForegroundColor(Color::Reset))?;
                        }
                        in_sel = want_sel;
                        prev_in_search = in_search;
                        prev_is_current = is_current_match;
                    }

                    // Render the character (may be partially clipped at scroll_x boundary)
                    if ch == '\t' {
                        // Number of visible spaces from this tab
                        let start = if vcol < sx { sx } else { vcol };
                        let vis_spaces = vcol_end.saturating_sub(start).min(text_area_width - visible_written);
                        for _ in 0..vis_spaces {
                            batch.push(' ');
                        }
                        visible_written += vis_spaces;
                    } else {
                        batch.push(ch);
                        visible_written += ch_w;
                    }
                    vcol = vcol_end;
                    char_idx += 1;
                }

                if !batch.is_empty() {
                    w.queue(style::Print(&batch))?;
                }
                if in_sel || prev_in_search {
                    w.queue(SetBackgroundColor(Color::Reset))?;
                    w.queue(SetForegroundColor(Color::Reset))?;
                }
                cols_written += gutter_width + 1 + visible_written;
            } else {
                if show_line_numbers {
                    let line_num = format!("{:>width$} ", line_idx + 1, width = gutter_width);
                    cols_written += line_num.len();
                    w.queue(SetForegroundColor(Color::DarkGrey))?;
                    w.queue(style::Print(&line_num))?;
                    w.queue(SetForegroundColor(Color::Reset))?;
                }
            }

            let remaining = (vp.width as usize).saturating_sub(cols_written);
            if remaining > 0 {
                write_spaces(w, remaining)?;
            }
        }
    }

    // -- Render status bar (always the row just after text) --
    render_status_bar(editor, w, &vp)?;

    // -- Render help bar (only when toggled on with ^H) --
    if editor.show_help {
        render_help_bar(editor, w, &vp)?;
    }

    // -- Render search prompt (when in search mode) --
    if editor.mode == Mode::Searching {
        render_search_bar(editor, w, &vp)?;
    }

    // -- Position the cursor --
    if editor.mode == Mode::Searching {
        // During search, draw an outline cursor in the document at the saved position.
        if let Some((saved_line, saved_col)) = editor.search_saved_cursor {
            if saved_line >= editor.scroll_y && saved_line < editor.scroll_y + text_height {
                let saved_screen_y = (saved_line - editor.scroll_y) as u16;
                let tab_w = editor.config.tab_width;
                let saved_visual_col = if saved_line < line_count {
                    let line_slice = editor.buffer().text.line_slice(saved_line);
                    let mut vc: usize = 0;
                    for (i, ch) in line_slice.chars().enumerate() {
                        if i >= saved_col {
                            break;
                        }
                        if ch == '\t' {
                            vc += tab_w - (vc % tab_w);
                        } else {
                            vc += char_width(ch, tab_w);
                        }
                    }
                    vc
                } else {
                    saved_col
                };
                let outline_x = (gutter_width + 1 + saved_visual_col.saturating_sub(editor.scroll_x)) as u16;
                // Draw the character (or space) at that position with an underline-style outline.
                w.queue(cursor::MoveTo(outline_x, saved_screen_y))?;
                w.queue(SetBackgroundColor(Color::DarkGrey))?;
                w.queue(SetForegroundColor(Color::White))?;
                w.queue(style::SetAttribute(style::Attribute::Underlined))?;
                // Print the actual character at the cursor position, or a space if past EOL.
                let outline_ch = if saved_line < line_count {
                    let line_slice = editor.buffer().text.line_slice(saved_line);
                    line_slice.chars().nth(saved_col)
                        .filter(|c| *c != '\n' && *c != '\r')
                        .unwrap_or(' ')
                } else {
                    ' '
                };
                if outline_ch == '\t' {
                    w.queue(style::Print(" "))?;
                } else {
                    w.queue(style::Print(format!("{}", outline_ch)))?;
                }
                w.queue(style::SetAttribute(style::Attribute::NoUnderline))?;
                w.queue(SetBackgroundColor(Color::Reset))?;
                w.queue(SetForegroundColor(Color::Reset))?;
            }
        }

        // Place the real cursor at the end of the query text in the search bar.
        let search_y = if editor.show_help {
            vp.height.saturating_sub(2)
        } else {
            vp.height.saturating_sub(1)
        };
        let label_len = 7; // " FIND: "
        let cursor_x = (label_len + 1 + editor.search_query.len()) as u16; // +1 for leading space in query display
        w.queue(cursor::MoveTo(cursor_x, search_y))?;
        w.queue(cursor::Show)?;
        w.queue(cursor::SetCursorStyle::BlinkingBar)?;
    } else {
        // Normal / Selecting mode — position cursor in the document.
        let cursor_pos = editor.cursors.cursor();
        let tab_w = editor.config.tab_width;

        let (screen_y, visual_col) = if editor.config.wrap_lines && text_area_width > 0 {
            // Wrap mode: screen_y must count visual rows, visual_col is
            // relative to the start of the cursor's visual row.
            let mut sy: usize = 0;
            for bl in editor.scroll_y..cursor_pos.line.min(line_count) {
                let lt: String = editor.buffer().text.line_slice(bl).chars().collect();
                sy += visual_rows_for(&lt, tab_w, text_area_width).len();
            }
            // Find cursor's visual row within its buffer line.
            let (vrow_idx, vrow_start) = if cursor_pos.line < line_count {
                let lt: String = editor.buffer().text.line_slice(cursor_pos.line).chars().collect();
                let vrows = visual_rows_for(&lt, tab_w, text_area_width);
                let mut idx = vrows.len() - 1;
                for (i, &(start, end)) in vrows.iter().enumerate() {
                    if cursor_pos.col >= start && (cursor_pos.col < end || i == vrows.len() - 1) {
                        idx = i;
                        break;
                    }
                }
                (idx, vrows[idx].0)
            } else {
                (0, 0)
            };
            sy += vrow_idx;

            // Compute visual column from the visual row's start char.
            let vc = if cursor_pos.line < line_count {
                let line_slice = editor.buffer().text.line_slice(cursor_pos.line);
                let mut v: usize = 0;
                for (i, ch) in line_slice.chars().enumerate() {
                    if i < vrow_start { continue; }
                    if i >= cursor_pos.col { break; }
                    if ch == '\t' { v += tab_w - (v % tab_w); }
                    else { v += char_width(ch, tab_w); }
                }
                v
            } else {
                cursor_pos.col
            };
            (sy, vc)
        } else {
            // No-wrap mode: 1 buffer line = 1 screen row.
            let sy = cursor_pos.line.saturating_sub(editor.scroll_y);
            let vc = if cursor_pos.line < line_count {
                let line_slice = editor.buffer().text.line_slice(cursor_pos.line);
                let mut v: usize = 0;
                for (i, ch) in line_slice.chars().enumerate() {
                    if i >= cursor_pos.col { break; }
                    if ch == '\t' { v += tab_w - (v % tab_w); }
                    else { v += char_width(ch, tab_w); }
                }
                v
            } else {
                cursor_pos.col
            };
            (sy, vc)
        };
        let screen_x = (gutter_width + 1 + visual_col.saturating_sub(editor.scroll_x)) as u16;
        w.queue(cursor::MoveTo(screen_x, screen_y as u16))?;
        w.queue(cursor::Show)?;

        // Pico-style: always use a steady block cursor (like a normal text editor)
        w.queue(cursor::SetCursorStyle::SteadyBlock)?;
    }

    w.flush()?;
    Ok(())
}

/// Render the status bar.
fn render_status_bar<W: Write>(
    editor: &Editor,
    w: &mut W,
    vp: &Viewport,
) -> io::Result<()> {
    let status_y = vp.height.saturating_sub(vp.chrome_rows);
    w.queue(cursor::MoveTo(0, status_y))?;

    let width = vp.width as usize;
    let mut used: usize = 0;

    // Mode indicator
    let (r, g, b) = editor.mode.color();
    let mode_label = format!(" {} ", editor.mode.label());
    w.queue(SetBackgroundColor(Color::Rgb { r, g, b }))?;
    w.queue(SetForegroundColor(Color::Black))?;
    w.queue(style::Print(&mode_label))?;
    used += mode_label.len();

    w.queue(SetBackgroundColor(Color::White))?;
    w.queue(SetForegroundColor(Color::Black))?;

    // File name
    let name = editor.buffer().display_name();
    let dirty = if editor.buffer().dirty { " [+]" } else { "" };
    let file_part = format!(" {}{} ", name, dirty);
    w.queue(style::Print(&file_part))?;
    used += file_part.len();

    // Status message (if any)
    if let Some(ref msg) = editor.status_msg {
        let msg_part = format!(" {} ", msg);
        w.queue(SetForegroundColor(Color::Blue))?;
        w.queue(style::Print(&msg_part))?;
        w.queue(SetForegroundColor(Color::Black))?;
        used += msg_part.len();
    }

    // Right side: ^H Help toggle + cursor position
    let c = editor.cursors.cursor();
    let right = format!(" ^H Help  Ln {}, Col {} ", c.line + 1, c.col + 1);
    let padding = width.saturating_sub(used + right.len());
    write_spaces(w, padding)?;
    w.queue(style::Print(&right))?;

    w.queue(SetBackgroundColor(Color::Reset))?;
    w.queue(SetForegroundColor(Color::Reset))?;

    Ok(())
}

/// Render the pico-style help bar at the bottom of the screen.
fn render_help_bar<W: Write>(
    _editor: &Editor,
    w: &mut W,
    vp: &Viewport,
) -> io::Result<()> {
    let help_y = vp.height.saturating_sub(1);
    w.queue(cursor::MoveTo(0, help_y))?;

    // Pico/nano-style shortcut hints
    let shortcuts = [
        ("^S", "Save"),
        ("^Q", "Quit"),
        ("^Z", "Undo"),
        ("^Y", "Redo"),
        ("^C", "Copy"),
        ("^X", "Cut"),
        ("^V", "Paste"),
        ("^F", "Find"),
        ("^K", "Del Ln"),
        ("^A", "Sel All"),
        ("^W", "Wrap"),
        ("^H", "Help"),
    ];

    let mut used: usize = 0;
    for (key, label) in &shortcuts {
        // Key in inverse video
        w.queue(SetBackgroundColor(Color::White))?;
        w.queue(SetForegroundColor(Color::Black))?;
        w.queue(style::Print(key))?;
        w.queue(SetBackgroundColor(Color::Reset))?;
        w.queue(SetForegroundColor(Color::DarkGrey))?;
        let lbl = format!(" {} ", label);
        w.queue(style::Print(&lbl))?;
        used += key.len() + lbl.len();
    }

    // Pad rest of the line
    let remaining = (vp.width as usize).saturating_sub(used);
    if remaining > 0 {
        w.queue(SetBackgroundColor(Color::Reset))?;
        write_spaces(w, remaining)?;
    }

    w.queue(SetForegroundColor(Color::Reset))?;
    w.queue(SetBackgroundColor(Color::Reset))?;

    Ok(())
}

/// Render the search prompt bar (appears below the status bar).
fn render_search_bar<W: Write>(
    editor: &Editor,
    w: &mut W,
    vp: &Viewport,
) -> io::Result<()> {
    // Search bar sits just below the status bar.
    // With help: status_y = h - chrome, help_y = h - 1, search_y = h - chrome + 1
    // Without help: status_y = h - chrome, search_y = h - 1
    let search_y = if editor.show_help {
        vp.height.saturating_sub(2)
    } else {
        vp.height.saturating_sub(1)
    };
    w.queue(cursor::MoveTo(0, search_y))?;

    let width = vp.width as usize;
    let mut used: usize = 0;

    // Label
    w.queue(SetBackgroundColor(Color::Rgb {
        r: 255,
        g: 165,
        b: 0,
    }))?;
    w.queue(SetForegroundColor(Color::Black))?;
    let label = " FIND: ";
    w.queue(style::Print(label))?;
    used += label.len();

    // Query text
    w.queue(SetBackgroundColor(Color::DarkGrey))?;
    w.queue(SetForegroundColor(Color::White))?;
    let query_display = format!(" {} ", editor.search_query);
    w.queue(style::Print(&query_display))?;
    used += query_display.len();

    // Match count
    let info = if editor.search_matches.is_empty() {
        if editor.search_query.is_empty() {
            String::new()
        } else {
            " (no matches)".to_string()
        }
    } else {
        format!(
            " ({}/{})",
            editor.search_match_idx + 1,
            editor.search_matches.len()
        )
    };
    if !info.is_empty() {
        w.queue(SetForegroundColor(Color::Rgb {
            r: 200,
            g: 200,
            b: 200,
        }))?;
        w.queue(style::Print(&info))?;
        used += info.len();
    }

    // Pad the rest
    let remaining = width.saturating_sub(used);
    if remaining > 0 {
        write_spaces(w, remaining)?;
    }

    w.queue(SetBackgroundColor(Color::Reset))?;
    w.queue(SetForegroundColor(Color::Reset))?;

    Ok(())
}

/// Calculate the width needed for line numbers.
fn line_number_width(total_lines: usize) -> usize {
    if total_lines == 0 {
        1
    } else {
        (total_lines as f64).log10().floor() as usize + 1
    }
}

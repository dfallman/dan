use crossterm::{
    cursor,
    style::{self, Color, SetBackgroundColor, SetForegroundColor},
    terminal,
    QueueableCommand,
};
use std::io::{self, Write};

use crate::editor::Editor;
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
        // 1 row for the status bar, plus 1 more if help legend is shown
        let chrome = if editor.show_help { 2 } else { 1 };
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
    if cursor_line < editor.scroll_y + scroll_off {
        editor.scroll_y = cursor_line.saturating_sub(scroll_off);
    }
    if cursor_line + scroll_off >= editor.scroll_y + text_height {
        editor.scroll_y = (cursor_line + scroll_off).saturating_sub(text_height) + 1;
    }

    w.queue(cursor::Hide)?;
    w.queue(cursor::MoveTo(0, 0))?;

    // -- Render text lines --
    let line_count = editor.buffer().line_count();
    let show_line_numbers = editor.config.line_numbers;
    let gutter_width = if show_line_numbers {
        line_number_width(line_count)
    } else {
        0
    };

    // Get selection range for highlighting
    let sel_range = editor.selection_range();

    for row in 0..text_height {
        let line_idx = editor.scroll_y + row;
        w.queue(cursor::MoveTo(0, row as u16))?;
        // Ensure clean color state at the start of each row (Terminal.app compat).
        w.queue(SetForegroundColor(Color::Reset))?;
        w.queue(SetBackgroundColor(Color::Reset))?;

        // Track how many columns we've written so we can space-pad the rest.
        let mut cols_written: usize = 0;

        if line_idx < line_count {
            // Line number gutter — Cyan for active line, White for others
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

            // Line text — iterate RopeSlice chars directly (zero-alloc)
            let line_slice = editor.buffer().text.line_slice(line_idx);
            let max_text_width = (vp.width as usize).saturating_sub(gutter_width + 1);
            let line_start_pos = editor.buffer().text.line_to_char(line_idx);
            let tab_w = editor.config.tab_width;

            let mut batch = String::new();
            let mut in_sel = false;
            let mut vcol: usize = 0; // visual column
            let mut char_idx: usize = 0; // char index within line

            for ch in line_slice.chars() {
                if vcol >= max_text_width {
                    break;
                }
                if ch == '\n' || ch == '\r' {
                    char_idx += 1;
                    continue;
                }

                let char_pos = line_start_pos + char_idx;
                let want_sel = if let Some((sel_start, sel_end)) = sel_range {
                    char_pos >= sel_start && char_pos < sel_end
                } else {
                    false
                };

                // If the selection state changes, flush the accumulated batch.
                if want_sel != in_sel {
                    if !batch.is_empty() {
                        w.queue(style::Print(&batch))?;
                        batch.clear();
                    }
                    if want_sel {
                        w.queue(SetBackgroundColor(Color::Rgb {
                            r: 70,
                            g: 130,
                            b: 180,
                        }))?;
                        w.queue(SetForegroundColor(Color::White))?;
                    } else {
                        w.queue(SetBackgroundColor(Color::Reset))?;
                        w.queue(SetForegroundColor(Color::Reset))?;
                    }
                    in_sel = want_sel;
                }

                // Expand tabs to spaces for correct visual width
                if ch == '\t' {
                    let spaces = tab_w - (vcol % tab_w);
                    for _ in 0..spaces.min(max_text_width - vcol) {
                        batch.push(' ');
                    }
                    vcol += spaces;
                } else {
                    batch.push(ch);
                    vcol += char_width(ch, tab_w);
                }
                char_idx += 1;
            }
            // Flush remaining batch
            if !batch.is_empty() {
                w.queue(style::Print(&batch))?;
            }
            if in_sel {
                w.queue(SetBackgroundColor(Color::Reset))?;
                w.queue(SetForegroundColor(Color::Reset))?;
            }
            cols_written += vcol;
        } else {
            // Past end of file — show line numbers in dim grey.
            let line_num = format!("{:>width$} ", line_idx + 1, width = gutter_width);
            cols_written += line_num.len();
            w.queue(SetForegroundColor(Color::DarkGrey))?;
            w.queue(style::Print(&line_num))?;
            w.queue(SetForegroundColor(Color::Reset))?;
        }

        // Pad with spaces to overwrite any stale content — uses static buffer.
        let remaining = (vp.width as usize).saturating_sub(cols_written);
        if remaining > 0 {
            write_spaces(w, remaining)?;
        }
    }

    // -- Render status bar (always the row just after text) --
    render_status_bar(editor, w, &vp)?;

    // -- Render help bar (only when toggled on with ^H) --
    if editor.show_help {
        render_help_bar(editor, w, &vp)?;
    }

    // -- Position the cursor --
    // Compute visual column by expanding tabs in the cursor's line.
    let cursor_pos = editor.cursors.cursor();
    let screen_y = cursor_pos.line.saturating_sub(editor.scroll_y) as u16;
    let tab_w = editor.config.tab_width;
    let visual_col = if cursor_pos.line < line_count {
        let line_slice = editor.buffer().text.line_slice(cursor_pos.line);
        let mut vc: usize = 0;
        for (i, ch) in line_slice.chars().enumerate() {
            if i >= cursor_pos.col {
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
        cursor_pos.col
    };
    let screen_x = (gutter_width + 1 + visual_col) as u16;
    w.queue(cursor::MoveTo(screen_x, screen_y))?;
    w.queue(cursor::Show)?;

    // Pico-style: always use a steady bar cursor (like a normal text editor)
    w.queue(cursor::SetCursorStyle::SteadyBlock)?;

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

/// Calculate the width needed for line numbers.
fn line_number_width(total_lines: usize) -> usize {
    if total_lines == 0 {
        1
    } else {
        (total_lines as f64).log10().floor() as usize + 1
    }
}

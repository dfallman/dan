use crossterm::{
    cursor,
    style::{self, Color, SetBackgroundColor, SetForegroundColor},
    QueueableCommand,
};
use std::io::{self, Write};

use crate::editor::Editor;
use super::{write_spaces, Viewport};

/// Render the status bar.
pub fn render_status_bar<W: Write>(
    editor: &Editor,
    w: &mut W,
    vp: &Viewport,
) -> io::Result<()> {
    let status_y = vp.height.saturating_sub(vp.chrome_rows);
    w.queue(cursor::MoveTo(0, status_y))?;

    let width = vp.width as usize;
    let mut used: usize = 0;

    // Mode indicator (derive visual mode from selection state)
    let (mode_bg, mode_text) = if editor.has_selection() {
        (Color::DarkYellow, "SELECT")
    } else {
        (editor.mode.color(), editor.mode.label())
    };
    let mode_label = format!(" {} ", mode_text);
    w.queue(SetBackgroundColor(mode_bg))?;
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
        w.queue(SetBackgroundColor(Color::DarkBlue))?;
        w.queue(SetForegroundColor(Color::Black))?;
        w.queue(style::Print(&msg_part))?;
        w.queue(SetBackgroundColor(Color::White))?;
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

/// Shortcut definitions for the help bar.
fn help_shortcuts() -> Vec<(&'static str, &'static str)> {
    vec![
        ("^S", "Save"),
        ("^A", "Save As"),
        ("^Q", "Quit"),
        ("^Z", "Undo"),
        ("^Y", "Redo"),
        ("^C", "Copy"),
        ("^X", "Cut"),
        ("^V", "Paste"),
        ("^F", "Find"),
        ("^G", "Go to"),
        ("^K", "Del ln"),
        ("^D", "Dupl"),
        ("^W", "Wrap"),
        ("^L", "Highl"),
        ("^H", "Help"),
    ]
}

/// Width of a single shortcut entry: key + " label ".
fn shortcut_width(key: &str, label: &str) -> usize {
    // key displayed, then " label " (space-label-space)
    key.len() + 1 + label.len() + 1
}

/// The width of the " HELP " label prefix.
const HELP_LABEL_WIDTH: usize = 6; // " HELP "

/// Calculate how many rows the help bar needs given a terminal width.
pub fn help_row_count(term_width: u16) -> u16 {
    let shortcuts = help_shortcuts();
    let width = term_width as usize;
    if width == 0 { return 1; }

    // First row starts after the " HELP " label.
    let mut rows: u16 = 1;
    let mut x = HELP_LABEL_WIDTH;

    for (key, label) in &shortcuts {
        let sw = shortcut_width(key, label);
        if x + sw > width && x > HELP_LABEL_WIDTH {
            // Doesn't fit — start a new row.
            rows += 1;
            x = sw;
        } else {
            x += sw;
        }
    }
    rows
}

/// Render the pico-style help bar at the bottom of the screen.
/// Builds upward from the row above the status bar when it needs
/// multiple lines.
pub fn render_help_bar<W: Write>(
    _editor: &Editor,
    w: &mut W,
    vp: &Viewport,
) -> io::Result<()> {
    let shortcuts = help_shortcuts();
    let width = vp.width as usize;
    let num_rows = help_row_count(vp.width) as usize;

    // Layout shortcuts into rows.
    // Each row is a Vec of (key, label) pairs.
    let mut rows: Vec<Vec<(&str, &str)>> = vec![Vec::new()];
    let mut x = HELP_LABEL_WIDTH; // first row starts after " HELP "

    for (key, label) in &shortcuts {
        let sw = shortcut_width(key, label);
        if x + sw > width && x > HELP_LABEL_WIDTH && !rows.last().unwrap().is_empty() {
            rows.push(Vec::new());
            x = sw;
        } else {
            x += sw;
        }
        rows.last_mut().unwrap().push((key, label));
    }

    // The bottom-most help row sits directly above the status bar.
    // Status bar is at vp.height - 1, so the bottom help row is at
    // vp.height - 2, and rows build upward from there.
    let base_y = vp.height.saturating_sub(2);

    for (row_idx, row_items) in rows.iter().enumerate() {
        // Rows render top-to-bottom, row 0 is the topmost.
        let y = base_y.saturating_sub((num_rows - 1 - row_idx) as u16);
        w.queue(cursor::MoveTo(0, y))?;

        let mut used: usize = 0;

        if row_idx == 0 {
            // First row: " HELP " label in yellow (like mode labels)
            w.queue(SetBackgroundColor(Color::DarkYellow))?;
            w.queue(SetForegroundColor(Color::Black))?;
            w.queue(style::Print(" HELP "))?;
            used += HELP_LABEL_WIDTH;
        }

        // Render shortcut items
        for (key, label) in row_items {
            w.queue(SetBackgroundColor(Color::DarkBlue))?;
            w.queue(SetForegroundColor(Color::Black))?;
            w.queue(style::Print(key))?;
            w.queue(SetBackgroundColor(Color::White))?;
            w.queue(SetForegroundColor(Color::Black))?;
            let lbl = format!(" {} ", label);
            w.queue(style::Print(&lbl))?;
            used += key.len() + lbl.len();
        }

        // On the last row, right-align the version string
        if row_idx == rows.len() - 1 {
            let version_str = format!(
                "Dan v{} ({}) ",
                crate::VERSION.trim(),
                crate::GIT_HASH,
            );
            let remaining = width.saturating_sub(used + version_str.len());
            if remaining > 0 {
                w.queue(SetBackgroundColor(Color::White))?;
                write_spaces(w, remaining)?;
            }
            w.queue(SetBackgroundColor(Color::White))?;
            w.queue(SetForegroundColor(Color::DarkGrey))?;
            w.queue(style::Print(&version_str))?;
        } else {
            // Pad remaining width with white background
            let remaining = width.saturating_sub(used);
            if remaining > 0 {
                w.queue(SetBackgroundColor(Color::White))?;
                write_spaces(w, remaining)?;
            }
        }
    }

    w.queue(SetForegroundColor(Color::Reset))?;
    w.queue(SetBackgroundColor(Color::Reset))?;

    Ok(())
}

/// Render the search prompt bar (appears below the status bar).
pub fn render_search_bar<W: Write>(
    editor: &Editor,
    w: &mut W,
    vp: &Viewport,
) -> io::Result<()> {
    // Search bar overlays above help (if shown) and above the status bar.
    let help_offset = if editor.show_help { help_row_count(vp.width) } else { 0 };
    let search_y = vp.height.saturating_sub(2 + help_offset);
    w.queue(cursor::MoveTo(0, search_y))?;

    let width = vp.width as usize;
    let mut used: usize = 0;

    // Label
    w.queue(SetBackgroundColor(Color::DarkYellow))?;
    w.queue(SetForegroundColor(Color::Black))?;
    let label = "    → ";
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
            " (0) ".to_string()
        }
    } else {
        format!(
            " ({}/{}) ",
            editor.search_match_idx + 1,
            editor.search_matches.len()
        )
    };
    if !info.is_empty() {
        w.queue(SetForegroundColor(Color::White))?;
        w.queue(style::Print(&info))?;
        used += info.len();
    }

    // Pad the rest
    let remaining = width.saturating_sub(used);
    if remaining > 0 {
        w.queue(SetBackgroundColor(Color::DarkGrey))?;
        write_spaces(w, remaining)?;
    }

    w.queue(SetBackgroundColor(Color::Reset))?;
    w.queue(SetForegroundColor(Color::Reset))?;

    Ok(())
}

/// Render the go-to-line prompt bar (appears below the status bar).
pub fn render_goto_line_bar<W: Write>(
    editor: &Editor,
    w: &mut W,
    vp: &Viewport,
) -> io::Result<()> {
    // GoTo bar overlays above help (if shown) and above the status bar.
    let help_offset = if editor.show_help { help_row_count(vp.width) } else { 0 };
    let bar_y = vp.height.saturating_sub(2 + help_offset);
    w.queue(cursor::MoveTo(0, bar_y))?;

    let width = vp.width as usize;
    let mut used: usize = 0;

    // Label
    w.queue(SetBackgroundColor(Color::DarkCyan))?;
    w.queue(SetForegroundColor(Color::Black))?;
    let label = "    → ";
    w.queue(style::Print(label))?;
    used += label.len();

    // Line number input
    w.queue(SetBackgroundColor(Color::DarkGrey))?;
    w.queue(SetForegroundColor(Color::White))?;
    let input_display = format!(" {} ", editor.goto_line_input);
    w.queue(style::Print(&input_display))?;
    used += input_display.len();

    // Hint
    let total_lines = editor.buffer().line_count();
    let hint = format!(" (1-{}) ", total_lines);
    w.queue(SetForegroundColor(Color::Grey))?;
    w.queue(style::Print(&hint))?;
    used += hint.len();

    // Pad the rest
    let remaining = width.saturating_sub(used);
    if remaining > 0 {
        w.queue(SetBackgroundColor(Color::DarkGrey))?;
        write_spaces(w, remaining)?;
    }

    w.queue(SetBackgroundColor(Color::Reset))?;
    w.queue(SetForegroundColor(Color::Reset))?;

    Ok(())
}

/// Render the save-as prompt bar (appears below the status bar).
pub fn render_save_as_bar<W: Write>(
    editor: &Editor,
    w: &mut W,
    vp: &Viewport,
) -> io::Result<()> {
    let help_offset = if editor.show_help { help_row_count(vp.width) } else { 0 };
    let bar_y = vp.height.saturating_sub(2 + help_offset);
    w.queue(cursor::MoveTo(0, bar_y))?;

    let width = vp.width as usize;
    let mut used: usize = 0;

    // Label
    w.queue(SetBackgroundColor(Color::DarkGreen))?;
    w.queue(SetForegroundColor(Color::Black))?;
    let label = " Save As: ";
    w.queue(style::Print(label))?;
    used += label.len();

    // Path input
    w.queue(SetBackgroundColor(Color::DarkGrey))?;
    w.queue(SetForegroundColor(Color::White))?;
    let input_display = format!(" {} ", editor.save_as_input);
    w.queue(style::Print(&input_display))?;
    used += input_display.len();

    // Hint
    // w.queue(SetForegroundColor(Color::Grey))?;
    // let hint = " (Enter=save, Esc=cancel) ";
    // w.queue(style::Print(hint))?;
    // used += hint.len();

    // Pad the rest
    let remaining = width.saturating_sub(used);
    if remaining > 0 {
        w.queue(SetBackgroundColor(Color::DarkGrey))?;
        write_spaces(w, remaining)?;
    }

    w.queue(SetBackgroundColor(Color::Reset))?;
    w.queue(SetForegroundColor(Color::Reset))?;

    Ok(())
}

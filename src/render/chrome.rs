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

/// Render the pico-style help bar at the bottom of the screen.
pub fn render_help_bar<W: Write>(
    _editor: &Editor,
    w: &mut W,
    vp: &Viewport,
) -> io::Result<()> {
    // Help bar is the first overlay above the status bar.
    let help_y = vp.height.saturating_sub(2);
    w.queue(cursor::MoveTo(0, help_y))?;
    
    // Pico/nano-style shortcut hints
    let shortcuts = [
        ("^S", "Save"),
        ("^⇧S", "SavAs"),
        ("^Q", "Quit"),
        ("^Z", "Undo"),
        ("^Y", "Redo"),
        ("^C", "Copy"),
        ("^X", "Cut"),
        ("^V", "Paste"),
        ("^F", "Find"),
        ("^G", "GoTo"),
        ("^K", "Del Ln"),
        ("^A", "Sel All"),
        ("^W", "Wrap"),
        ("^L", "Highl"),
        ("^H", "Help"),
    ];

    let mut used: usize = 0;
    for (key, label) in &shortcuts {
        // Key in inverse video
        w.queue(SetBackgroundColor(Color::DarkBlue))?;
        w.queue(SetForegroundColor(Color::Black))?;
        w.queue(style::Print(key))?;
        w.queue(SetBackgroundColor(Color::White))?;
        w.queue(SetForegroundColor(Color::Black))?;
        let lbl = format!(" {} ", label);
        w.queue(style::Print(&lbl))?;
        used += key.len() + lbl.len();
    }

    // Right-aligned version string
    let version_str = format!(
        "Dan v{} ({}) ",
        crate::VERSION.trim(),
        crate::GIT_HASH,
    );
    let remaining = (vp.width as usize).saturating_sub(used + version_str.len());

    // Pad between shortcuts and version
    if remaining > 0 {
        w.queue(SetBackgroundColor(Color::White))?;
        write_spaces(w, remaining)?;
    }

    // Version in subdued style
    w.queue(SetBackgroundColor(Color::White))?;
    w.queue(SetForegroundColor(Color::DarkGrey))?;
    w.queue(style::Print(&version_str))?;

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
    let search_y = if editor.show_help {
        vp.height.saturating_sub(3)
    } else {
        vp.height.saturating_sub(2)
    };
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
    let bar_y = if editor.show_help {
        vp.height.saturating_sub(3)
    } else {
        vp.height.saturating_sub(2)
    };
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
    let bar_y = if editor.show_help {
        vp.height.saturating_sub(3)
    } else {
        vp.height.saturating_sub(2)
    };
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
    w.queue(SetForegroundColor(Color::Grey))?;
    let hint = " (Enter=save, Esc=cancel) ";
    w.queue(style::Print(hint))?;
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

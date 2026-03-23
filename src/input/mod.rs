use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::editor::commands::Command;

/// Map a crossterm event to an editor command.
/// Pico-style: no modes — all keys work the same way at all times.
pub fn map_event(event: &Event) -> Command {
    match event {
        Event::Key(key) => map_key(key),
        Event::Paste(text) => Command::InsertString(text.clone()),
        _ => Command::Noop,
    }
}

fn map_key(key: &KeyEvent) -> Command {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    // -- Ctrl+Shift shortcuts --
    if ctrl && shift {
        return match key.code {
            KeyCode::Char('c') | KeyCode::Char('C') => Command::Copy,
            _ => Command::Noop,
        };
    }

    // -- Ctrl shortcuts (GUI-style) --
    if ctrl {
        return match key.code {
            KeyCode::Char('c') => Command::ForceQuit,
            KeyCode::Char('s') => Command::Save,
            KeyCode::Char('q') => Command::Quit,
            KeyCode::Char('z') => Command::Undo,
            KeyCode::Char('y') => Command::Redo,
            KeyCode::Char('x') => Command::Cut,
            KeyCode::Char('v') => Command::Paste,
            KeyCode::Char('a') => Command::SelectAll,
            KeyCode::Char('f') => Command::SearchForward,
            KeyCode::Char('g') => Command::SearchNext,
            KeyCode::Left      => Command::MoveWordBackward,
            KeyCode::Right     => Command::MoveWordForward,
            KeyCode::Home      => Command::MoveBufferTop,
            KeyCode::End       => Command::MoveBufferBottom,
            KeyCode::Char('k') => Command::DeleteLine,
            KeyCode::Char('d') => Command::DuplicateLineOrSelection,
            KeyCode::Char('h') => Command::ToggleHelp,
            _ => Command::Noop,
        };
    }

    // -- Shift+arrow = select --
    if shift {
        return match key.code {
            KeyCode::Left  => Command::SelectLeft,
            KeyCode::Right => Command::SelectRight,
            KeyCode::Up    => Command::SelectUp,
            KeyCode::Down  => Command::SelectDown,
            KeyCode::Home  => Command::SelectLineStart,
            KeyCode::End   => Command::SelectLineEnd,
            KeyCode::BackTab => Command::Dedent,
            // Shift+char — insert uppercase / shifted character
            KeyCode::Char(ch) => Command::InsertChar(ch),
            _ => Command::Noop,
        };
    }

    // -- Regular keys --
    match key.code {
        // Navigation
        KeyCode::Left     => Command::MoveLeft,
        KeyCode::Right    => Command::MoveRight,
        KeyCode::Up       => Command::MoveUp,
        KeyCode::Down     => Command::MoveDown,
        KeyCode::Home     => Command::MoveLineStart,
        KeyCode::End      => Command::MoveLineEnd,
        KeyCode::PageUp   => Command::PageUp,
        KeyCode::PageDown => Command::PageDown,

        // Editing — direct insert, no mode switch needed
        KeyCode::Enter     => Command::InsertNewline,
        KeyCode::Tab       => Command::InsertTab,
        KeyCode::BackTab   => Command::Dedent,
        KeyCode::Backspace => Command::DeleteBackward,
        KeyCode::Delete    => Command::DeleteForward,
        KeyCode::Char(ch)  => Command::InsertChar(ch),

        // Escape cancels selection
        KeyCode::Esc => Command::Noop,

        _ => Command::Noop,
    }
}

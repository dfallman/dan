use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::editor::commands::Command;
use crate::editor::mode::Mode;

/// Map a crossterm event to an editor command.
/// The current `mode` changes how keys are interpreted.
pub fn map_event(event: &Event, mode: Mode) -> Command {
    match event {
        Event::Key(key) => {
            if mode == Mode::ConfirmQuit {
                return map_confirm_quit_key(key);
            }
            if mode == Mode::Searching {
                return map_search_key(key);
            }
            if mode == Mode::GoToLine {
                return map_goto_line_key(key);
            }
            map_key(key)
        }
        Event::Paste(text) => Command::InsertString(text.clone()),
        _ => Command::Noop,
    }
}

/// Key mapping while in the quit-confirmation prompt.
fn map_confirm_quit_key(key: &KeyEvent) -> Command {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        // ^S = save and quit
        KeyCode::Char('s') if ctrl => Command::SaveAndQuit,
        // ^Y = yes, quit without saving
        KeyCode::Char('y') if ctrl => Command::ForceQuit,
        // Anything else (Esc, any key) cancels
        _ => Command::CancelQuit,
    }
}

/// Key mapping while inside the interactive search prompt.
fn map_search_key(key: &KeyEvent) -> Command {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    match key.code {
        // Esc cancels search and restores cursor
        KeyCode::Esc => Command::SearchCancel,
        // Shift+Enter = prev match (cycle backwards)
        KeyCode::Enter if shift => Command::SearchPrev,
        // Enter confirms search — exits search, selects matched text
        KeyCode::Enter => Command::SearchConfirm,
        // Ctrl+G = next match, Ctrl+Shift+G = prev match
        KeyCode::Char('g') if ctrl && shift => Command::SearchPrev,
        KeyCode::Char('g') | KeyCode::Char('G') if ctrl => Command::SearchNext,
        // Backspace deletes from query
        KeyCode::Backspace => Command::SearchDeleteChar,
        // Printable chars (including shifted) are appended to the query
        KeyCode::Char(ch) if !ctrl => Command::SearchInsertChar(ch),
        _ => Command::Noop,
    }
}

fn map_key(key: &KeyEvent) -> Command {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    // -- Ctrl+Shift shortcuts --
    if ctrl && shift {
        return match key.code {
            KeyCode::Char('c') | KeyCode::Char('C') => Command::ForceQuit,
            KeyCode::Left  => Command::SelectWordBackward,
            KeyCode::Right => Command::SelectWordForward,
            _ => Command::Noop,
        };
    }

    // -- Alt+Shift shortcuts (selection by word / line) --
    if alt && shift {
        return match key.code {
            KeyCode::Left  => Command::SelectWordBackward,
            KeyCode::Right => Command::SelectWordForward,
            KeyCode::Up    => Command::SelectUp,
            KeyCode::Down  => Command::SelectDown,
            _ => Command::Noop,
        };
    }

    // -- Ctrl shortcuts (GUI-style) --
    if ctrl {
        return match key.code {
            KeyCode::Char('c') => Command::Copy,
            KeyCode::Char('s') => Command::Save,
            KeyCode::Char('q') => Command::Quit,
            KeyCode::Char('z') => Command::Undo,
            KeyCode::Char('y') => Command::Redo,
            KeyCode::Char('x') => Command::Cut,
            KeyCode::Char('v') => Command::Paste,
            KeyCode::Char('a') => Command::SelectAll,
            KeyCode::Char('f') | KeyCode::Char('/') => Command::SearchForward,
            KeyCode::Char('g') => Command::GoToLineOpen,
            KeyCode::Left      => Command::MoveWordBackward,
            KeyCode::Right     => Command::MoveWordForward,
            KeyCode::Home      => Command::MoveBufferTop,
            KeyCode::End       => Command::MoveBufferBottom,
            KeyCode::Char('k') => Command::DeleteLine,
            KeyCode::Char('d') => Command::DuplicateLineOrSelection,
            KeyCode::Char('w') => Command::ToggleWrap,
            KeyCode::Char('h') => Command::ToggleHelp,
            KeyCode::Char('l') => Command::ToggleSyntax,
            _ => Command::Noop,
        };
    }

    // -- Alt/Option shortcuts (word jump + line swap) --
    if alt {
        return match key.code {
            KeyCode::Left  => Command::MoveWordBackward,
            KeyCode::Right => Command::MoveWordForward,
            KeyCode::Up    => Command::SwapLineUp,
            KeyCode::Down  => Command::SwapLineDown,
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

        // F7 = open search (works even when Ctrl+F is intercepted by the terminal)
        KeyCode::F(7) => Command::SearchForward,

        // Escape cancels selection
        KeyCode::Esc => Command::Noop,

        _ => Command::Noop,
    }
}

/// Key mapping while inside the go-to-line prompt.
fn map_goto_line_key(key: &KeyEvent) -> Command {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Esc => Command::GoToLineCancel,
        KeyCode::Enter => Command::GoToLineConfirm,
        KeyCode::Backspace => Command::GoToLineDeleteChar,
        KeyCode::Char(ch) if !ctrl => Command::GoToLineInsertChar(ch),
        _ => Command::Noop,
    }
}

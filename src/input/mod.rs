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
			if mode == Mode::SaveAs {
				return map_save_as_key(key);
			}
			if mode == Mode::ConfirmOverwrite {
				return map_confirm_overwrite_key(key);
			}
			match mode {
				Mode::ReplacingSearch => map_replace_search_key(key),
				Mode::ReplacingWith => map_replace_with_key(key),
				Mode::ReplacingStep => map_replace_step_key(key),
				Mode::RecoverSwap => map_recover_swap_key(key),
				_ => map_key(key),
			}
		}
		Event::Paste(text) => Command::InsertString(text.clone()),
		_ => Command::Noop,
	}
}

/// Key mapping while in the quit-confirmation prompt.
fn map_confirm_quit_key(key: &KeyEvent) -> Command {
	let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
	match key.code {
		KeyCode::Char('s') | KeyCode::Char('S') => Command::SaveAndQuit,
		KeyCode::Char('f') | KeyCode::Char('F') => Command::ForceQuit,
		KeyCode::Char('q') | KeyCode::Char('Q') if ctrl => Command::CancelQuit,
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
		// Ctrl+R = elevate search matches directly into global Replace loop
		KeyCode::Char('r') | KeyCode::Char('R') if ctrl => Command::SearchConvertToReplace,
		// Backspace deletes from query
		KeyCode::Backspace => Command::SearchDeleteChar,
		// Printable chars (including shifted) are appended to the query
		KeyCode::Char(ch) if !ctrl => Command::SearchInsertChar(ch),
		_ => Command::Noop,
	}
}

/// Key mapping while inside the Replace: target prompt.
fn map_replace_search_key(key: &KeyEvent) -> Command {
	let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

	match key.code {
		KeyCode::Esc => Command::ReplaceCancel,
		KeyCode::Enter => Command::ReplaceSearchConfirm,
		KeyCode::Backspace => Command::ReplaceDeleteChar,
		KeyCode::Char(ch) if !ctrl => Command::ReplaceInsertChar(ch),
		_ => Command::Noop,
	}
}

/// Key mapping while inside the Replace With: prompt.
fn map_replace_with_key(key: &KeyEvent) -> Command {
	let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

	match key.code {
		KeyCode::Esc => Command::ReplaceCancel,
		KeyCode::Enter => Command::ReplaceWithConfirm,
		KeyCode::Backspace => Command::ReplaceDeleteChar,
		KeyCode::Char(ch) if !ctrl => Command::ReplaceInsertChar(ch),
		_ => Command::Noop,
	}
}

/// Key mapping while interacting through Match Replacement Steps.
fn map_replace_step_key(key: &KeyEvent) -> Command {
	match key.code {
		KeyCode::Char('y') | KeyCode::Char('Y') => Command::ReplaceActionYes,
		KeyCode::Char('n') | KeyCode::Char('N') => Command::ReplaceActionNo,
		KeyCode::Char('a') | KeyCode::Char('A') => Command::ReplaceActionAll,
		KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => Command::ReplaceCancel,
		_ => Command::Noop,
	}
}

/// Key mapping for crash recovery prompt selections
fn map_recover_swap_key(key: &KeyEvent) -> Command {
	match key.code {
		KeyCode::Char('y') | KeyCode::Char('Y') => Command::RecoverSwapAccept,
		KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Command::RecoverSwapDecline,
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
			KeyCode::Char('\\') | KeyCode::Char('|') => Command::SelectAll,
			KeyCode::Left => Command::SelectWordBackward,
			KeyCode::Right => Command::SelectWordForward,
			KeyCode::Up => Command::MoveFastUp,
			KeyCode::Down => Command::MoveFastDown,
			KeyCode::Char('/')
			| KeyCode::Char('_')
			| KeyCode::Char('?')
			| KeyCode::Char('-')
			| KeyCode::Char('e') => Command::ToggleComment,
			_ => Command::Noop,
		};
	}

	// -- Alt+Shift shortcuts (selection by word / line) --
	if alt && shift {
		return match key.code {
			KeyCode::Left => Command::SelectWordBackward,
			KeyCode::Right => Command::SelectWordForward,
			KeyCode::Up => Command::SelectUp,
			KeyCode::Down => Command::SelectDown,
			_ => Command::Noop,
		};
	}

	// -- Ctrl shortcuts (GUI-style) --
	if ctrl {
		return match key.code {
			KeyCode::Char('c') => Command::Copy,
			KeyCode::Char('s') => Command::Save,
			KeyCode::Char('\\') => Command::SelectAll,
			KeyCode::Char('q') => Command::Quit,
			KeyCode::Char('z') => Command::Undo,
			KeyCode::Char('y') => Command::Redo,
			KeyCode::Char('x') => Command::Cut,
			KeyCode::Char('v') => Command::Paste,
			KeyCode::Char('a') => Command::SaveAsOpen,
			KeyCode::Char('f') => Command::SearchForward,
			KeyCode::Char('/')
			| KeyCode::Char('_')
			| KeyCode::Char('?')
			| KeyCode::Char('-')
			| KeyCode::Char('e') => Command::ToggleComment,
			KeyCode::Char('g') => Command::GoToLineOpen,
			KeyCode::Left => Command::MoveWordBackward,
			KeyCode::Right => Command::MoveWordForward,
			KeyCode::Up => Command::ScrollViewportUp,
			KeyCode::Down => Command::ScrollViewportDown,
			KeyCode::Home => Command::MoveBufferTop,
			KeyCode::End => Command::MoveBufferBottom,
			KeyCode::Char('k') => Command::DeleteLine,
			KeyCode::Char('d') => Command::DuplicateLineOrSelection,
			KeyCode::Char('w') => Command::ToggleWrap,
			KeyCode::Char('h') => Command::ToggleHelp,
			KeyCode::Char('l') => Command::FormatDocument,
			KeyCode::Char('t') => Command::ToggleSyntax,
			_ => Command::Noop,
		};
	}

	// -- Alt/Option shortcuts (word jump + line swap) --
	if alt {
		return match key.code {
			KeyCode::Left => Command::MoveWordBackward,
			KeyCode::Right => Command::MoveWordForward,
			KeyCode::Up => Command::SwapLineUp,
			KeyCode::Down => Command::SwapLineDown,
			_ => Command::Noop,
		};
	}

	// -- Shift+arrow = select --
	if shift {
		return match key.code {
			KeyCode::Left => Command::SelectLeft,
			KeyCode::Right => Command::SelectRight,
			KeyCode::Up => Command::SelectUp,
			KeyCode::Down => Command::SelectDown,
			KeyCode::Home => Command::SelectLineStart,
			KeyCode::End => Command::SelectLineEnd,
			KeyCode::BackTab => Command::Dedent,
			// Shift+char — insert uppercase / shifted character
			KeyCode::Char(ch) => Command::InsertChar(ch),
			_ => Command::Noop,
		};
	}

	// -- Regular keys --
	match key.code {
		// Navigation
		KeyCode::Left => Command::MoveLeft,
		KeyCode::Right => Command::MoveRight,
		KeyCode::Up => Command::MoveUp,
		KeyCode::Down => Command::MoveDown,
		KeyCode::Home => Command::MoveLineStart,
		KeyCode::End => Command::MoveLineEnd,
		KeyCode::PageUp => Command::PageUp,
		KeyCode::PageDown => Command::PageDown,

		// Editing — direct insert, no mode switch needed
		KeyCode::Enter => Command::InsertNewline,
		KeyCode::Tab => Command::InsertTab,
		KeyCode::BackTab => Command::Dedent,
		KeyCode::Backspace => Command::DeleteBackward,
		KeyCode::Delete => Command::DeleteForward,
		KeyCode::Char(ch) => Command::InsertChar(ch),

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

/// Key mapping while inside the save-as prompt.
fn map_save_as_key(key: &KeyEvent) -> Command {
	let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

	match key.code {
		KeyCode::Esc => Command::SaveAsCancel,
		KeyCode::Enter => Command::SaveAsConfirm,
		KeyCode::Backspace => Command::SaveAsDeleteChar,
		KeyCode::Left => Command::SaveAsCursorLeft,
		KeyCode::Right => Command::SaveAsCursorRight,
		KeyCode::Char(ch) if !ctrl => Command::SaveAsInsertChar(ch),
		_ => Command::Noop,
	}
}

/// Key mapping while in the overwrite-confirmation prompt.
fn map_confirm_overwrite_key(key: &KeyEvent) -> Command {
	let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
	match key.code {
		// ^O = confirm overwrite
		KeyCode::Char('o') if ctrl => Command::ConfirmOverwrite,
		// Anything else cancels back to Save As
		_ => Command::CancelOverwrite,
	}
}

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent};

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
			if mode == Mode::Palette {
				return map_palette_key(key);
			}
			match mode {

				Mode::ReplacingWith => map_replace_with_key(key),
				Mode::ReplacingStep => map_replace_step_key(key),
				Mode::RecoverSwap => map_recover_swap_key(key),
				_ => map_key(key),
			}
		}
		Event::Paste(text) => {
			// Confirm / step dialogs have no text field — ignore paste so it
			// cannot land in the document behind the prompt.
			if matches!(
				mode,
				Mode::ConfirmQuit
					| Mode::ConfirmOverwrite
					| Mode::ReplacingStep
					| Mode::RecoverSwap
			) {
				Command::Noop
			} else {
				Command::InsertString(text.clone())
			}
		}
		Event::Mouse(me) => map_mouse(me, mode),
		_ => Command::Noop,
	}
}

fn map_mouse(me: &MouseEvent, mode: Mode) -> Command {
	if mode != Mode::Editing {
		return Command::Noop;
	}
	use crossterm::event::{MouseButton, MouseEventKind};
	match me.kind {
		MouseEventKind::Down(MouseButton::Left) => Command::MouseDown {
			col: me.column,
			row: me.row,
			extend: me.modifiers.contains(KeyModifiers::SHIFT),
		},
		MouseEventKind::Drag(MouseButton::Left) => Command::MouseDrag {
			col: me.column,
			row: me.row,
		},
		MouseEventKind::Up(MouseButton::Left) => Command::MouseUp {
			col: me.column,
			row: me.row,
		},
		MouseEventKind::ScrollUp => Command::ScrollViewportUp,
		MouseEventKind::ScrollDown => Command::ScrollViewportDown,
		_ => Command::Noop,
	}
}

/// Key mapping while in the quit-confirmation prompt.
fn map_confirm_quit_key(key: &KeyEvent) -> Command {
	let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
	match key.code {
		KeyCode::Char('s') | KeyCode::Char('S') if ctrl => Command::SaveAndQuit,
		KeyCode::Char('f') | KeyCode::Char('F') if ctrl => Command::ForceQuit,
		KeyCode::Char('q') | KeyCode::Char('Q') if ctrl => Command::CancelQuit,
		KeyCode::Esc => Command::CancelQuit,
		_ => Command::Noop,
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
		// Ctrl+G = next match; Ctrl+T / Ctrl+Shift+G = prev match
		KeyCode::Char('g') if ctrl && shift => Command::SearchPrev,
		KeyCode::Char('g') | KeyCode::Char('G') if ctrl => Command::SearchNext,
		KeyCode::Char('t') | KeyCode::Char('T') if ctrl => Command::SearchPrev,
		// Ctrl+R = elevate search matches directly into global Replace loop
		KeyCode::Char('r') | KeyCode::Char('R') if ctrl => Command::SearchConvertToReplace,
		// Ctrl+V = paste into the search query (not the document)
		KeyCode::Char('v') | KeyCode::Char('V') if ctrl => Command::Paste,
		// Backspace deletes from query
		KeyCode::Backspace => Command::SearchDeleteChar,
		KeyCode::Left => Command::PromptCursorLeft,
		KeyCode::Right => Command::PromptCursorRight,
		// Printable chars (including shifted) are appended to the query
		KeyCode::Char(ch) if !ctrl => Command::SearchInsertChar(ch),
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
		KeyCode::Left => Command::PromptCursorLeft,
		KeyCode::Right => Command::PromptCursorRight,
		KeyCode::Char('v') | KeyCode::Char('V') if ctrl => Command::Paste,
		KeyCode::Char(ch) if !ctrl => Command::ReplaceInsertChar(ch),
		_ => Command::Noop,
	}
}

/// Key mapping while interacting through Match Replacement Steps.
fn map_replace_step_key(key: &KeyEvent) -> Command {
	let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
	match key.code {
		KeyCode::Char('y') | KeyCode::Char('Y') if ctrl => Command::ReplaceActionYes,
		KeyCode::Char('n') | KeyCode::Char('N') if ctrl => Command::ReplaceActionNo,
		KeyCode::Char('a') | KeyCode::Char('A') if ctrl => Command::ReplaceActionAll,
		KeyCode::Esc => Command::ReplaceCancel,
		_ => Command::Noop,
	}
}

/// Key mapping for crash recovery prompt selections
fn map_recover_swap_key(key: &KeyEvent) -> Command {
	let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
	match key.code {
		KeyCode::Char('y') | KeyCode::Char('Y') if ctrl => Command::RecoverSwapAccept,
		KeyCode::Char('n') | KeyCode::Char('N') if ctrl => Command::RecoverSwapDecline,
		KeyCode::Char('q') | KeyCode::Char('Q') if ctrl => Command::ForceQuitAll,
		KeyCode::Esc => Command::ForceQuitAll,
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
			KeyCode::Char('c') | KeyCode::Char('C') => Command::ForceQuitAll,
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
			KeyCode::Char('r') => Command::ToggleWhitespace,
			KeyCode::Char('w') => Command::ToggleWrap,
			KeyCode::Char('h') => Command::ToggleHelp,
			KeyCode::Char('l') => Command::FormatDocument,
			KeyCode::Char('t') => Command::ToggleSyntax,
			KeyCode::Char('p') => Command::PaletteOpen,
			KeyCode::Char('n') => Command::NewBuffer,
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
		KeyCode::Left => Command::PromptCursorLeft,
		KeyCode::Right => Command::PromptCursorRight,
		KeyCode::Char('v') | KeyCode::Char('V') if ctrl => Command::Paste,
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
		KeyCode::Left => Command::PromptCursorLeft,
		KeyCode::Right => Command::PromptCursorRight,
		KeyCode::Char('v') | KeyCode::Char('V') if ctrl => Command::Paste,
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

/// Key mapping while the command palette is open.
fn map_palette_key(key: &KeyEvent) -> Command {
	let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
	match key.code {
		KeyCode::Esc => Command::PaletteCancel,
		KeyCode::Enter => Command::PaletteConfirm,
		KeyCode::Up => Command::PaletteUp,
		KeyCode::Down => Command::PaletteDown,
		KeyCode::PageUp => Command::PalettePageUp,
		KeyCode::PageDown => Command::PalettePageDown,
		KeyCode::Backspace => Command::PaletteDeleteChar,
		KeyCode::Char('s') | KeyCode::Char('S') if ctrl => Command::PaletteClosePromptSave,
		KeyCode::Char('d') | KeyCode::Char('D') if ctrl => Command::PaletteCloseBuffer,
		KeyCode::Char('v') | KeyCode::Char('V') if ctrl => Command::Paste,
		KeyCode::Char(ch) if !ctrl => Command::PaletteInsertChar(ch),
		_ => Command::Noop,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

	fn key(code: KeyCode, mods: KeyModifiers) -> Event {
		Event::Key(KeyEvent::new(code, mods))
	}

	fn assert_map(cases: &[(&str, Event, Mode, Command)]) {
		for (label, event, mode, expected) in cases {
			let got = map_event(event, *mode);
			assert_eq!(
				got, *expected,
				"case `{label}`: mode={mode:?} event={event:?}\n  expected: {expected:?}\n  got:      {got:?}"
			);
		}
	}

	#[test]
	fn editing_mode_core_bindings() {
		use KeyModifiers as M;
		assert_map(&[
			// Motion
			("left", key(KeyCode::Left, M::NONE), Mode::Editing, Command::MoveLeft),
			("right", key(KeyCode::Right, M::NONE), Mode::Editing, Command::MoveRight),
			("up", key(KeyCode::Up, M::NONE), Mode::Editing, Command::MoveUp),
			("down", key(KeyCode::Down, M::NONE), Mode::Editing, Command::MoveDown),
			("home", key(KeyCode::Home, M::NONE), Mode::Editing, Command::MoveLineStart),
			("end", key(KeyCode::End, M::NONE), Mode::Editing, Command::MoveLineEnd),
			("pageup", key(KeyCode::PageUp, M::NONE), Mode::Editing, Command::PageUp),
			("pagedown", key(KeyCode::PageDown, M::NONE), Mode::Editing, Command::PageDown),
			// Editing
			("enter", key(KeyCode::Enter, M::NONE), Mode::Editing, Command::InsertNewline),
			("tab", key(KeyCode::Tab, M::NONE), Mode::Editing, Command::InsertTab),
			("backtab", key(KeyCode::BackTab, M::NONE), Mode::Editing, Command::Dedent),
			("backspace", key(KeyCode::Backspace, M::NONE), Mode::Editing, Command::DeleteBackward),
			("delete", key(KeyCode::Delete, M::NONE), Mode::Editing, Command::DeleteForward),
			("char a", key(KeyCode::Char('a'), M::NONE), Mode::Editing, Command::InsertChar('a')),
			("char é", key(KeyCode::Char('é'), M::NONE), Mode::Editing, Command::InsertChar('é')),
			("f7 search", key(KeyCode::F(7), M::NONE), Mode::Editing, Command::SearchForward),
			("esc noop", key(KeyCode::Esc, M::NONE), Mode::Editing, Command::Noop),
			// Shift+arrow selection
			("shift+left", key(KeyCode::Left, M::SHIFT), Mode::Editing, Command::SelectLeft),
			("shift+right", key(KeyCode::Right, M::SHIFT), Mode::Editing, Command::SelectRight),
			("shift+up", key(KeyCode::Up, M::SHIFT), Mode::Editing, Command::SelectUp),
			("shift+down", key(KeyCode::Down, M::SHIFT), Mode::Editing, Command::SelectDown),
			("shift+home", key(KeyCode::Home, M::SHIFT), Mode::Editing, Command::SelectLineStart),
			("shift+end", key(KeyCode::End, M::SHIFT), Mode::Editing, Command::SelectLineEnd),
			("shift+backtab", key(KeyCode::BackTab, M::SHIFT), Mode::Editing, Command::Dedent),
			("shift+A", key(KeyCode::Char('A'), M::SHIFT), Mode::Editing, Command::InsertChar('A')),
			// Ctrl chords
			("ctrl+c", key(KeyCode::Char('c'), M::CONTROL), Mode::Editing, Command::Copy),
			("ctrl+s", key(KeyCode::Char('s'), M::CONTROL), Mode::Editing, Command::Save),
			("ctrl+\\", key(KeyCode::Char('\\'), M::CONTROL), Mode::Editing, Command::SelectAll),
			("ctrl+q", key(KeyCode::Char('q'), M::CONTROL), Mode::Editing, Command::Quit),
			("ctrl+z", key(KeyCode::Char('z'), M::CONTROL), Mode::Editing, Command::Undo),
			("ctrl+y", key(KeyCode::Char('y'), M::CONTROL), Mode::Editing, Command::Redo),
			("ctrl+x", key(KeyCode::Char('x'), M::CONTROL), Mode::Editing, Command::Cut),
			("ctrl+v", key(KeyCode::Char('v'), M::CONTROL), Mode::Editing, Command::Paste),
			("ctrl+a", key(KeyCode::Char('a'), M::CONTROL), Mode::Editing, Command::SaveAsOpen),
			("ctrl+f", key(KeyCode::Char('f'), M::CONTROL), Mode::Editing, Command::SearchForward),
			("ctrl+e comment", key(KeyCode::Char('e'), M::CONTROL), Mode::Editing, Command::ToggleComment),
			("ctrl+/ comment", key(KeyCode::Char('/'), M::CONTROL), Mode::Editing, Command::ToggleComment),
			("ctrl+g", key(KeyCode::Char('g'), M::CONTROL), Mode::Editing, Command::GoToLineOpen),
			("ctrl+left", key(KeyCode::Left, M::CONTROL), Mode::Editing, Command::MoveWordBackward),
			("ctrl+right", key(KeyCode::Right, M::CONTROL), Mode::Editing, Command::MoveWordForward),
			("ctrl+up", key(KeyCode::Up, M::CONTROL), Mode::Editing, Command::ScrollViewportUp),
			("ctrl+down", key(KeyCode::Down, M::CONTROL), Mode::Editing, Command::ScrollViewportDown),
			("ctrl+home", key(KeyCode::Home, M::CONTROL), Mode::Editing, Command::MoveBufferTop),
			("ctrl+end", key(KeyCode::End, M::CONTROL), Mode::Editing, Command::MoveBufferBottom),
			("ctrl+k", key(KeyCode::Char('k'), M::CONTROL), Mode::Editing, Command::DeleteLine),
			("ctrl+d", key(KeyCode::Char('d'), M::CONTROL), Mode::Editing, Command::DuplicateLineOrSelection),
			("ctrl+r whitespace", key(KeyCode::Char('r'), M::CONTROL), Mode::Editing, Command::ToggleWhitespace),
			("ctrl+w", key(KeyCode::Char('w'), M::CONTROL), Mode::Editing, Command::ToggleWrap),
			("ctrl+h", key(KeyCode::Char('h'), M::CONTROL), Mode::Editing, Command::ToggleHelp),
			("ctrl+l", key(KeyCode::Char('l'), M::CONTROL), Mode::Editing, Command::FormatDocument),
			("ctrl+t", key(KeyCode::Char('t'), M::CONTROL), Mode::Editing, Command::ToggleSyntax),
			("ctrl+p", key(KeyCode::Char('p'), M::CONTROL), Mode::Editing, Command::PaletteOpen),
			("ctrl+n", key(KeyCode::Char('n'), M::CONTROL), Mode::Editing, Command::NewBuffer),
			// Ctrl+Shift
			("ctrl+shift+c forcequit", key(KeyCode::Char('c'), M::CONTROL | M::SHIFT), Mode::Editing, Command::ForceQuitAll),
			("ctrl+shift+\\ selectall", key(KeyCode::Char('\\'), M::CONTROL | M::SHIFT), Mode::Editing, Command::SelectAll),
			("ctrl+shift+| selectall", key(KeyCode::Char('|'), M::CONTROL | M::SHIFT), Mode::Editing, Command::SelectAll),
			("ctrl+shift+left", key(KeyCode::Left, M::CONTROL | M::SHIFT), Mode::Editing, Command::SelectWordBackward),
			("ctrl+shift+right", key(KeyCode::Right, M::CONTROL | M::SHIFT), Mode::Editing, Command::SelectWordForward),
			("ctrl+shift+up", key(KeyCode::Up, M::CONTROL | M::SHIFT), Mode::Editing, Command::MoveFastUp),
			("ctrl+shift+down", key(KeyCode::Down, M::CONTROL | M::SHIFT), Mode::Editing, Command::MoveFastDown),
			("ctrl+shift+e comment", key(KeyCode::Char('e'), M::CONTROL | M::SHIFT), Mode::Editing, Command::ToggleComment),
			// Alt
			("alt+left", key(KeyCode::Left, M::ALT), Mode::Editing, Command::MoveWordBackward),
			("alt+right", key(KeyCode::Right, M::ALT), Mode::Editing, Command::MoveWordForward),
			("alt+up", key(KeyCode::Up, M::ALT), Mode::Editing, Command::SwapLineUp),
			("alt+down", key(KeyCode::Down, M::ALT), Mode::Editing, Command::SwapLineDown),
			// Alt+Shift
			("alt+shift+left", key(KeyCode::Left, M::ALT | M::SHIFT), Mode::Editing, Command::SelectWordBackward),
			("alt+shift+right", key(KeyCode::Right, M::ALT | M::SHIFT), Mode::Editing, Command::SelectWordForward),
			("alt+shift+up", key(KeyCode::Up, M::ALT | M::SHIFT), Mode::Editing, Command::SelectUp),
			("alt+shift+down", key(KeyCode::Down, M::ALT | M::SHIFT), Mode::Editing, Command::SelectDown),
		]);
	}

	#[test]
	fn search_mode_bindings() {
		use KeyModifiers as M;
		assert_map(&[
			("esc", key(KeyCode::Esc, M::NONE), Mode::Searching, Command::SearchCancel),
			("enter", key(KeyCode::Enter, M::NONE), Mode::Searching, Command::SearchConfirm),
			("shift+enter", key(KeyCode::Enter, M::SHIFT), Mode::Searching, Command::SearchPrev),
			("ctrl+g next", key(KeyCode::Char('g'), M::CONTROL), Mode::Searching, Command::SearchNext),
			("ctrl+G next", key(KeyCode::Char('G'), M::CONTROL), Mode::Searching, Command::SearchNext),
			("ctrl+t prev", key(KeyCode::Char('t'), M::CONTROL), Mode::Searching, Command::SearchPrev),
			("ctrl+T prev", key(KeyCode::Char('T'), M::CONTROL), Mode::Searching, Command::SearchPrev),
			("ctrl+shift+g prev", key(KeyCode::Char('g'), M::CONTROL | M::SHIFT), Mode::Searching, Command::SearchPrev),
			("ctrl+r replace", key(KeyCode::Char('r'), M::CONTROL), Mode::Searching, Command::SearchConvertToReplace),
			("ctrl+R replace", key(KeyCode::Char('R'), M::CONTROL), Mode::Searching, Command::SearchConvertToReplace),
			("backspace", key(KeyCode::Backspace, M::NONE), Mode::Searching, Command::SearchDeleteChar),
			("left", key(KeyCode::Left, M::NONE), Mode::Searching, Command::PromptCursorLeft),
			("right", key(KeyCode::Right, M::NONE), Mode::Searching, Command::PromptCursorRight),
			("char x", key(KeyCode::Char('x'), M::NONE), Mode::Searching, Command::SearchInsertChar('x')),
			("shift+X", key(KeyCode::Char('X'), M::SHIFT), Mode::Searching, Command::SearchInsertChar('X')),
			("ctrl+v paste", key(KeyCode::Char('v'), M::CONTROL), Mode::Searching, Command::Paste),
			("ctrl+s noop", key(KeyCode::Char('s'), M::CONTROL), Mode::Searching, Command::Noop),
		]);
	}

	#[test]
	fn prompt_modes_bindings() {
		use KeyModifiers as M;
		assert_map(&[
			// GoToLine
			("goto esc", key(KeyCode::Esc, M::NONE), Mode::GoToLine, Command::GoToLineCancel),
			("goto enter", key(KeyCode::Enter, M::NONE), Mode::GoToLine, Command::GoToLineConfirm),
			("goto bs", key(KeyCode::Backspace, M::NONE), Mode::GoToLine, Command::GoToLineDeleteChar),
			("goto left", key(KeyCode::Left, M::NONE), Mode::GoToLine, Command::PromptCursorLeft),
			("goto right", key(KeyCode::Right, M::NONE), Mode::GoToLine, Command::PromptCursorRight),
			("goto 4", key(KeyCode::Char('4'), M::NONE), Mode::GoToLine, Command::GoToLineInsertChar('4')),
			("goto ctrl+v paste", key(KeyCode::Char('v'), M::CONTROL), Mode::GoToLine, Command::Paste),
			("goto ctrl+a noop", key(KeyCode::Char('a'), M::CONTROL), Mode::GoToLine, Command::Noop),
			// SaveAs
			("saveas esc", key(KeyCode::Esc, M::NONE), Mode::SaveAs, Command::SaveAsCancel),
			("saveas enter", key(KeyCode::Enter, M::NONE), Mode::SaveAs, Command::SaveAsConfirm),
			("saveas bs", key(KeyCode::Backspace, M::NONE), Mode::SaveAs, Command::SaveAsDeleteChar),
			("saveas left", key(KeyCode::Left, M::NONE), Mode::SaveAs, Command::PromptCursorLeft),
			("saveas right", key(KeyCode::Right, M::NONE), Mode::SaveAs, Command::PromptCursorRight),
			("saveas /", key(KeyCode::Char('/'), M::NONE), Mode::SaveAs, Command::SaveAsInsertChar('/')),
			("saveas ctrl+v paste", key(KeyCode::Char('v'), M::CONTROL), Mode::SaveAs, Command::Paste),
			// ConfirmOverwrite
			("overwrite ctrl+o", key(KeyCode::Char('o'), M::CONTROL), Mode::ConfirmOverwrite, Command::ConfirmOverwrite),
			("overwrite esc cancel", key(KeyCode::Esc, M::NONE), Mode::ConfirmOverwrite, Command::CancelOverwrite),
			("overwrite a cancel", key(KeyCode::Char('a'), M::NONE), Mode::ConfirmOverwrite, Command::CancelOverwrite),
			// ConfirmQuit
			("quit ctrl+s", key(KeyCode::Char('s'), M::CONTROL), Mode::ConfirmQuit, Command::SaveAndQuit),
			("quit ctrl+S", key(KeyCode::Char('S'), M::CONTROL), Mode::ConfirmQuit, Command::SaveAndQuit),
			("quit ctrl+f", key(KeyCode::Char('f'), M::CONTROL), Mode::ConfirmQuit, Command::ForceQuit),
			("quit ctrl+q", key(KeyCode::Char('q'), M::CONTROL), Mode::ConfirmQuit, Command::CancelQuit),
			("quit esc", key(KeyCode::Esc, M::NONE), Mode::ConfirmQuit, Command::CancelQuit),
			("quit a noop", key(KeyCode::Char('a'), M::NONE), Mode::ConfirmQuit, Command::Noop),
			// ReplacingWith
			("replwith esc", key(KeyCode::Esc, M::NONE), Mode::ReplacingWith, Command::ReplaceCancel),
			("replwith enter", key(KeyCode::Enter, M::NONE), Mode::ReplacingWith, Command::ReplaceWithConfirm),
			("replwith bs", key(KeyCode::Backspace, M::NONE), Mode::ReplacingWith, Command::ReplaceDeleteChar),
			("replwith left", key(KeyCode::Left, M::NONE), Mode::ReplacingWith, Command::PromptCursorLeft),
			("replwith right", key(KeyCode::Right, M::NONE), Mode::ReplacingWith, Command::PromptCursorRight),
			("replwith x", key(KeyCode::Char('x'), M::NONE), Mode::ReplacingWith, Command::ReplaceInsertChar('x')),
			("replwith ctrl+v paste", key(KeyCode::Char('v'), M::CONTROL), Mode::ReplacingWith, Command::Paste),
			// ReplacingStep
			("replstep ctrl+y", key(KeyCode::Char('y'), M::CONTROL), Mode::ReplacingStep, Command::ReplaceActionYes),
			("replstep ctrl+n", key(KeyCode::Char('n'), M::CONTROL), Mode::ReplacingStep, Command::ReplaceActionNo),
			("replstep ctrl+a", key(KeyCode::Char('a'), M::CONTROL), Mode::ReplacingStep, Command::ReplaceActionAll),
			("replstep esc", key(KeyCode::Esc, M::NONE), Mode::ReplacingStep, Command::ReplaceCancel),
			("replstep y noop", key(KeyCode::Char('y'), M::NONE), Mode::ReplacingStep, Command::Noop),
			// RecoverSwap
			("recover ctrl+y", key(KeyCode::Char('y'), M::CONTROL), Mode::RecoverSwap, Command::RecoverSwapAccept),
			("recover ctrl+n", key(KeyCode::Char('n'), M::CONTROL), Mode::RecoverSwap, Command::RecoverSwapDecline),
			("recover ctrl+q", key(KeyCode::Char('q'), M::CONTROL), Mode::RecoverSwap, Command::ForceQuitAll),
			("recover esc", key(KeyCode::Esc, M::NONE), Mode::RecoverSwap, Command::ForceQuitAll),
			("recover a noop", key(KeyCode::Char('a'), M::NONE), Mode::RecoverSwap, Command::Noop),
		]);
	}

	#[test]
	fn palette_mode_bindings() {
		use KeyModifiers as M;
		assert_map(&[
			("esc", key(KeyCode::Esc, M::NONE), Mode::Palette, Command::PaletteCancel),
			("enter", key(KeyCode::Enter, M::NONE), Mode::Palette, Command::PaletteConfirm),
			("up", key(KeyCode::Up, M::NONE), Mode::Palette, Command::PaletteUp),
			("down", key(KeyCode::Down, M::NONE), Mode::Palette, Command::PaletteDown),
			("pageup", key(KeyCode::PageUp, M::NONE), Mode::Palette, Command::PalettePageUp),
			("pagedown", key(KeyCode::PageDown, M::NONE), Mode::Palette, Command::PalettePageDown),
			("backspace", key(KeyCode::Backspace, M::NONE), Mode::Palette, Command::PaletteDeleteChar),
			("ctrl+s save", key(KeyCode::Char('s'), M::CONTROL), Mode::Palette, Command::PaletteClosePromptSave),
			("ctrl+d close", key(KeyCode::Char('d'), M::CONTROL), Mode::Palette, Command::PaletteCloseBuffer),
			("char f", key(KeyCode::Char('f'), M::NONE), Mode::Palette, Command::PaletteInsertChar('f')),
			("ctrl+v paste", key(KeyCode::Char('v'), M::CONTROL), Mode::Palette, Command::Paste),
			("ctrl+p noop", key(KeyCode::Char('p'), M::CONTROL), Mode::Palette, Command::Noop),
		]);
	}

	#[test]
	fn paste_and_non_key_events() {
		assert_map(&[
			(
				"paste",
				Event::Paste("hello".into()),
				Mode::Editing,
				Command::InsertString("hello".into()),
			),
			(
				"paste in search routes to prompt",
				Event::Paste("x".into()),
				Mode::Searching,
				Command::InsertString("x".into()),
			),
			(
				"paste in goto routes to prompt",
				Event::Paste("12".into()),
				Mode::GoToLine,
				Command::InsertString("12".into()),
			),
			(
				"paste in confirm-quit is noop",
				Event::Paste("x".into()),
				Mode::ConfirmQuit,
				Command::Noop,
			),
			(
				"paste in replace-step is noop",
				Event::Paste("x".into()),
				Mode::ReplacingStep,
				Command::Noop,
			),
			("resize noop", Event::Resize(80, 24), Mode::Editing, Command::Noop),
		]);
	}

	fn mouse(kind: crossterm::event::MouseEventKind, col: u16, row: u16) -> Event {
		use crossterm::event::MouseEvent;
		Event::Mouse(MouseEvent {
			kind,
			column: col,
			row,
			modifiers: KeyModifiers::NONE,
		})
	}

	fn mouse_mods(
		kind: crossterm::event::MouseEventKind,
		col: u16,
		row: u16,
		mods: KeyModifiers,
	) -> Event {
		use crossterm::event::MouseEvent;
		Event::Mouse(MouseEvent {
			kind,
			column: col,
			row,
			modifiers: mods,
		})
	}

	#[test]
	fn mouse_editing_maps_click_drag_wheel() {
		use crossterm::event::{MouseButton, MouseEventKind as K};
		assert_map(&[
			(
				"down",
				mouse(K::Down(MouseButton::Left), 3, 5),
				Mode::Editing,
				Command::MouseDown {
					col: 3,
					row: 5,
					extend: false,
				},
			),
			(
				"shift+down",
				mouse_mods(K::Down(MouseButton::Left), 3, 5, KeyModifiers::SHIFT),
				Mode::Editing,
				Command::MouseDown {
					col: 3,
					row: 5,
					extend: true,
				},
			),
			(
				"drag",
				mouse(K::Drag(MouseButton::Left), 4, 6),
				Mode::Editing,
				Command::MouseDrag { col: 4, row: 6 },
			),
			(
				"up",
				mouse(K::Up(MouseButton::Left), 4, 6),
				Mode::Editing,
				Command::MouseUp { col: 4, row: 6 },
			),
			(
				"wheel up",
				mouse(K::ScrollUp, 0, 0),
				Mode::Editing,
				Command::ScrollViewportUp,
			),
			(
				"wheel down",
				mouse(K::ScrollDown, 0, 0),
				Mode::Editing,
				Command::ScrollViewportDown,
			),
			(
				"right click noop",
				mouse(K::Down(MouseButton::Right), 1, 1),
				Mode::Editing,
				Command::Noop,
			),
		]);
	}

	#[test]
	fn mouse_ignored_in_palette_and_search() {
		use crossterm::event::{MouseButton, MouseEventKind as K};
		let ev = mouse(K::Down(MouseButton::Left), 2, 2);
		assert_eq!(map_event(&ev, Mode::Palette), Command::Noop);
		assert_eq!(map_event(&ev, Mode::Searching), Command::Noop);
		assert_eq!(
			map_event(&mouse(K::ScrollDown, 0, 0), Mode::Palette),
			Command::Noop
		);
	}

	#[test]
	fn ctrl_r_overload_depends_on_mode() {
		// Editing: whitespace toggle. Searching: promote to replace.
		use KeyModifiers as M;
		assert_map(&[
			(
				"editing ctrl+r",
				key(KeyCode::Char('r'), M::CONTROL),
				Mode::Editing,
				Command::ToggleWhitespace,
			),
			(
				"searching ctrl+r",
				key(KeyCode::Char('r'), M::CONTROL),
				Mode::Searching,
				Command::SearchConvertToReplace,
			),
		]);
	}
}

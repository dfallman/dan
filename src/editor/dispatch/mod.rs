//! Command dispatch — `Editor::execute` and domain handlers.
//!
//! Split from a single match into domain modules (motion, editing, clipboard,
//! search, replace, prompts, file, palette_buffers, toggles, transforms).

mod clipboard;
mod editing;
mod file;
mod motion;
mod mouse;
mod palette_buffers;
pub(super) mod prompts;
mod replace;
mod search;
mod toggles;
mod transforms;

use super::commands::{Command, EditAction};
use super::Editor;

impl Editor {
	/// Execute a command.
	pub fn execute(&mut self, cmd: Command) {
		let action = match &cmd {
			Command::InsertChar(ch) if ch.is_whitespace() || ch.is_ascii_punctuation() => EditAction::Whitespace,
			Command::InsertChar(_) | Command::InsertTab | Command::Paste | Command::Dedent | Command::InsertString(_) => EditAction::Insert,
			Command::InsertNewline => EditAction::Whitespace,
			Command::DeleteBackward | Command::DeleteForward | Command::DeleteLine | Command::Cut => EditAction::Delete,
			_ => EditAction::Other,
		};

		if action != self.last_edit_action || action == EditAction::Other {
			self.buffer_mut().commit_edits();
		}
		self.last_edit_action = action;

		// Wheel / Ctrl+↑↓ pan pins the viewport so render won't yank back to
		// the selection head. Any other real command (including Shift+arrow
		// select) clears the pin so the cursor stays visible again. Noop is
		// ignored so release/noise events don't accidentally unpin.
		match cmd {
			Command::ScrollViewportUp | Command::ScrollViewportDown => {
				self.pin_viewport = true;
			}
			Command::Noop => {}
			_ => {
				self.pin_viewport = false;
			}
		}

		match cmd {
			Command::MoveLeft => self.cmd_move_left(),
			Command::MoveRight => self.cmd_move_right(),
			Command::MoveUp => self.cmd_move_up(),
			Command::MoveDown => self.cmd_move_down(),
			Command::MoveLineStart => self.cmd_move_line_start(),
			Command::MoveLineEnd => self.cmd_move_line_end(),
			Command::MoveWordForward => self.cmd_move_word_forward(),
			Command::MoveWordBackward => self.cmd_move_word_backward(),
			Command::SwapLineUp => self.cmd_swap_line_up(),
			Command::SwapLineDown => self.cmd_swap_line_down(),
			Command::MoveBufferTop => self.cmd_move_buffer_top(),
			Command::MoveBufferBottom => self.cmd_move_buffer_bottom(),
			Command::PageUp => self.cmd_page_up(),
			Command::PageDown => self.cmd_page_down(),
			Command::ScrollViewportUp => self.cmd_scroll_viewport_up(),
			Command::ScrollViewportDown => self.cmd_scroll_viewport_down(),
			Command::MoveFastUp => self.cmd_move_fast_up(),
			Command::MoveFastDown => self.cmd_move_fast_down(),
			Command::SelectLeft => self.cmd_select_left(),
			Command::SelectRight => self.cmd_select_right(),
			Command::SelectUp => self.cmd_select_up(),
			Command::SelectDown => self.cmd_select_down(),
			Command::SelectWordForward => self.cmd_select_word_forward(),
			Command::SelectWordBackward => self.cmd_select_word_backward(),
			Command::SelectLineStart => self.cmd_select_line_start(),
			Command::SelectLineEnd => self.cmd_select_line_end(),
			Command::SelectAll => self.cmd_select_all(),
			Command::MouseDown { col, row, extend } => self.cmd_mouse_down(col, row, extend),
			Command::MouseDrag { col, row } => self.cmd_mouse_drag(col, row),
			Command::MouseUp { col, row } => self.cmd_mouse_up(col, row),
			Command::InsertChar(ch) => self.cmd_insert_char(ch),
			Command::InsertString(s) => self.cmd_insert_string(s),
			Command::InsertNewline => self.cmd_insert_newline(),
			Command::InsertTab => self.cmd_insert_tab(),
			Command::Dedent => self.cmd_dedent(),
			Command::DeleteBackward => self.cmd_delete_backward(),
			Command::DeleteForward => self.cmd_delete_forward(),
			Command::DeleteLine => self.cmd_delete_line(),
			Command::DuplicateLineOrSelection => self.cmd_duplicate_line_or_selection(),
			Command::Undo => self.cmd_undo(),
			Command::Redo => self.cmd_redo(),
			Command::Copy => self.cmd_copy(),
			Command::Cut => self.cmd_cut(),
			Command::Paste => self.cmd_paste(),
			Command::SearchForward => self.cmd_search_forward(),
			Command::SearchInsertChar(ch) => self.cmd_search_insert_char(ch),
			Command::SearchDeleteChar => self.cmd_search_delete_char(),
			Command::SearchConfirm => self.cmd_search_confirm(),
			Command::SearchCancel => self.cmd_search_cancel(),
			Command::SearchConvertToReplace => self.cmd_search_convert_to_replace(),
			Command::SearchNext => self.cmd_search_next(),
			Command::SearchPrev => self.cmd_search_prev(),
			Command::ReplaceInsertChar(ch) => self.cmd_replace_insert_char(ch),
			Command::ReplaceDeleteChar => self.cmd_replace_delete_char(),
			Command::ReplaceWithConfirm => self.cmd_replace_with_confirm(),
			Command::ReplaceActionYes => self.cmd_replace_action_yes(),
			Command::ReplaceActionNo => self.cmd_replace_action_no(),
			Command::ReplaceActionAll => self.cmd_replace_action_all(),
			Command::ReplaceCancel => self.cmd_replace_cancel(),
			Command::GoToLineOpen => self.cmd_go_to_line_open(),
			Command::GoToLineInsertChar(ch) => self.cmd_go_to_line_insert_char(ch),
			Command::GoToLineDeleteChar => self.cmd_go_to_line_delete_char(),
			Command::GoToLineConfirm => self.cmd_go_to_line_confirm(),
			Command::GoToLineCancel => self.cmd_go_to_line_cancel(),
			Command::SaveAsOpen => self.cmd_save_as_open(),
			Command::SaveAsInsertChar(ch) => self.cmd_save_as_insert_char(ch),
			Command::SaveAsDeleteChar => self.cmd_save_as_delete_char(),
			Command::PromptCursorLeft => self.cmd_prompt_cursor_left(),
			Command::PromptCursorRight => self.cmd_prompt_cursor_right(),
			Command::SaveAsConfirm => self.cmd_save_as_confirm(),
			Command::SaveAsCancel => self.cmd_save_as_cancel(),
			Command::ConfirmOverwrite => self.cmd_confirm_overwrite(),
			Command::CancelOverwrite => self.cmd_cancel_overwrite(),
			Command::Save => self.cmd_save(),
			Command::Quit => self.cmd_quit(),
			Command::ForceQuit => self.cmd_force_quit(),
			Command::ForceQuitAll => self.cmd_force_quit_all(),
			Command::SaveAndQuit => self.cmd_save_and_quit(),
			Command::CancelQuit => self.cmd_cancel_quit(),
			Command::RecoverSwapAccept => self.cmd_recover_swap_accept(),
			Command::RecoverSwapDecline => self.cmd_recover_swap_decline(),
			Command::CopyPathAbs => self.cmd_copy_path_abs(),
			Command::CopyPathRel => self.cmd_copy_path_rel(),
			Command::RevealInFinder => self.cmd_reveal_in_finder(),
			Command::OpenContainingFolder => self.cmd_open_containing_folder(),
			Command::ShowBufferInfo => self.cmd_show_buffer_info(),
			Command::PaletteOpen => self.cmd_palette_open(),
			Command::PaletteCancel => self.cmd_palette_cancel(),
			Command::PaletteInsertChar(ch) => self.cmd_palette_insert_char(ch),
			Command::PaletteDeleteChar => self.cmd_palette_delete_char(),
			Command::PaletteUp => self.cmd_palette_up(),
			Command::PaletteDown => self.cmd_palette_down(),
			Command::PalettePageUp => self.cmd_palette_page_up(),
			Command::PalettePageDown => self.cmd_palette_page_down(),
			Command::PaletteConfirm => self.cmd_palette_confirm(),
			Command::PaletteCloseBuffer => self.cmd_palette_close_buffer(),
			Command::PaletteClosePromptSave => self.cmd_palette_close_prompt_save(),
			Command::PaletteClosePromptDiscard => self.cmd_palette_close_prompt_discard(),
			Command::PaletteClosePromptCancel => self.cmd_palette_close_prompt_cancel(),
			Command::NewBuffer => self.cmd_new_buffer(),
			Command::OpenFilePicker => self.cmd_open_file_picker(),
			Command::ReloadBuffer => self.cmd_reload_buffer(),
			Command::CloseBuffer => self.cmd_close_buffer(),
			Command::CloseOthers => self.cmd_close_others(),
			Command::CloseAll => self.cmd_close_all(),
			Command::SaveAll => self.cmd_save_all(),
			Command::ShowRecentFiles => self.cmd_show_recent_files(),
			Command::ToggleHelp => self.cmd_toggle_help(),
			Command::ToggleWrap => self.cmd_toggle_wrap(),
			Command::ToggleWhitespace => self.cmd_toggle_whitespace(),
			Command::FormatDocument => self.cmd_format_document(),
			Command::ToggleComment => self.cmd_toggle_comment(),
			Command::ToggleSyntax => self.cmd_toggle_syntax(),
			Command::ToggleLineNumbers => self.cmd_toggle_line_numbers(),
			Command::ReloadConfiguration => self.cmd_reload_configuration(),
			Command::ShowVersion => self.cmd_show_version(),
			Command::IndentSpaces => self.cmd_indent_spaces(),
			Command::IndentTabs => self.cmd_indent_tabs(),
			Command::TabWidth(w) => self.cmd_tab_width(w),
			Command::LineEndingsLF => self.cmd_line_endings_l_f(),
			Command::LineEndingsCRLF => self.cmd_line_endings_c_r_l_f(),
			Command::TrimTrailingWhitespaceNow => self.cmd_trim_trailing_whitespace_now(),
			Command::ConvertTabsToSpaces => self.cmd_convert_tabs_to_spaces(),
			Command::ConvertSpacesToTabs => self.cmd_convert_spaces_to_tabs(),
			Command::SortLinesAsc => self.cmd_sort_lines_asc(),
			Command::SortLinesDesc => self.cmd_sort_lines_desc(),
			Command::DedupAdjacent => self.cmd_dedup_adjacent(),
			Command::ConvertUpper => self.cmd_convert_upper(),
			Command::ConvertLower => self.cmd_convert_lower(),
			Command::ConvertTitle => self.cmd_convert_title(),
			Command::ReverseSelection => self.cmd_reverse_selection(),
			Command::Noop => {}
		}
	}
}

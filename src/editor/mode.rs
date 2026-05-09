use crossterm::style::Color;

/// Editor mode — editing is the default, no modal switching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
	/// Normal editing — typing inserts text, arrows move cursor.
	Editing,
	/// Incremental search — typing updates the search query.
	Searching,
	/// Go-to-line prompt — typing enters a line number.
	GoToLine,
	/// Save-as prompt — typing enters a file path.
	SaveAs,
	/// Confirming quit with unsaved changes.
	ConfirmQuit,
	/// Confirming overwrite of existing file.
	ConfirmOverwrite,
	/// Replace prompt: entering the replacement string.
	ReplacingWith,
	/// Replace prompt: confirming each match (y/n/a/q).
	ReplacingStep,
	/// Recover-from-swap prompt: choose recover / keep-mine / discard.
	RecoverSwap,
}

impl Mode {
	/// Background color used for this mode's status-bar segment.
	pub fn color(self, theme: &crate::ui::theme::Theme) -> Color {
		match self {
			Mode::Editing => theme.mode_edit,
			Mode::Searching => theme.mode_search,
			Mode::GoToLine => theme.mode_goto,
			Mode::SaveAs => theme.mode_save,
			Mode::ConfirmQuit => theme.mode_danger,
			Mode::ConfirmOverwrite => theme.mode_danger,
			Mode::ReplacingWith => theme.mode_replace,
			Mode::ReplacingStep => theme.mode_replace,
			Mode::RecoverSwap => theme.mode_danger,
		}
	}
}



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
	/// Capturing global replacement macro string.
	ReplacingWith,
	/// Interactive Replace confirmation step.
	ReplacingStep,
	/// Interactive `.swp` recovery lock structurally trapping users natively!
	RecoverSwap,
}

impl Mode {
	/// Label shown in the status bar.



	/// Status bar background color mapping visually cleanly to the explicit active theme contexts globally natively.
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



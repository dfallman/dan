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
	/// Capturing global replace target string.
	ReplacingSearch,
	/// Capturing global replacement macro string.
	ReplacingWith,
	/// Interactive Replace confirmation step.
	ReplacingStep,
	/// Interactive `.swp` recovery lock structurally trapping users natively!
	RecoverSwap,
}

impl Mode {
	/// Label shown in the status bar.
	pub fn label(self) -> &'static str {
		match self {
			Mode::Editing => "EDIT",
			Mode::Searching => "FIND",
			Mode::GoToLine => "GOTO",
			Mode::SaveAs => "SAVE",
			Mode::ConfirmQuit => "QUIT",
			Mode::ConfirmOverwrite => "SAVE",
			Mode::ReplacingSearch => "REPL",
			Mode::ReplacingWith => "RWTH",
			Mode::ReplacingStep => "REPL",
			Mode::RecoverSwap => "SWAP",
		}
	}

	/// Status bar background color.
	pub fn color(self) -> Color {
		match self {
			Mode::Editing => Color::Blue,
			Mode::Searching => Color::DarkYellow,
			Mode::GoToLine => Color::DarkCyan,
			Mode::SaveAs => Color::DarkGreen,
			Mode::ConfirmQuit => Color::DarkRed,
			Mode::ConfirmOverwrite => Color::DarkRed,
			Mode::ReplacingSearch => Color::DarkMagenta,
			Mode::ReplacingWith => Color::DarkMagenta,
			Mode::ReplacingStep => Color::DarkMagenta,
			Mode::RecoverSwap => Color::DarkRed,
		}
	}
}

impl std::fmt::Display for Mode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.label())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_mode_labels() {
		assert_eq!(Mode::Editing.label(), "EDIT");
		assert_eq!(Mode::Searching.label(), "FIND");
		assert_eq!(Mode::GoToLine.label(), "GOTO");
		assert_eq!(Mode::SaveAs.label(), "SAVE");
		assert_eq!(Mode::ConfirmQuit.label(), "QUIT");
		assert_eq!(Mode::ConfirmOverwrite.label(), "SAVE");
	}

	#[test]
	fn test_mode_display() {
		assert_eq!(format!("{}", Mode::Editing), "EDIT");
	}
}

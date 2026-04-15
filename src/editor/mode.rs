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
			Mode::Editing => "Edit",
			Mode::Searching => "Find",
			Mode::GoToLine => "Goto",
			Mode::SaveAs => "Save",
			Mode::ConfirmQuit => "Quit",
			Mode::ConfirmOverwrite => "Save",
			Mode::ReplacingSearch => "Repl",
			Mode::ReplacingWith => "Rwth",
			Mode::ReplacingStep => "Repl",
			Mode::RecoverSwap => "Swap",
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
		assert_eq!(Mode::Editing.label(), "Edit");
		assert_eq!(Mode::Searching.label(), "Find");
		assert_eq!(Mode::GoToLine.label(), "Goto");
		assert_eq!(Mode::SaveAs.label(), "Save");
		assert_eq!(Mode::ConfirmQuit.label(), "Quit");
		assert_eq!(Mode::ConfirmOverwrite.label(), "Save");
	}

	#[test]
	fn test_mode_display() {
		assert_eq!(format!("{}", Mode::Editing), "Edit");
	}
}

use crossterm::style::Color;

/// Editor mode — pico-style: editing is the default, no modal switching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Normal editing — typing inserts text, arrows move cursor.
    Editing,
    /// Incremental search — typing updates the search query.
    Searching,
}

impl Mode {
    /// Label shown in the status bar.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Editing => "EDIT",
            Mode::Searching => "FIND",
        }
    }

    /// Status bar background color.
    pub fn color(self) -> Color {
        match self {
            Mode::Editing => Color::Blue,
            Mode::Searching => Color::DarkYellow,
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
    }

    #[test]
    fn test_mode_display() {
        assert_eq!(format!("{}", Mode::Editing), "EDIT");
    }
}

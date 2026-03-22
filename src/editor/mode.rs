/// Editor mode — pico-style: editing is the default, no modal switching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Normal editing — typing inserts text, arrows move cursor.
    Editing,
    /// Selecting text (shift+arrows or Ctrl+A).
    Selecting,
}

impl Mode {
    /// Label shown in the status bar.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Editing => "EDIT",
            Mode::Selecting => "SELECT",
        }
    }

    /// Status bar color as (r, g, b).
    pub fn color(self) -> (u8, u8, u8) {
        match self {
            Mode::Editing => (100, 180, 255),   // blue
            Mode::Selecting => (255, 200, 80),  // gold
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
        assert_eq!(Mode::Selecting.label(), "SELECT");
    }

    #[test]
    fn test_mode_display() {
        assert_eq!(format!("{}", Mode::Editing), "EDIT");
    }
}

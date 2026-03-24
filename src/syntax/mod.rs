// Syntax highlighting powered by syntect.

use std::path::Path;

use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

/// Holds loaded syntax definitions and themes for highlighting.
pub struct Highlighter {
    pub syntax_set: SyntaxSet,
    pub theme: Theme,
}

impl Highlighter {
    /// Create a highlighter with bundled defaults.
    pub fn new(theme_name: &str) -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set
            .themes
            .get(theme_name)
            .or_else(|| theme_set.themes.get("base16-eighties.dark"))
            .cloned()
            .unwrap_or_else(|| {
                theme_set
                    .themes
                    .values()
                    .next()
                    .expect("syntect ships at least one theme")
                    .clone()
            });
        Self { syntax_set, theme }
    }

    /// Detect the appropriate syntax for a file path (by extension).
    /// Falls back to plain-text if the extension is unknown or path is None.
    pub fn detect_syntax(&self, path: Option<&Path>) -> &SyntaxReference {
        if let Some(p) = path {
            self.syntax_set
                .find_syntax_for_file(p)
                .ok()
                .flatten()
                .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text())
        } else {
            self.syntax_set.find_syntax_plain_text()
        }
    }
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::new("base16-eighties.dark")
    }
}

// Syntax highlighting powered by syntect.

use std::path::Path;

use syntect::highlighting::Theme;
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect_assets::assets::HighlightingAssets;

/// Holds loaded syntax definitions and themes for highlighting.
pub struct Highlighter {
	pub syntax_set: SyntaxSet,
	pub theme: Theme,
}

impl Highlighter {
	/// Create a highlighter with bundled defaults.
	pub fn new(theme_name: &str) -> Self {
		let assets = HighlightingAssets::from_binary();

		let syntax_set = match assets.get_syntax_set() {
			Ok(s) => s.clone(),
			Err(_) => SyntaxSet::load_defaults_newlines(),
		};

		// Check if the theme exists safely (ignores case), otherwise fallback
		let active_theme_name = if let Some(matched) = assets.themes().find(|name| name.eq_ignore_ascii_case(theme_name)) {
			matched
		} else {
			if cfg!(debug_assertions) {
				eprintln!("[DEBUG] Theme '{}' not found, falling back to OneHalfDark", theme_name);
			}
			"OneHalfDark" // A safe, great looking fallback bundled in syntect-assets
		};

		let theme = assets.get_theme(active_theme_name).clone();

		Self { syntax_set, theme }
	}

	/// Detect the appropriate syntax for a file path (by extension).
	/// Falls back to plain-text if the extension is unknown or path is None.
	pub fn detect_syntax(&self, path: Option<&Path>) -> &SyntaxReference {
		path.and_then(|p| p.extension())
			.and_then(|ext| ext.to_str())
			.and_then(|ext_str| self.syntax_set.find_syntax_by_extension(ext_str))
			.unwrap_or_else(|| self.syntax_set.find_syntax_plain_text())
	}
}

impl Default for Highlighter {
	fn default() -> Self {
		Self::new("OneHalfDark")
	}
}

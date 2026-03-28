use serde::Deserialize;
use std::path::PathBuf;

/// Editor configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
	/// Tab width in spaces.
	pub tab_width: usize,
	/// Whether to expand tabs to spaces.
	pub expand_tab: bool,
	/// Show line numbers.
	pub line_numbers: bool,
	/// Highlight the line the cursor is on.
	pub highlight_active: bool,
	/// Scroll padding (lines above/below cursor to keep visible).
	pub scroll_off: usize,
	/// Theme name (used for syntect theme selection).
	pub theme: String,
	/// Wrap long lines (true) or scroll horizontally (false).
	pub wrap_lines: bool,
	/// Enable syntax highlighting (requires a file with a known extension).
	#[serde(alias = "syntax_highlighting")]
	pub syntax_highlight: bool,
	/// Auto-indent new lines (copy leading whitespace from current line).
	pub auto_indent: bool,
	/// Show "^H Help" in the toolbar.
	pub show_help: bool,
	/// Show detected language in the toolbar.
	pub show_lang: bool,
}

impl Default for Config {
	fn default() -> Self {
		Self {
			tab_width: 4,
			expand_tab: false,
			line_numbers: true,
			highlight_active: true,
			scroll_off: 5,
			theme: "base16-eighties.dark".to_string(),
			wrap_lines: true,
			syntax_highlight: true,
			auto_indent: true,
			show_help: true,
			show_lang: true,
		}
	}
}

impl Config {
	/// Load config from the default config path (~/.config/dan/config.toml).
	pub fn load() -> Self {
		let mut config = Self::default();
		if let Some(path) = config_path() {
			if path.exists() {
				if let Ok(content) = std::fs::read_to_string(&path) {
					if let Ok(c) = toml::from_str(&content) {
						config = c;
					}
				}
			}
		}

		// Try local config (allows local developer overrides)
		let local_path = PathBuf::from("config.toml");
		if local_path.exists() {
			if let Ok(content) = std::fs::read_to_string(&local_path) {
				if let Ok(c) = toml::from_str(&content) {
					config = c;
				}
			}
		}

		// Disable colors if NO_COLOR is present and not empty
		if let Ok(val) = std::env::var("NO_COLOR") {
			if !val.is_empty() {
				config.highlight_active = false;
				config.syntax_highlight = false;
			}
		}

		config
	}
}

/// Get the config file path.
fn config_path() -> Option<PathBuf> {
	dirs::config_dir().map(|d| d.join("dan").join("config.toml"))
}

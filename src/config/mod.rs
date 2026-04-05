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
	/// Show character encoding in the toolbar.
	pub show_encoding: bool,
	/// Show detected language in the toolbar.
	pub show_lang: bool,
	/// How many lines to jump for fast scroll navigation (Ctrl+Shift+Up/Down)
	pub fast_scroll_steps: usize,
	/// Automatically insert closing brackets and quotes
	pub auto_close: bool,
	/// Formats all excess spaces terminating lines globally during save commits.
	#[serde(skip)]
	pub trim_trailing_whitespace: Option<bool>,
	/// Line termination style requested statically (LF / CRLF).
	#[serde(skip)]
	pub end_of_line: Option<String>,
}

impl Default for Config {
	fn default() -> Self {
		Self {
			tab_width: 4,
			expand_tab: false,
			line_numbers: true,
			highlight_active: true,
			scroll_off: 5,
			theme: "OneHalfDark".to_string(),
			wrap_lines: true,
			syntax_highlight: true,
			auto_indent: true,
			show_help: true,
			show_encoding: true,
			show_lang: true,
			fast_scroll_steps: 10,
			auto_close: true,
			trim_trailing_whitespace: None,
			end_of_line: None,
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
					if cfg!(debug_assertions) {
						eprintln!("[DEBUG] Config::load() read global config from: {}", path.display());
					}
					if let Ok(c) = toml::from_str(&content) {
						config = c;
					}
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

	/// Dynamically overrides the Active formatting parameters natively tracking `.editorconfig` components.
	pub fn apply_editorconfig(&mut self, path: &std::path::Path) {
		let query_path = if path.is_absolute() {
			path.to_path_buf()
		} else if let Ok(cwd) = std::env::current_dir() {
			cwd.join(path)
		} else {
			return; // Gracefully abort if directory structure cannot be fundamentally determined
		};

		if let Ok(conf) = editorconfig::get_config(&query_path) {
			if cfg!(debug_assertions) {
				eprintln!("[DEBUG] apply_editorconfig() parsed .editorconfig applying overrides for: {}", query_path.display());
			}
			
			if let Some(style) = conf.get("indent_style") {
				self.expand_tab = style == "space";
			}
			if let Some(size) = conf.get("indent_size") {
				if let Ok(w) = size.parse::<usize>() {
					self.tab_width = w;
				}
			}
			if let Some(trim) = conf.get("trim_trailing_whitespace") {
				self.trim_trailing_whitespace = Some(trim == "true");
			}
			if let Some(eol) = conf.get("end_of_line") {
				self.end_of_line = Some(eol.to_string());
			}
		}
	}
}

/// Get the config file path.
fn config_path() -> Option<PathBuf> {
	dirs::config_dir().map(|d| d.join("dan").join("config.toml"))
}

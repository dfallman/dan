use editorconfig_parser::{EditorConfig, EditorConfigProperty, EndOfLine, IndentStyle};
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
	/// Indent soft-wrapped continuation rows to match leading indentation.
	pub breakindent: bool,
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
	/// Show the full path in the status bar instead of just the filename
	pub show_full_path: bool,
	/// Automatically insert closing brackets and quotes
	pub auto_close: bool,
	/// Render comment scopes in italics.
	pub comments_are_italics: bool,
	/// Render whitespace characters (spaces, tabs, line endings) visually.
	pub show_whitespace: bool,
	/// Strip trailing whitespace from each line on save (None = leave as-is).
	#[serde(skip)]
	pub trim_trailing_whitespace: Option<bool>,
	/// Line termination style requested statically (LF / CRLF).
	#[serde(skip)]
	pub end_of_line: Option<String>,
	/// Enable terminal mouse capture (click, drag-select, wheel).
	pub mouse: bool,
	/// Terminal cursor shape: "block", "line", or "underscore".
	pub cursor_style: String,
	/// Whether the terminal cursor blinks.
	pub cursor_blink: bool,
	/// Optional cursor color as "#RRGGBB" or "#RGB". Invalid values are ignored at use time.
	pub cursor_color: Option<String>,
}

impl Default for Config {
	fn default() -> Self {
		Self {
			tab_width: 4,
			expand_tab: false,
			line_numbers: true,
			highlight_active: true,
			scroll_off: 5,
			// "default" auto-detects via COLORFGBG then OSC (OneHalfDark /
			// OneHalfLight). Explicit theme names skip the OSC query.
			theme: "default".to_string(),
			wrap_lines: true,
			breakindent: false,
			syntax_highlight: true,
			auto_indent: true,
			show_help: true,
			show_encoding: true,
			show_lang: true,
			show_full_path: false,
			fast_scroll_steps: 10,
			auto_close: true,
			comments_are_italics: true,
			show_whitespace: false,
			trim_trailing_whitespace: None,
			end_of_line: None,
			mouse: true,
			cursor_style: "block".to_string(),
			cursor_blink: false,
			cursor_color: None,
		}
	}
}

impl Config {
	/// Map `cursor_style` + `cursor_blink` to a crossterm cursor style.
	/// Unknown styles behave as `"block"`.
	pub fn terminal_cursor_style(&self) -> crossterm::cursor::SetCursorStyle {
		use crossterm::cursor::SetCursorStyle;
		match (self.cursor_style.as_str(), self.cursor_blink) {
			("line", false) => SetCursorStyle::SteadyBar,
			("line", true) => SetCursorStyle::BlinkingBar,
			("underscore", false) => SetCursorStyle::SteadyUnderScore,
			("underscore", true) => SetCursorStyle::BlinkingUnderScore,
			(_, false) => SetCursorStyle::SteadyBlock,
			(_, true) => SetCursorStyle::BlinkingBlock,
		}
	}

	/// Parsed RGB if `cursor_color` is a valid `#RGB` / `#RRGGBB` hex string.
	pub fn cursor_color_rgb(&self) -> Option<[u8; 3]> {
		self.cursor_color.as_deref().and_then(parse_cursor_color)
	}

	/// Load config from the default config path (~/.config/dan/config.toml).
	pub fn load() -> Self {
		let mut config = Self::default();
		if let Some(path) = config_path() {
			if path.exists() {
				if let Ok(content) = std::fs::read_to_string(&path) {
					if cfg!(debug_assertions) {
						eprintln!("[DEBUG] Config::load() read global config from: {}", path.display());
					}
					// Accept CRLF and bare-CR line endings. TOML requires `\n`;
					// a pasted Cursor section with `\r` would otherwise fail the
					// whole file and silently fall back to defaults.
					let content = content.replace("\r\n", "\n").replace('\r', "\n");
					match toml::from_str(&content) {
						Ok(c) => config = c,
						Err(e) => {
							eprintln!(
								"dan: could not parse {}: {e}\nUsing defaults.",
								path.display()
							);
						}
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

	/// Overlay any matching `.editorconfig` settings onto the current config.
	pub fn apply_editorconfig(&mut self, path: &std::path::Path) {
		let query_path = if path.is_absolute() {
			path.to_path_buf()
		} else if let Ok(cwd) = std::env::current_dir() {
			cwd.join(path)
		} else {
			return; // Gracefully abort if directory structure cannot be fundamentally determined
		};

		for conf in collect_editorconfigs(&query_path).iter().rev() {
			if cfg!(debug_assertions) {
				eprintln!("[DEBUG] apply_editorconfig() parsed .editorconfig applying overrides for: {}", query_path.display());
			}

			let props = conf.resolve(&query_path);
			if let EditorConfigProperty::Value(style) = props.indent_style {
				self.expand_tab = style == IndentStyle::Space;
			}
			if let EditorConfigProperty::Value(size) = props.indent_size {
				self.tab_width = size;
			}
			if let EditorConfigProperty::Value(trim) = props.trim_trailing_whitespace {
				self.trim_trailing_whitespace = Some(trim);
			}
			if let EditorConfigProperty::Value(eol) = props.end_of_line {
				self.end_of_line = Some(
					match eol {
						EndOfLine::Lf => "lf",
						EndOfLine::Cr => "cr",
						EndOfLine::Crlf => "crlf",
					}
					.to_string(),
				);
			}
		}
	}
}

/// Collect the `.editorconfig` files governing `query_path`, nearest first.
///
/// Walks from the file's own directory up towards the filesystem root, stopping
/// after the first file declaring `root = true`. Each config carries the
/// directory it was found in, so its section globs match against a path
/// relative to that directory rather than an absolute one.
fn collect_editorconfigs(query_path: &std::path::Path) -> Vec<EditorConfig> {
	let mut found = Vec::new();
	// `.ancestors()` starts at the file itself; skip(1) begins at its directory.
	for dir in query_path.ancestors().skip(1) {
		let Ok(source) = std::fs::read_to_string(dir.join(".editorconfig")) else {
			continue;
		};
		let conf = EditorConfig::parse(&source).with_cwd(dir);
		let is_root = conf.root();
		found.push(conf);
		if is_root {
			break;
		}
	}
	found
}

/// Parse `#RGB` or `#RRGGBB` (case-insensitive). Other forms return `None`.
pub(crate) fn parse_cursor_color(raw: &str) -> Option<[u8; 3]> {
	let s = raw.strip_prefix('#')?;
	let expand = |nibble: u8| (nibble << 4) | nibble;
	match s.len() {
		3 => {
			let mut bytes = [0u8; 3];
			for (i, ch) in s.chars().enumerate() {
				bytes[i] = expand(ch.to_digit(16)? as u8);
			}
			Some(bytes)
		}
		6 => {
			let r = u8::from_str_radix(&s[0..2], 16).ok()?;
			let g = u8::from_str_radix(&s[2..4], 16).ok()?;
			let b = u8::from_str_radix(&s[4..6], 16).ok()?;
			Some([r, g, b])
		}
		_ => None,
	}
}

/// Get the config file path.
fn config_path() -> Option<PathBuf> {
	let preferred = dirs::home_dir().map(|d| d.join(".config").join("dan").join("config.toml"));
	if let Some(p) = &preferred {
		if p.exists() {
			return preferred;
		}
	}
	// Fallback to OS native (e.g. ~/Library/Application Support/dan/config.toml on macOS)
	dirs::config_dir().map(|d| d.join("dan").join("config.toml"))
}

#[cfg(test)]
mod tests {
	use super::*;

	use crossterm::cursor::SetCursorStyle;

	#[test]
	fn cursor_defaults() {
		let c = Config::default();
		assert_eq!(c.cursor_style, "block");
		assert!(!c.cursor_blink);
		assert_eq!(c.cursor_color, None);
		assert_eq!(c.terminal_cursor_style(), SetCursorStyle::SteadyBlock);
		assert_eq!(c.cursor_color_rgb(), None);
	}

	#[test]
	fn cursor_style_and_blink_from_toml() {
		let c: Config = toml::from_str(
			r#"
			cursor_style = "line"
			cursor_blink = true
			"#,
		)
		.unwrap();
		assert_eq!(c.cursor_style, "line");
		assert!(c.cursor_blink);
		assert_eq!(c.terminal_cursor_style(), SetCursorStyle::BlinkingBar);
	}

	#[test]
	fn terminal_cursor_style_mapping_table() {
		let cases = [
			("block", false, SetCursorStyle::SteadyBlock),
			("block", true, SetCursorStyle::BlinkingBlock),
			("line", false, SetCursorStyle::SteadyBar),
			("line", true, SetCursorStyle::BlinkingBar),
			("underscore", false, SetCursorStyle::SteadyUnderScore),
			("underscore", true, SetCursorStyle::BlinkingUnderScore),
			("nope", false, SetCursorStyle::SteadyBlock),
			("nope", true, SetCursorStyle::BlinkingBlock),
		];
		for (style, blink, expected) in cases {
			let c = Config {
				cursor_style: style.to_string(),
				cursor_blink: blink,
				..Config::default()
			};
			assert_eq!(
				c.terminal_cursor_style(),
				expected,
				"style={style:?} blink={blink}"
			);
		}
	}

	#[test]
	fn cursor_color_hex_parsing() {
		assert_eq!(parse_cursor_color("#FF8800"), Some([0xff, 0x88, 0x00]));
		assert_eq!(parse_cursor_color("#f80"), Some([0xff, 0x88, 0x00]));
		assert_eq!(parse_cursor_color("#AbCdEf"), Some([0xab, 0xcd, 0xef]));
		assert_eq!(parse_cursor_color(""), None);
		assert_eq!(parse_cursor_color("FF8800"), None);
		assert_eq!(parse_cursor_color("#gg0000"), None);
		assert_eq!(parse_cursor_color("#12345"), None);
		assert_eq!(parse_cursor_color("rgb(255,0,0)"), None);

		let c: Config = toml::from_str(r##"cursor_color = "#0af""##).unwrap();
		assert_eq!(c.cursor_color.as_deref(), Some("#0af"));
		assert_eq!(c.cursor_color_rgb(), Some([0x00, 0xaa, 0xff]));

		let bad: Config = toml::from_str(r#"cursor_color = "nope""#).unwrap();
		assert_eq!(bad.cursor_color_rgb(), None);
	}

	#[test]
	fn toml_with_bare_cr_line_endings_parses_after_normalize() {
		// Mimic a pasted Cursor section that used CR instead of LF.
		let raw = "cursor_style = \"line\"\rcursor_blink = true\rcursor_color = \"#FF8800\"\n";
		assert!(
			toml::from_str::<Config>(raw).is_err(),
			"bare CR must be invalid TOML without normalization"
		);
		let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
		let c: Config = toml::from_str(&normalized).unwrap();
		assert_eq!(c.cursor_style, "line");
		assert!(c.cursor_blink);
		assert_eq!(c.cursor_color_rgb(), Some([0xff, 0x88, 0x00]));
	}

	#[test]
	fn mouse_defaults_to_true() {
		assert!(Config::default().mouse);
	}

	#[test]
	fn mouse_false_from_toml() {
		let c: Config = toml::from_str("mouse = false").unwrap();
		assert!(!c.mouse);
	}

	/// Scratch directory rooted in the system temp dir. The crate has no
	/// dev-dependencies, so this stands in for `tempfile`.
	struct TempTree(PathBuf);

	impl TempTree {
		fn new(tag: &str) -> Self {
			let mut dir = std::env::temp_dir();
			dir.push(format!("dan-editorconfig-{tag}-{}", std::process::id()));
			let _ = std::fs::remove_dir_all(&dir);
			std::fs::create_dir_all(&dir).unwrap();
			Self(dir)
		}

		fn write(&self, rel: &str, contents: &str) -> PathBuf {
			let path = self.0.join(rel);
			std::fs::create_dir_all(path.parent().unwrap()).unwrap();
			std::fs::write(&path, contents).unwrap();
			path
		}
	}

	impl Drop for TempTree {
		fn drop(&mut self) {
			let _ = std::fs::remove_dir_all(&self.0);
		}
	}

	#[test]
	fn editorconfig_overlays_matching_section() {
		let tree = TempTree::new("basic");
		tree.write(
			".editorconfig",
			"root = true\n[*.rs]\nindent_style = space\nindent_size = 2\ntrim_trailing_whitespace = true\nend_of_line = crlf\n",
		);
		let file = tree.write("main.rs", "");

		let mut config = Config::default();
		config.apply_editorconfig(&file);

		assert!(config.expand_tab);
		assert_eq!(config.tab_width, 2);
		assert_eq!(config.trim_trailing_whitespace, Some(true));
		assert_eq!(config.end_of_line.as_deref(), Some("crlf"));
	}

	#[test]
	fn editorconfig_leaves_defaults_when_glob_does_not_match() {
		let tree = TempTree::new("nomatch");
		tree.write(".editorconfig", "root = true\n[*.py]\nindent_size = 2\n");
		let file = tree.write("main.rs", "");

		let mut config = Config::default();
		config.apply_editorconfig(&file);

		assert_eq!(config.tab_width, 4);
		assert!(!config.expand_tab);
		assert_eq!(config.end_of_line, None);
	}

	/// A nested file must still match a top-level `[*.rs]` glob, and the
	/// nearest `.editorconfig` must win on keys both files set.
	#[test]
	fn editorconfig_nearest_file_overrides_outer() {
		let tree = TempTree::new("nearest");
		tree.write(".editorconfig", "root = true\n[*.rs]\nindent_style = space\nindent_size = 2\n");
		tree.write("src/.editorconfig", "[*.rs]\nindent_size = 8\n");
		let file = tree.write("src/main.rs", "");

		let mut config = Config::default();
		config.apply_editorconfig(&file);

		assert_eq!(config.tab_width, 8, "nearer .editorconfig should win on indent_size");
		assert!(config.expand_tab, "unset keys should still inherit from the outer file");
	}

	/// `root = true` stops the upward walk, so the outer file is never read.
	#[test]
	fn editorconfig_root_halts_upward_walk() {
		let tree = TempTree::new("rootstop");
		tree.write(".editorconfig", "[*.rs]\nindent_size = 2\n");
		tree.write("inner/.editorconfig", "root = true\n[*.rs]\nindent_style = tab\n");
		let file = tree.write("inner/main.rs", "");

		let mut config = Config::default();
		config.apply_editorconfig(&file);

		assert_eq!(config.tab_width, 4, "outer indent_size must not leak past root = true");
		assert!(!config.expand_tab);
	}

	#[test]
	fn editorconfig_maps_lf_end_of_line() {
		let tree = TempTree::new("eol");
		tree.write(".editorconfig", "root = true\n[*]\nend_of_line = lf\n");
		let file = tree.write("main.rs", "");

		let mut config = Config::default();
		config.apply_editorconfig(&file);

		assert_eq!(config.end_of_line.as_deref(), Some("lf"));
	}
}

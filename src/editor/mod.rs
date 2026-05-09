pub mod commands;
pub mod cursor;
mod dispatch;
mod editing;
pub mod formatter;
pub mod mode;
mod navigation;
mod search;
mod selection;
pub(crate) mod viewport;
pub(crate) mod visual_col;

pub(crate) use viewport::visual_rows_for;

use crate::buffer::Buffer;
use crate::config::Config;
use crate::editor::cursor::CursorSet;
use crate::editor::mode::Mode;
use crate::syntax::Highlighter;

use crossterm::terminal;

/// Core editor state — pico-style modeless editor.
pub struct Editor {
	/// Loaded configuration.
	pub config: Config,
	/// All open buffers.
	pub buffers: Vec<Buffer>,
	/// Index of the active buffer.
	pub active_buffer: usize,
	/// Current mode (Editing or Searching).
	pub mode: Mode,
	/// Cursors / selections for the active buffer.
	pub cursors: CursorSet,
	/// Status message displayed in the status bar.
	pub status_msg: Option<String>,
	/// Whether the editor should quit.
	pub should_quit: bool,
	/// Viewport scroll offset (top visible line).
	pub scroll_y: usize,
	/// Viewport visual row scroll offset (for wrap mode sub-line scrolling).
	pub scroll_vrow: usize,
	/// Horizontal scroll offset (first visible column, used when wrap_lines=false).
	pub scroll_x: usize,
	/// OS system clipboard.
	pub sys_clipboard: Option<arboard::Clipboard>,
	/// Internal fallback clipboard content.
	pub internal_clipboard: String,
	/// Current terminal width (updated on resize).
	pub terminal_width: u16,
	/// Current terminal height (updated on resize).
	pub terminal_height: u16,
	/// Suppresses the next internal Paste command after a bracketed
	/// paste event, preventing double-insert on terminals that send
	/// both Event::Paste and Event::Key(Ctrl+V).
	suppress_next_paste: bool,
	/// Whether the help legend bar is visible (toggled with ^H).
	pub show_help: bool,
	/// Current search query string (populated during search mode).
	pub search_query: String,
	/// Pattern entered in the replace prompt's first step.
	pub replace_query: String,
	/// Replacement text entered in the replace prompt's second step.
	pub replace_with: String,
	/// All current matches as (start_char, end_char) pairs.
	pub search_matches: Vec<(usize, usize)>,
	/// Index of the currently-highlighted match.
	pub search_match_idx: usize,
	/// Saved cursor position before entering search (so Esc can restore).
	pub search_saved_cursor: Option<(usize, usize)>,
	/// Last completed search query (persists across search sessions).
	last_search_query: String,
	/// Syntax highlighter (shared across buffers).
	pub highlighter: Highlighter,
	/// Per-frame parse-state snapshot cache. Lives on `Editor` (not
	/// `Highlighter`) because `ParseState` is not `Send` and `Highlighter`
	/// is constructed in a spawned thread.
	pub highlight_cache: std::cell::RefCell<crate::syntax::HighlightCache>,
	/// Current input text in the go-to-line prompt.
	pub goto_line_input: String,
	/// Current input text in the save-as prompt.
	pub save_as_input: String,
	/// Cursor position within the save-as input.
	pub prompt_cursor: usize,
	pub prompt_view_start: std::cell::Cell<usize>,
	/// Path pending overwrite confirmation.
	pub save_as_pending_path: Option<String>,
	/// Receiver for the result of an in-flight async formatter run.
	pub fmt_rx: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
	/// True while a formatter run is in flight.
	pub is_formatting: bool,
	/// `Buffer::version` captured when the formatter was spawned. On result
	/// arrival, if the buffer's current version differs the result is
	/// discarded — the user typed during the format and the diff would
	/// otherwise misattribute their keystrokes (R3.3).
	pub fmt_baseline_version: Option<u64>,
	/// Timestamp of the last autosave run; gates the 5-second autosave cadence.
	pub last_autosave: std::time::Instant,
	/// Previous frame's screen buffer; used by the differential renderer.
	pub last_screen: Option<crate::render::buffer::ScreenBuffer>,
	/// Active UI theme.
	pub theme: std::sync::Arc<crate::ui::theme::Theme>,
	/// Active UI locale.
	pub locale: Box<dyn crate::ui::i18n::Locale>,
	/// Kind of the most recent edit; used to group consecutive same-kind
	/// edits into a single undo step.
	pub last_edit_action: crate::editor::commands::EditAction,
}

impl Editor {
	pub fn new() -> Self {
		let (tw, th) = terminal::size().unwrap_or((80, 24));
		let config = Config::load();
		
		// macOS limits the main-thread stack to 8 MB. In release builds,
		// syntect's syntax-set initialization can blow that and SIGKILL the
		// process. Spawn a dedicated 32 MB-stack thread and join it so the
		// expensive initialization happens off the main thread.
		let mode = terminal_colorsaurus::theme_mode(terminal_colorsaurus::QueryOptions::default()).unwrap_or(terminal_colorsaurus::ThemeMode::Dark);
		let is_light_bg = mode == terminal_colorsaurus::ThemeMode::Light;

		let mut theme = config.theme.clone();
		if theme == "default" {
			theme = if is_light_bg {
				"OneHalfLight".to_string()
			} else {
				"OneHalfDark".to_string()
			};
		}

		let mut highlighter = std::thread::Builder::new()
			.stack_size(32 * 1024 * 1024)
			.spawn(move || Highlighter::new(&theme))
			.expect("Failed to spawn syntect tokenizer thread")
			.join()
			.expect("Highlighter instantiation crashed");

		if config.comments_are_italics {
			use syntect::highlighting::{FontStyle, ScopeSelectors, StyleModifier, ThemeItem};
			use std::str::FromStr;

			if let Ok(scope) = ScopeSelectors::from_str("comment") {
				highlighter.theme.scopes.push(ThemeItem {
					scope,
					style: StyleModifier {
						foreground: None,
						background: None,
						font_style: Some(FontStyle::ITALIC),
					},
				});
			}
		}

		// Force markdown emphasis scopes to render with the corresponding font style,
		// regardless of whether the active theme defines font_style on these scopes.
		// Mirrors the comments_are_italics pattern above.
		{
			use syntect::highlighting::{FontStyle, ScopeSelectors, StyleModifier, ThemeItem};
			use std::str::FromStr;

			let md_scopes: &[(&str, FontStyle)] = &[
				("markup.bold", FontStyle::BOLD),
				("markup.italic", FontStyle::ITALIC),
				("markup.heading", FontStyle::BOLD),
				("markup.underline", FontStyle::UNDERLINE),
			];
			for (sel, fs) in md_scopes {
				if let Ok(scope) = ScopeSelectors::from_str(sel) {
					highlighter.theme.scopes.push(ThemeItem {
						scope,
						style: StyleModifier {
							foreground: None,
							background: None,
							font_style: Some(*fs),
						},
					});
				}
			}
		}

		Self {
			config,
			buffers: vec![Buffer::new()],
			active_buffer: 0,
			mode: Mode::Editing,
			cursors: CursorSet::new(),
			status_msg: None,
			should_quit: false,
			scroll_y: 0,
			scroll_vrow: 0,
			scroll_x: 0,
			sys_clipboard: arboard::Clipboard::new().ok(),
			internal_clipboard: String::new(),
			terminal_width: tw,
			terminal_height: th,
			suppress_next_paste: false,
			show_help: false,
			search_query: String::new(),
			replace_query: String::new(),
			replace_with: String::new(),
			search_matches: Vec::new(),
			search_match_idx: 0,
			search_saved_cursor: None,
			last_search_query: String::new(),
			highlighter,
			highlight_cache: std::cell::RefCell::new(crate::syntax::HighlightCache::new()),
			goto_line_input: String::new(),
			save_as_input: String::new(),
			prompt_cursor: 0,
			prompt_view_start: std::cell::Cell::new(0),
			save_as_pending_path: None,
			fmt_rx: None,
			is_formatting: false,
			fmt_baseline_version: None,
			last_autosave: std::time::Instant::now(),
			last_screen: None,
			theme: std::sync::Arc::new(crate::ui::theme::Theme::default(is_light_bg)),
			locale: Box::new(crate::ui::i18n::EnglishLocale),
			last_edit_action: crate::editor::commands::EditAction::Other,
		}
	}

	/// Drive periodic background work: writes the autosave swap-file when
	/// the buffer has been dirty for ≥5s, and applies any in-flight
	/// formatter result that's now ready. Returns true if any work was
	/// performed (signalling the caller to re-render).
	pub fn poll_async_tasks(&mut self) -> bool {
		let mut did_work = false;

		let now = std::time::Instant::now();
		if self.buffer().dirty && now.duration_since(self.last_autosave).as_secs() >= 5 {
			if let Some(ref swp) = self.buffer().swp_path {
				let text_clone = self.buffer().text.clone(); // O(1) Arc shallow clone
				let p = swp.clone();
				std::thread::spawn(move || {
					let content = text_clone.to_string_full();
					crate::recovery::write_swap_atomic(&p, &content);
				});
			}
			self.last_autosave = now;
		}

		if let Some(rx) = &self.fmt_rx {
			if let Ok(res) = rx.try_recv() {
				self.is_formatting = false;
				self.fmt_rx = None;
				let baseline = self.fmt_baseline_version.take();
				let buffer_changed = baseline
					.map(|v| v != self.buffer().version)
					.unwrap_or(false);
				match res {
					Ok(_) if buffer_changed => {
						self.set_status("Formatter result discarded — buffer changed during format");
					}
					Ok(formatted_text) => {
						let content = self.buffer().text.to_string_full();
						let content_chars: Vec<char> = content.chars().collect();
						let formatted_chars: Vec<char> = formatted_text.chars().collect();

						let mut prefix = 0;
						while prefix < content_chars.len()
							&& prefix < formatted_chars.len()
							&& content_chars[prefix] == formatted_chars[prefix]
						{
							prefix += 1;
						}

						let mut suffix = 0;
						while suffix < content_chars.len() - prefix
							&& suffix < formatted_chars.len() - prefix
							&& content_chars[content_chars.len() - 1 - suffix]
								== formatted_chars[formatted_chars.len() - 1 - suffix]
						{
							suffix += 1;
						}

						let end_char = content_chars.len() - suffix;
						if prefix < end_char || prefix < formatted_chars.len() - suffix {
							let insert_text: String = formatted_chars
								[prefix..formatted_chars.len() - suffix]
								.iter()
								.collect();
							self.buffer_mut().delete_range(prefix, end_char);
							self.buffer_mut().insert_str(prefix, &insert_text);
							self.buffer_mut().commit_edits();
							self.set_status("File formatted successfully");
						} else {
							self.set_status("File is already formatted");
						}
					}
					Err(e) => {
						self.set_status(&e);
					}
				}
				did_work = true;
			}
		}
		did_work
	}

	/// Open a file into a new buffer and switch to it.
	pub fn open_file(&mut self, path: &std::path::Path) -> std::io::Result<()> {
		let (mut buffer, sniffed_expand_tab, sniffed_tab_width) = Buffer::from_file(path)?;

		self.config.apply_editorconfig(path);
		if let Some(et) = sniffed_expand_tab {
			self.config.expand_tab = et;
		}
		if let Some(tw) = sniffed_tab_width {
			self.config.tab_width = tw;
		}

		let swp_path = crate::recovery::get_swap_path(path);

		if crate::recovery::check_recovery(&swp_path).is_some() {
			self.mode = Mode::RecoverSwap;
		}
		buffer.swp_path = Some(swp_path);

		self.buffers.push(buffer);
		self.active_buffer = self.buffers.len() - 1;
		self.cursors = CursorSet::new();
		self.scroll_y = 0;
		self.scroll_vrow = 0;
		Ok(())
	}

	/// Get a reference to the active buffer.
	pub fn buffer(&self) -> &Buffer {
		&self.buffers[self.active_buffer]
	}

	/// Get a mutable reference to the active buffer.
	pub fn buffer_mut(&mut self) -> &mut Buffer {
		&mut self.buffers[self.active_buffer]
	}

	/// The configured tab display width (columns).
	pub fn tab_width(&self) -> usize {
		self.config.tab_width
	}

	/// Whether Tab inserts spaces (true) or a literal tab (false).
	pub fn expand_tab(&self) -> bool {
		self.config.expand_tab
	}

	/// Set a status message.
	pub fn set_status(&mut self, msg: impl Into<String>) {
		self.status_msg = Some(msg.into());
	}

	/// Clear the status message.
	pub fn clear_status(&mut self) {
		self.status_msg = None;
	}

	/// Toggle comments for the selected lines (or current line) using syntax-aware prefixes.
	pub fn toggle_comment(&mut self) {
		let syntax = self
			.highlighter
			.detect_syntax(self.buffer().file_path.as_deref());
		let prefix = match syntax.name.as_str() {
			"Python"
			| "Ruby"
			| "Shell-Unix-Generic"
			| "Bourne Again Shell (bash)"
			| "YAML"
			| "TOML"
			| "Makefile"
			| "Perl"
			| "PowerShell"
			| "R"
			| "Elixir" => "#",
			"Lua" | "SQL" | "Haskell" | "Ada" | "AppleScript" => "--",
			"HTML" | "XML" | "Markdown" => "<!--", // Note: HTML usually requires `-->` block suffix, simplistic fallback used
			"CSS" => "/*",                         // simplistic fallback used
			_ => "//",                             // Rust, C, C++, JS, TS, Java, Go, Swift, PHP, D, etc.
		};

		let (start_line, end_line) = if self.has_selection() {
			let (start_c, end_c) = self.cursors.primary().ordered();
			(start_c.line, end_c.line)
		} else {
			let l = self.cursors.cursor().line;
			(l, l)
		};

		// Toggle is uncomment-if-all-already-commented, else comment-all.
		let mut all_commented = true;
		for line_idx in start_line..=end_line {
			let line_text: String = self.buffer().text.line_slice(line_idx).chars().collect();
			if line_text.trim_end().is_empty() {
				continue;
			}
			if !line_text.trim_start().starts_with(prefix) {
				all_commented = false;
				break;
			}
		}

		// Iterate bottom-up so each insert/remove leaves earlier line offsets intact.
		for line_idx in (start_line..=end_line).rev() {
			let line_text: String = self.buffer().text.line_slice(line_idx).chars().collect();
			let stripped = line_text.trim_start();

			if stripped.is_empty() {
				continue;
			}

			let indent_len = line_text.chars().count() - stripped.chars().count();
			let insert_pos = self.buffer().text.line_to_char(line_idx) + indent_len;

			if all_commented {
				// Also consume the space we inserted alongside the prefix, if present.
				let to_remove = if stripped.starts_with(&format!("{} ", prefix)) {
					prefix.chars().count() + 1
				} else {
					prefix.chars().count()
				};
				self.buffer_mut()
					.delete_range(insert_pos, insert_pos + to_remove);
			} else {
				self.buffer_mut()
					.insert_str(insert_pos, &format!("{} ", prefix));
			}
		}

		self.buffer_mut().commit_edits();
	}


	/// Update stored terminal dimensions (called on resize events).
	pub fn handle_resize(&mut self, width: u16, height: u16) {
		self.terminal_width = width;
		self.terminal_height = height;
	}
}

impl Default for Editor {
	fn default() -> Self {
		Self::new()
	}
}

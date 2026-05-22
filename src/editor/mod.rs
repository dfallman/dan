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
use crate::editor::mode::Mode;
use crate::syntax::Highlighter;

use crossterm::terminal;
use std::io;

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
	/// Status message displayed in the status bar.
	pub status_msg: Option<String>,
	/// Whether the editor should quit.
	pub should_quit: bool,
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
	/// None = not in a quit cycle; Some(i) = currently prompting buffer i.
	pub quit_cycle_idx: Option<usize>,
	/// Command palette state (query, items, filter, selection).
	pub palette: crate::palette::PaletteState,
	/// MRU recent-files list, capped at 50; persisted to disk (debounced).
	pub recent_files: std::collections::VecDeque<crate::palette::index::RecentFile>,
	/// Project root (lazily detected on first palette open).
	pub project_root: std::path::PathBuf,
	/// Background-walked project file index (drained from `project_index_rx`).
	pub project_index: Vec<std::path::PathBuf>,
	/// Receiver for the in-flight project index walker; None when idle.
	pub project_index_rx: Option<std::sync::mpsc::Receiver<std::path::PathBuf>>,
	/// Set whenever `recent_files` changes; cleared after a debounced save.
	pub recent_files_dirty: bool,
	/// Timestamp of the last recent-files write; gates the 5-second cadence.
	pub last_recent_save: std::time::Instant,
	/// Monotonic sequence assigned to each new untitled buffer. The startup
	/// scratch is seq 1 (renders as `[Untitled]`); subsequent NewBuffer
	/// invocations get 2, 3, 4… (rendered as `[Untitled 2]`, etc).
	pub next_untitled_seq: usize,
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

		// Startup scratch: untitled_seq = 1 (renders as plain "[Untitled]").
		let mut scratch = Buffer::new();
		scratch.untitled_seq = Some(1);

		Self {
			config,
			buffers: vec![scratch],
			active_buffer: 0,
			mode: Mode::Editing,
			status_msg: None,
			should_quit: false,
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
			highlighter,
			highlight_cache: std::cell::RefCell::new(crate::syntax::HighlightCache::new()),
			goto_line_input: String::new(),
			save_as_input: String::new(),
			prompt_cursor: 0,
			prompt_view_start: std::cell::Cell::new(0),
			save_as_pending_path: None,
			last_autosave: std::time::Instant::now(),
			last_screen: None,
			theme: std::sync::Arc::new(crate::ui::theme::Theme::default(is_light_bg)),
			locale: Box::new(crate::ui::i18n::EnglishLocale),
			last_edit_action: crate::editor::commands::EditAction::Other,
			quit_cycle_idx: None,
			palette: crate::palette::PaletteState::new(),
			recent_files: crate::palette::index::load_recent_files().into_iter().collect(),
			project_root: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
			project_index: Vec::new(),
			project_index_rx: None,
			recent_files_dirty: false,
			last_recent_save: std::time::Instant::now(),
			next_untitled_seq: 2,
		}
	}

	/// Create a fresh unpathed buffer with the next monotonic untitled_seq,
	/// push it onto `buffers`, and make it the active buffer. Used by
	/// Command::NewBuffer (and Ctrl-N).
	pub fn push_new_untitled(&mut self) {
		let mut buf = Buffer::new();
		buf.untitled_seq = Some(self.next_untitled_seq);
		self.next_untitled_seq += 1;
		self.buffers.push(buf);
		self.active_buffer = self.buffers.len() - 1;
	}

	/// True if the buffer at `idx` is the auto-created startup scratch in its
	/// pristine state (untitled_seq = 1, no path, no edits, clean). Such a
	/// buffer is "auto-disposable": when the user takes any explicit action
	/// (open file, create another buffer), we silently drop it so the user
	/// never has a stray [Untitled] they didn't ask for.
	fn is_disposable_startup_scratch(&self, idx: usize) -> bool {
		self.buffers.get(idx)
			.map(|b| {
				b.untitled_seq == Some(1)
					&& b.file_path.is_none()
					&& !b.dirty
					&& b.text.len_chars() == 0
			})
			.unwrap_or(false)
	}

	/// If the startup scratch is still pristine at index 0, drop it and
	/// adjust active_buffer. Caller must immediately push or switch to
	/// another buffer (we don't synthesize a fallback here).
	///
	/// Also rewinds `next_untitled_seq` back to 1 when no untitled buffers
	/// remain — otherwise `dan some_file.rs` then `Ctrl-N` would surprise
	/// the user with `[Untitled 2]` (seq 1 was claimed by the disposed
	/// startup scratch).
	fn maybe_dispose_startup_scratch(&mut self) {
		if self.is_disposable_startup_scratch(0) {
			self.buffers.remove(0);
			if self.active_buffer > 0 {
				self.active_buffer -= 1;
			}
			if !self.buffers.iter().any(|b| b.untitled_seq.is_some()) {
				self.next_untitled_seq = 1;
			}
		}
	}

	/// Drive periodic background work: writes the autosave swap-file for any
	/// dirty buffer when ≥5s have elapsed, and applies any in-flight
	/// formatter result (across all buffers) that's now ready. Returns true
	/// if any work was performed (signalling the caller to re-render).
	pub fn poll_async_tasks(&mut self) -> bool {
		let mut did_work = false;

		// Drain the project-index walker channel. When the walker thread
		// completes (sender dropped → Disconnected), drop the receiver so the
		// "indexing…" footer indicator clears and the main loop returns to its
		// idle 500ms poll cadence (otherwise `has_pending_async` stays true
		// forever and we tight-poll at 25ms).
		if self.project_index_rx.is_some() {
			let mut disconnected = false;
			loop {
				match self.project_index_rx.as_ref().unwrap().try_recv() {
					Ok(p) => {
						self.project_index.push(p);
						did_work = true;
					}
					Err(std::sync::mpsc::TryRecvError::Empty) => break,
					Err(std::sync::mpsc::TryRecvError::Disconnected) => {
						disconnected = true;
						did_work = true;
						break;
					}
				}
			}
			if disconnected {
				self.project_index_rx = None;
			}
		}

		// Debounced recent-files persistence (5s after last change).
		if self.recent_files_dirty
			&& std::time::Instant::now().duration_since(self.last_recent_save).as_secs() >= 5
		{
			let snap: Vec<_> = self.recent_files.iter().cloned().collect();
			std::thread::spawn(move || crate::palette::index::save_recent_files(&snap));
			self.recent_files_dirty = false;
			self.last_recent_save = std::time::Instant::now();
		}

		let now = std::time::Instant::now();
		if now.duration_since(self.last_autosave).as_secs() >= 5 {
			for buf in &self.buffers {
				if buf.dirty {
					if let Some(ref swp) = buf.swp_path {
						let text_clone = buf.text.clone(); // O(1) Arc shallow clone
						let p = swp.clone();
						std::thread::spawn(move || {
							let content = text_clone.to_string_full();
							crate::recovery::write_swap_atomic(&p, &content);
						});
					}
				}
			}
			self.last_autosave = now;
		}

		// Poll formatter receivers on every buffer — a format job may belong
		// to a buffer that isn't currently active (the user may have switched
		// away while formatting). Apply each result to the buffer it came
		// from via indexed access, NOT through buffer_mut().
		let n = self.buffers.len();
		for i in 0..n {
			let fmt_result = self.buffers[i]
				.fmt_rx
				.as_ref()
				.and_then(|rx| rx.try_recv().ok());
			let Some(res) = fmt_result else { continue };

			self.buffers[i].is_formatting = false;
			self.buffers[i].fmt_rx = None;
			let baseline = self.buffers[i].fmt_baseline_version.take();
			let buffer_changed = baseline
				.map(|v| v != self.buffers[i].version)
				.unwrap_or(false);

			match res {
				Ok(_) if buffer_changed => {
					self.set_status("Formatter result discarded — buffer changed during format");
				}
				Ok(formatted_text) => {
					let content = self.buffers[i].text.to_string_full();
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
					while suffix < content_chars.len().saturating_sub(prefix)
						&& suffix < formatted_chars.len().saturating_sub(prefix)
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
						self.buffers[i].delete_range(prefix, end_char);
						self.buffers[i].insert_str(prefix, &insert_text);
						self.buffers[i].commit_edits();
						// A reformat can return fewer lines than the buffer had,
						// leaving the cursor past the new end. Re-clamp so the
						// next render's line lookups stay in bounds.
						self.buffers[i].clamp_cursors();
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
		did_work
	}

	/// Open a file into a new buffer and switch to it.
	///
	/// If a buffer with the same path is already open, just switches to it
	/// rather than loading a duplicate copy.
	pub fn open_file(&mut self, path: &std::path::Path) -> std::io::Result<()> {
		// Already open? Just switch.
		if let Some(idx) = self
			.buffers
			.iter()
			.position(|b| b.file_path.as_deref() == Some(path))
		{
			self.active_buffer = idx;
			return Ok(());
		}

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

		// User has explicitly chosen to open a file — the auto-created startup
		// scratch (if it's still pristine) has served its placeholder duty.
		self.maybe_dispose_startup_scratch();
		self.buffers.push(buffer);
		self.active_buffer = self.buffers.len() - 1;
		self.push_recent_file(path);
		Ok(())
	}

	/// Record that `path` was opened just now. Promotes it to the front of the
	/// MRU list (deduping any prior entry), caps at 50, and marks the recent
	/// list dirty so `poll_async_tasks` will persist it.
	pub fn push_recent_file(&mut self, path: &std::path::Path) {
		use std::time::{SystemTime, UNIX_EPOCH};
		let unix = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.map(|d| d.as_secs())
			.unwrap_or(0);
		self.recent_files.retain(|r| r.path != path);
		self.recent_files.push_front(crate::palette::index::RecentFile {
			path: path.to_path_buf(),
			last_opened_unix: unix,
		});
		while self.recent_files.len() > 50 {
			self.recent_files.pop_back();
		}
		self.recent_files_dirty = true;
	}

	/// Build the palette item list from current state and open the modal.
	///
	/// Items, in order: open buffers (current first), recent files (excluding
	/// already-open), project-index files (excluding open + recent), then
	/// every action from the static registry. Lazily detects the project root
	/// and spawns the index walker the first time the palette is opened.
	pub fn open_palette(&mut self) {
		use crate::palette::PaletteItem;

		// Lazy: detect project root and start the indexer on first open.
		if self.project_index.is_empty() && self.project_index_rx.is_none() {
			let start = self.buffer().file_path.as_deref().and_then(|p| p.parent())
				.map(|p| p.to_path_buf())
				.unwrap_or_else(|| self.project_root.clone());
			self.project_root = crate::palette::index::detect_project_root(&start);
			let (tx, rx) = std::sync::mpsc::channel();
			crate::palette::index::spawn_index_walker(self.project_root.clone(), tx);
			self.project_index_rx = Some(rx);
		}

		let mut items: Vec<PaletteItem> = Vec::new();

		// 1. Open buffers (current first, then ascending order).
		let active = self.active_buffer;
		let mut order: Vec<usize> = (0..self.buffers.len()).collect();
		order.sort_by_key(|&i| if i == active { 0 } else { i + 1 });
		for &idx in &order {
			let b = &self.buffers[idx];
			let path_display = b
				.file_path
				.as_ref()
				.map(|p| p.display().to_string())
				.unwrap_or_else(|| b.display_name());
			items.push(PaletteItem::Buffer {
				idx,
				dirty: b.dirty,
				path_display,
				is_current: idx == active,
			});
		}

		// 1b. "New buffer" — sits at the bottom of the buffer section, above
		// the divider, so it's always one keystroke away even when buffers
		// haven't matched the query.
		items.push(PaletteItem::Action {
			id: crate::palette::ActionId::NewBuffer,
			label: "New buffer".to_string(),
			hint: Some("⌃N".to_string()),
		});

		// 2. Recent files (excluding currently-open).
		let open_paths: std::collections::HashSet<_> = self
			.buffers
			.iter()
			.filter_map(|b| b.file_path.clone())
			.collect();
		for r in &self.recent_files {
			if open_paths.contains(&r.path) { continue; }
			items.push(PaletteItem::File {
				path: r.path.clone(),
				display: r.path.display().to_string(),
				last_opened: Some(r.last_opened()),
			});
		}

		// 3. Project index files (excluding open + recent).
		let recent_paths: std::collections::HashSet<_> = self
			.recent_files
			.iter()
			.map(|r| r.path.clone())
			.collect();
		for p in &self.project_index {
			if open_paths.contains(p) || recent_paths.contains(p) { continue; }
			items.push(PaletteItem::File {
				path: p.clone(),
				display: p.strip_prefix(&self.project_root).unwrap_or(p).display().to_string(),
				last_opened: None,
			});
		}

		// 4. Actions (excluding NewBuffer — already injected above).
		items.extend(crate::palette::action_registry().into_iter().filter(|i| {
			!matches!(
				i,
				PaletteItem::Action { id: crate::palette::ActionId::NewBuffer, .. }
			)
		}));

		self.palette.open_with(items);
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

	/// After saving or force-discarding one dirty buffer, find the next dirty
	/// buffer and either prompt for it or exit if none remain.
	fn advance_quit_cycle(&mut self) {
		let next = self.buffers.iter().position(|b| b.dirty);
		match next {
			None => {
				self.quit_cycle_idx = None;
				self.should_quit = true;
			}
			Some(i) => {
				self.active_buffer = i;
				self.quit_cycle_idx = Some(i);
				self.mode = crate::editor::mode::Mode::ConfirmQuit;
			}
		}
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
			let (start_c, end_c) = self.buffer().cursors.primary().ordered();
			(start_c.line, end_c.line)
		} else {
			let l = self.buffer().cursors.cursor().line;
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

	/// Remove buffer at `idx`. If the closed buffer was active, picks the
	/// previous neighbour (or 0 if there is no previous). If the last buffer
	/// is closed, replaces with a fresh [Untitled] buffer.
	///
	/// Caller is responsible for confirming any unsaved changes — this method
	/// drops the buffer unconditionally.
	#[allow(dead_code)]
	pub fn close_buffer(&mut self, idx: usize) -> io::Result<()> {
		if idx >= self.buffers.len() {
			return Err(io::Error::new(io::ErrorKind::NotFound, "buffer index out of range"));
		}
		self.buffers.remove(idx);

		if self.buffers.is_empty() {
			let mut scratch = Buffer::new();
			scratch.untitled_seq = Some(1);
			self.buffers.push(scratch);
			self.active_buffer = 0;
			return Ok(());
		}

		if idx <= self.active_buffer && self.active_buffer > 0 {
			self.active_buffer -= 1;
		}
		if self.active_buffer >= self.buffers.len() {
			self.active_buffer = self.buffers.len() - 1;
		}
		Ok(())
	}
}

impl Default for Editor {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn close_buffer_picks_previous_neighbour() {
		let mut e = Editor::new();
		e.buffers.push(Buffer::new());
		e.buffers.push(Buffer::new());
		e.active_buffer = 1;
		e.close_buffer(1).expect("close");
		assert_eq!(e.buffers.len(), 2);
		assert_eq!(e.active_buffer, 0);
	}

	#[test]
	fn close_last_buffer_creates_scratch() {
		let mut e = Editor::new();
		assert_eq!(e.buffers.len(), 1);
		e.close_buffer(0).expect("close");
		assert_eq!(e.buffers.len(), 1);
		assert_eq!(e.active_buffer, 0);
		assert_eq!(e.buffer().display_name(), "[Untitled]");
	}

	#[test]
	fn close_buffer_clamps_active_index() {
		let mut e = Editor::new();
		e.buffers.push(Buffer::new());
		e.buffers.push(Buffer::new());
		e.active_buffer = 2;
		e.close_buffer(0).expect("close");
		assert_eq!(e.buffers.len(), 2);
		assert_eq!(e.active_buffer, 1);
	}

	#[test]
	fn formatter_result_with_fewer_lines_clamps_cursor() {
		// Regression: an async formatter that returns fewer lines than the
		// buffer had used to leave the cursor pointing past the new end of the
		// document. The next render then called `line_slice(stale_line)` and
		// panicked ("Attempt to index past end of Rope"). Applying the result
		// must re-clamp the buffer's cursors.
		use std::sync::mpsc;

		let mut e = Editor::new();
		{
			let buf = e.buffer_mut();
			buf.text = crate::buffer::rope::TextRope::from_str("a\nb\nc\nd");
			buf.cursors.set_cursor(3, 1); // on the last line ("d")
		}

		// Simulate an in-flight format whose result shrinks the document.
		let (tx, rx) = mpsc::channel();
		tx.send(Ok("x\n".to_string())).expect("send"); // 2 lines: "x\n", ""
		{
			let buf = e.buffer_mut();
			buf.fmt_baseline_version = Some(buf.version); // unchanged -> applied
			buf.fmt_rx = Some(rx);
			buf.is_formatting = true;
		}

		let did_work = e.poll_async_tasks();
		assert!(did_work, "formatter result should have been applied");

		let line_count = e.buffer().line_count();
		let cursor_line = e.buffer().cursors.cursor().line;
		assert!(
			cursor_line < line_count,
			"cursor line {} must stay within line_count {}",
			cursor_line,
			line_count
		);
	}
}

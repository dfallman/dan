pub mod commands;
pub mod cursor;
mod dispatch;
mod editing;
pub mod formatter;
pub mod mode;
pub(crate) mod mouse;
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

/// Cap on the background-walked project file index. A huge monorepo would
/// otherwise grow `project_index` without bound and never free it (P3-G). Once
/// reached, draining stops and the walker's receiver is dropped (which makes the
/// walker thread exit on its next `send`).
const MAX_PROJECT_INDEX: usize = 50_000;

/// Cap on project-index paths drained per `poll_async_tasks` tick. Prevents a
/// large-repo walk from triggering a full re-render for every single file in
/// one tight-poll iteration.
const INDEX_DRAIN_BATCH: usize = 50;

/// Transient chrome notice when indent sniffing overrides config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoBanner {
	pub expand_tab: bool,
	pub tab_width: usize,
	/// When true, do not paint (e.g. while `RecoverSwap` is active).
	pub pending: bool,
}

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
	/// Sniff-override INFO bar (above help/toolbar); cleared on next key.
	pub info_banner: Option<InfoBanner>,
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
	/// True when the current `search_query` is `/pattern/` regex mode.
	pub search_is_regex: bool,
	/// Compiled pattern for the current regex query; cleared when the query changes.
	pub(crate) cached_regex: Option<regex::Regex>,
	/// True when regex mode failed to compile; chrome shows "invalid regex".
	pub search_regex_error: bool,
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
	/// When true, render skips scroll-to-cursor so a wheel / Ctrl+↑↓ pan
	/// can leave the selection head off-screen. Cleared by any non-pan
	/// command (including Shift+arrow selection, which must follow the head).
	pub pin_viewport: bool,
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
	/// True when the startup terminal colour query (OSC 10/11, via
	/// terminal-colorsaurus) failed or was deemed unsupported. Some terminals
	/// answer the query's DA1 sentinel *before* the colour replies, so
	/// colorsaurus bails out and the late colour replies leak into stdin —
	/// main.rs drains that stray input before the first frame when this is set.
	pub color_query_failed: bool,
}

/// The system clipboard — never opened under `cfg(test)`.
///
/// `cmd_paste` prefers the system clipboard and only falls back to the internal
/// one when it is empty. A test that seeds `internal_clipboard` would therefore
/// paste whatever the developer happened to have copied, so the suite passed or
/// failed on the state of the machine's pasteboard rather than on the code.
fn system_clipboard() -> Option<arboard::Clipboard> {
	#[cfg(test)]
	{
		None
	}
	#[cfg(not(test))]
	{
		arboard::Clipboard::new().ok()
	}
}

impl Editor {
	pub fn new() -> Self {
		let (tw, th) = terminal::size().unwrap_or((80, 24));
		let config = Config::load();
		
		// macOS limits the main-thread stack to 8 MB. In release builds,
		// syntect's syntax-set initialization can blow that and SIGKILL the
		// process. Spawn a dedicated 32 MB-stack thread and join it so the
		// expensive initialization happens off the main thread.
		let mode_result = terminal_colorsaurus::theme_mode(terminal_colorsaurus::QueryOptions::default());
		// A failed/unsupported query means the terminal may still send its colour
		// replies late (see `color_query_failed`); record it so main.rs can flush
		// the leftover bytes before they get parsed as typed input.
		let color_query_failed = mode_result.is_err();
		let mode = mode_result.unwrap_or(terminal_colorsaurus::ThemeMode::Dark);
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
		// Give it a swap path so autosave / the panic hook cover unsaved work
		// even in a never-saved buffer (P0-1).
		scratch.swp_path = Some(crate::recovery::untitled_swap_path(1));

		Self {
			config,
			buffers: vec![scratch],
			active_buffer: 0,
			mode: Mode::Editing,
			status_msg: None,
			info_banner: None,
			should_quit: false,
			scroll_x: 0,
			sys_clipboard: system_clipboard(),
			internal_clipboard: String::new(),
			terminal_width: tw,
			terminal_height: th,
			suppress_next_paste: false,
			show_help: false,
			search_query: String::new(),
			search_is_regex: false,
			cached_regex: None,
			search_regex_error: false,
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
			pin_viewport: false,
			quit_cycle_idx: None,
			palette: crate::palette::PaletteState::new(),
			recent_files: crate::palette::index::load_recent_files().into_iter().collect(),
			project_root: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
			project_index: Vec::new(),
			project_index_rx: None,
			recent_files_dirty: false,
			last_recent_save: std::time::Instant::now(),
			next_untitled_seq: 2,
			color_query_failed,
		}
	}

	/// Create a fresh unpathed buffer with the next monotonic untitled_seq,
	/// push it onto `buffers`, and make it the active buffer. Used by
	/// Command::NewBuffer (and Ctrl-N).
	pub fn push_new_untitled(&mut self) {
		let mut buf = Buffer::new();
		buf.untitled_seq = Some(self.next_untitled_seq);
		// Swap path so autosave / the panic hook cover this buffer (P0-1).
		buf.swp_path = Some(crate::recovery::untitled_swap_path(self.next_untitled_seq));
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
			let mut drop_rx = false;
			let mut index_added = false;
			let mut drained = 0usize;
			loop {
				if drained >= INDEX_DRAIN_BATCH {
					break;
				}
				if self.project_index.len() >= MAX_PROJECT_INDEX {
					// Cap reached: stop draining and drop the receiver so the
					// walker thread exits on its next send (P3-G).
					drop_rx = true;
					did_work = true;
					break;
				}
				match self.project_index_rx.as_ref().unwrap().try_recv() {
					Ok(p) => {
						self.project_index.push(p);
						did_work = true;
						index_added = true;
						drained += 1;
					}
					Err(std::sync::mpsc::TryRecvError::Empty) => break,
					Err(std::sync::mpsc::TryRecvError::Disconnected) => {
						drop_rx = true;
						did_work = true;
						break;
					}
				}
			}
			if drop_rx {
				self.project_index_rx = None;
			}
			if index_added && self.mode == Mode::Palette && self.palette.open {
				self.sync_palette_items();
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
			// Self-heal: is_formatting without a receiver would tight-poll forever.
			if self.buffers[i].is_formatting && self.buffers[i].fmt_rx.is_none() {
				self.buffers[i].is_formatting = false;
				self.buffers[i].fmt_baseline_version = None;
				self.buffers[i].fmt_child_pid = None;
				did_work = true;
				continue;
			}

			let recv = self.buffers[i].fmt_rx.as_ref().map(|rx| rx.try_recv());
			let res = match recv {
				None => continue,                                            // no format in flight
				Some(Err(std::sync::mpsc::TryRecvError::Empty)) => continue, // still running
				Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
					// Worker vanished without sending (panic / kill / the stdin
					// deadlock). Clear the in-flight flags so we don't tight-poll
					// at 25 ms forever (P2-E).
					self.buffers[i].is_formatting = false;
					self.buffers[i].fmt_rx = None;
					self.buffers[i].fmt_baseline_version = None;
					self.buffers[i].fmt_child_pid = None;
					did_work = true;
					continue;
				}
				Some(Ok(res)) => res,
			};

			self.buffers[i].is_formatting = false;
			self.buffers[i].fmt_rx = None;
			self.buffers[i].fmt_child_pid = None;
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

		let before_expand = self.config.expand_tab;
		let before_width = self.config.tab_width;

		if let Some(et) = sniffed_expand_tab {
			self.config.expand_tab = et;
		}
		if let Some(tw) = sniffed_tab_width {
			self.config.tab_width = tw;
		}

		let changed = self.config.expand_tab != before_expand
			|| self.config.tab_width != before_width;

		let swp_path = crate::recovery::get_swap_path(path);
		let recovering = crate::recovery::check_recovery(&swp_path).is_some();

		if recovering {
			self.mode = Mode::RecoverSwap;
		}
		buffer.swp_path = Some(swp_path);

		if changed {
			self.set_info_banner(
				self.config.expand_tab,
				self.config.tab_width,
				recovering,
			);
		}

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
	/// every action from the static registry. The project indexer is started
	/// lazily when the user types a query (see `ensure_project_indexer_started`).
	pub fn open_palette(&mut self) {
		let items = self.build_palette_items();
		self.palette.open_with(items);
	}

	/// Start the background project-file walker on first palette search.
	pub fn ensure_project_indexer_started(&mut self) {
		if !self.project_index.is_empty() || self.project_index_rx.is_some() {
			return;
		}
		let start = self
			.buffer()
			.file_path
			.as_deref()
			.and_then(|p| p.parent())
			.map(|p| p.to_path_buf())
			.unwrap_or_else(|| self.project_root.clone());
		self.project_root = crate::palette::index::detect_project_root(&start);
		let (tx, rx) = std::sync::mpsc::channel();
		crate::palette::index::spawn_index_walker(self.project_root.clone(), tx);
		self.project_index_rx = Some(rx);
	}

	/// Rebuild palette items while preserving the current query and cursor.
	pub fn sync_palette_items(&mut self) {
		if !self.palette.open {
			return;
		}
		let query = self.palette.query.clone();
		let query_cursor = self.palette.query_cursor;
		let items = self.build_palette_items();
		self.palette.open_with(items);
		self.palette.query = query;
		self.palette.query_cursor = query_cursor;
		self.palette.refilter();
	}

	fn build_palette_items(&self) -> Vec<crate::palette::PaletteItem> {
		use crate::palette::PaletteItem;

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

		items
	}

	/// Kill in-flight formatter children, drop async receivers, and stop the
	/// project indexer so shutdown doesn't leave orphaned processes or a
	/// tight-poll loop during the final unwind.
	pub fn shutdown_async_work(&mut self) {
		for buf in &mut self.buffers {
			if let Some(pid_slot) = buf.fmt_child_pid.take() {
				let pid = pid_slot.load(std::sync::atomic::Ordering::Relaxed);
				crate::editor::formatter::terminate_child(pid);
			}
			buf.fmt_rx = None;
			buf.is_formatting = false;
			buf.fmt_baseline_version = None;
		}
		self.project_index_rx = None;
	}

	/// Get a reference to the active buffer.
	pub fn buffer(&self) -> &Buffer {
		&self.buffers[self.active_buffer]
	}

	/// Get a mutable reference to the active buffer.
	pub fn buffer_mut(&mut self) -> &mut Buffer {
		&mut self.buffers[self.active_buffer]
	}

	/// The configured tab display width (columns). Clamped to `>= 1`: a zero
	/// width (from `config.toml` `tab_width = 0` or `.editorconfig`
	/// `indent_size = 0`) would otherwise reach `col % tab_w` in the renderer
	/// and panic with a divide-by-zero (P1-A).
	pub fn tab_width(&self) -> usize {
		self.config.tab_width.max(1)
	}

	/// Whether Tab inserts spaces (true) or a literal tab (false).
	pub fn expand_tab(&self) -> bool {
		self.config.expand_tab
	}

	/// Publish O(1) snapshots of the currently-dirty buffers to the global
	/// crash registry so the panic hook can flush them if the process dies
	/// (P0-2). Called once per event-loop iteration; `TextRope::clone` is a
	/// structural-sharing clone, so this stays cheap.
	pub fn publish_crash_snapshot(&self) {
		let entries = self
			.buffers
			.iter()
			.filter(|b| b.dirty)
			.filter_map(|b| {
				b.swp_path.as_ref().map(|p| crate::crash::CrashEntry {
					swap_path: p.clone(),
					text: b.text.clone(),
				})
			})
			.collect();
		crate::crash::publish(entries);
	}

	/// Set a status message.
	pub fn set_status(&mut self, msg: impl Into<String>) {
		self.status_msg = Some(msg.into());
	}

	/// Clear the status message.
	pub fn clear_status(&mut self) {
		self.status_msg = None;
	}

	pub fn set_info_banner(&mut self, expand_tab: bool, tab_width: usize, pending: bool) {
		self.info_banner = Some(InfoBanner {
			expand_tab,
			tab_width,
			pending,
		});
	}

	pub fn clear_info_banner(&mut self) {
		self.info_banner = None;
	}

	pub fn promote_info_banner(&mut self) {
		if let Some(b) = self.info_banner.as_mut() {
			b.pending = false;
		}
	}

	pub fn info_banner_visible(&self) -> bool {
		matches!(self.info_banner, Some(InfoBanner { pending: false, .. }))
	}

	/// After saving or force-discarding one dirty buffer, find the next dirty
	/// buffer and either prompt for it or exit if none remain.
	pub(crate) fn advance_quit_cycle(&mut self) {
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

	/// Toggle comments for the selected lines (or current line) using syntax-aware
	/// delimiters. Line comments use a prefix only; block styles (HTML/XML/Markdown,
	/// CSS) wrap each non-empty line with prefix + suffix.
	pub fn toggle_comment(&mut self) {
		let syntax = self
			.highlighter
			.detect_syntax(self.buffer().file_path.as_deref());
		let (prefix, suffix): (&str, &str) = match syntax.name.as_str() {
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
			| "Elixir" => ("#", ""),
			"Lua" | "SQL" | "Haskell" | "Ada" | "AppleScript" => ("--", ""),
			"HTML" | "XML" | "Markdown" => ("<!-- ", " -->"),
			"CSS" => ("/* ", " */"),
			_ => ("//", ""), // Rust, C, C++, JS, TS, Java, Go, Swift, PHP, D, etc.
		};

		let (start_line, end_line) = if self.has_selection() {
			let (start_c, end_c) = self.buffer().cursors.primary().ordered();
			(start_c.line, end_c.line)
		} else {
			let l = self.buffer().cursors.cursor().line;
			(l, l)
		};

		let is_commented = |stripped: &str| -> bool {
			if !stripped.starts_with(prefix) {
				return false;
			}
			if suffix.is_empty() {
				return true;
			}
			stripped.trim_end().ends_with(suffix)
		};

		// Toggle is uncomment-if-all-already-commented, else comment-all.
		let mut all_commented = true;
		for line_idx in start_line..=end_line {
			let line_text: String = self.buffer().text.line_slice(line_idx).chars().collect();
			if line_text.trim_end().is_empty() {
				continue;
			}
			if !is_commented(line_text.trim_start()) {
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
			let line_start = self.buffer().text.line_to_char(line_idx);
			let insert_pos = line_start + indent_len;

			if all_commented {
				if suffix.is_empty() {
					// Line comment: also consume the space we inserted after the
					// prefix, if present (legacy `// foo` / `# foo` form).
					let to_remove = if stripped.starts_with(&format!("{} ", prefix)) {
						prefix.chars().count() + 1
					} else {
						prefix.chars().count()
					};
					self.buffer_mut()
						.delete_range(insert_pos, insert_pos + to_remove);
				} else {
					// Block comment: strip trailing suffix first (from end of
					// content, before newline), then the leading prefix.
					let content_end = {
						let trimmed = stripped.trim_end();
						let trail_ws = stripped.chars().count() - trimmed.chars().count();
						insert_pos + stripped.chars().count() - trail_ws
					};
					let suffix_len = suffix.chars().count();
					self.buffer_mut()
						.delete_range(content_end - suffix_len, content_end);
					self.buffer_mut()
						.delete_range(insert_pos, insert_pos + prefix.chars().count());
				}
			} else if suffix.is_empty() {
				self.buffer_mut()
					.insert_str(insert_pos, &format!("{} ", prefix));
			} else {
				// Wrap: prefix at indent, suffix after the last non-newline char.
				let content = stripped.trim_end_matches(['\n', '\r']);
				let content_end = insert_pos + content.chars().count();
				self.buffer_mut().insert_str(content_end, suffix);
				self.buffer_mut().insert_str(insert_pos, prefix);
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
		// Remove the buffer's swap file so a discarded/closed buffer doesn't
		// leave a stale crash-recovery candidate behind (P4-M). Harmless no-op
		// if it was already cleaned by a save.
		if let Some(ref swp) = self.buffers[idx].swp_path {
			crate::recovery::cleanup_swap(swp);
		}
		self.buffers.remove(idx);

		if self.buffers.is_empty() {
			let mut scratch = Buffer::new();
			scratch.untitled_seq = Some(1);
			scratch.swp_path = Some(crate::recovery::untitled_swap_path(1));
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
	fn sniff_override_queues_info_banner_spaces() {
		let mut e = Editor::new();
		e.config.expand_tab = false;
		e.config.tab_width = 4;

		let mut tmp = std::env::temp_dir();
		tmp.push(format!("dan_sniff_spaces_{}.txt", std::process::id()));
		// Majority space indents of width 8 -> sniff expand_tab=true, tab_width=8
		let body = "        a\n        b\n        c\n        d\n";
		std::fs::write(&tmp, body).unwrap();

		e.open_file(&tmp).unwrap();
		let _ = std::fs::remove_file(&tmp);

		assert!(e.config.expand_tab);
		assert_eq!(e.config.tab_width, 8);
		let b = e.info_banner.as_ref().expect("banner queued");
		assert!(b.expand_tab);
		assert_eq!(b.tab_width, 8);
		assert!(!b.pending); // no RecoverSwap
	}

	#[test]
	fn sniff_matching_config_does_not_queue_banner() {
		let mut e = Editor::new();
		e.config.expand_tab = true;
		e.config.tab_width = 4;

		let mut tmp = std::env::temp_dir();
		tmp.push(format!("dan_sniff_match_{}.txt", std::process::id()));
		let body = "    a\n    b\n    c\n    d\n";
		std::fs::write(&tmp, body).unwrap();

		e.open_file(&tmp).unwrap();
		let _ = std::fs::remove_file(&tmp);

		assert!(e.info_banner.is_none());
	}

	#[test]
	fn sniff_tabs_when_already_tabs_no_banner() {
		let mut e = Editor::new();
		e.config.expand_tab = false;
		e.config.tab_width = 4;

		let mut tmp = std::env::temp_dir();
		tmp.push(format!("dan_sniff_tabs_{}.txt", std::process::id()));
		let body = "\ta\n\tb\n\tc\n\td\n";
		std::fs::write(&tmp, body).unwrap();

		e.open_file(&tmp).unwrap();
		let _ = std::fs::remove_file(&tmp);

		assert!(!e.config.expand_tab);
		assert!(e.info_banner.is_none());
	}

	#[test]
	fn recover_swap_defers_then_promotes_info_banner() {
		let mut e = Editor::new();
		e.set_info_banner(true, 8, true);
		assert!(!e.info_banner_visible());
		e.promote_info_banner();
		assert!(e.info_banner_visible());
	}

	#[test]
	fn clear_info_banner_removes_it() {
		let mut e = Editor::new();
		e.set_info_banner(false, 4, false);
		e.clear_info_banner();
		assert!(e.info_banner.is_none());
	}

	#[test]
	fn tab_width_never_zero() {
		// P1-A: tab_width = 0 (config.toml or .editorconfig indent_size = 0)
		// caused a `% 0` divide-by-zero panic in the renderer. The accessor
		// must clamp to >= 1.
		let mut e = Editor::new();
		e.config.tab_width = 0;
		assert!(e.tab_width() >= 1, "tab_width must be clamped to >= 1");
	}

	#[test]
	fn convert_spaces_to_tabs_safe_when_tab_width_zero() {
		// P4-K: with tab_width = 0, `" ".repeat(0)` is "" and
		// `leading.replace("", "\t")` interleaves a tab before every char,
		// mangling indentation. The handler must use the clamped width.
		use crate::editor::commands::Command;
		use crate::buffer::rope::TextRope;
		let mut e = Editor::new();
		e.config.tab_width = 0;
		e.buffer_mut().text = TextRope::from_str("    code\n");
		e.execute(Command::ConvertSpacesToTabs);
		let out = e.buffer().text.to_string_full();
		// Clamped width 1 → leading spaces become tabs, nothing interleaved.
		assert!(!out.contains(' '), "indentation mangled with tab_width 0: {:?}", out);
	}

	#[test]
	fn save_as_prompt_handles_multibyte() {
		// P1-B: prompt_cursor is a char count; String::insert/remove take a
		// byte index. Typing a non-ASCII char then another char panicked.
		use crate::editor::commands::Command;
		let mut e = Editor::new();
		e.save_as_input.clear();
		e.prompt_cursor = 0;
		e.execute(Command::SaveAsInsertChar('é'));
		e.execute(Command::SaveAsInsertChar('x'));
		assert_eq!(e.save_as_input, "éx");
		assert_eq!(e.prompt_cursor, 2);
		e.execute(Command::SaveAsDeleteChar);
		assert_eq!(e.save_as_input, "é");
		assert_eq!(e.prompt_cursor, 1);
	}

	#[test]
	fn replace_with_prompt_handles_multibyte() {
		// P1-C: same char-vs-byte bug on the replacement field.
		use crate::editor::commands::Command;
		use crate::editor::mode::Mode;
		let mut e = Editor::new();
		e.mode = Mode::ReplacingWith;
		e.replace_with.clear();
		e.prompt_cursor = 0;
		e.execute(Command::ReplaceInsertChar('ß'));
		e.execute(Command::ReplaceInsertChar('y'));
		assert_eq!(e.replace_with, "ßy");
		e.execute(Command::ReplaceDeleteChar);
		assert_eq!(e.replace_with, "ß");
	}

	#[test]
	fn wrap_selection_multibyte_then_copy_no_panic() {
		// P1-D: auto-close wrap computed the selection end with byte length,
		// leaving the selection head past len_chars. The next slice_to_string
		// (here via Copy) then panicked on an out-of-range rope slice.
		use crate::editor::commands::Command;
		use crate::buffer::rope::TextRope;
		let mut e = Editor::new();
		e.config.auto_close = true;
		e.buffer_mut().text = TextRope::from_str("é");
		e.execute(Command::SelectAll);
		e.execute(Command::InsertChar('"'));
		assert_eq!(e.buffer().text.to_string_full(), "\"é\"");
		e.execute(Command::Copy); // must not panic
		assert_eq!(e.internal_clipboard, "\"é\"");
	}

	#[test]
	fn duplicate_multibyte_selection_cursor_in_bounds() {
		// P4-Q: byte length used as a char offset misplaced the cursor past
		// the duplicated text.
		use crate::editor::commands::Command;
		use crate::buffer::rope::TextRope;
		let mut e = Editor::new();
		e.buffer_mut().text = TextRope::from_str("é");
		e.execute(Command::SelectAll);
		e.execute(Command::DuplicateLineOrSelection);
		assert_eq!(e.buffer().text.to_string_full(), "éé");
		assert_eq!(e.buffer().cursors.cursor().col, 2, "cursor should sit after both chars");
	}

	#[test]
	fn untitled_buffers_have_swap_paths() {
		// P0-1: untitled buffers (startup scratch + Ctrl-N) previously had no
		// swp_path, so the 5s autosave silently skipped them and a crash lost
		// 100% of unsaved work. Every buffer must carry a swap path.
		let mut e = Editor::new();
		assert!(e.buffer().swp_path.is_some(), "startup scratch needs a swap path");
		e.push_new_untitled();
		assert!(e.buffer().swp_path.is_some(), "new untitled buffer needs a swap path");
	}

	#[test]
	fn formatter_disconnect_clears_is_formatting() {
		// P2-E: if the formatter thread vanishes without sending (panic / OOM /
		// the deadlock case), try_recv returns Disconnected. The old code's
		// `.ok()` swallowed it, leaving is_formatting = true forever → permanent
		// 25 ms tight-poll. Disconnect must clear the in-flight flags.
		use std::sync::mpsc;
		let mut e = Editor::new();
		let (tx, rx) = mpsc::channel::<Result<String, String>>();
		drop(tx); // disconnected, no message ever sent
		{
			let buf = e.buffer_mut();
			buf.fmt_rx = Some(rx);
			buf.is_formatting = true;
			buf.fmt_baseline_version = Some(buf.version);
		}
		e.poll_async_tasks();
		assert!(!e.buffer().is_formatting, "is_formatting must clear on Disconnected");
		assert!(e.buffer().fmt_rx.is_none(), "fmt_rx must be dropped on Disconnected");
	}

	#[test]
	fn format_document_ignored_while_already_formatting() {
		// P3-J: repeated Ctrl-F must not spawn overlapping formatter workers.
		use crate::editor::commands::Command;
		let mut e = Editor::new();
		e.buffer_mut().is_formatting = true;
		e.execute(Command::FormatDocument);
		assert!(
			e.buffer().fmt_rx.is_none(),
			"a second format must not start while one is in flight"
		);
	}

	#[test]
	fn close_buffer_cleans_up_swap_file() {
		// P4-M: discarding/closing a buffer must remove its swap, else reopening
		// the file offers "recovery" of content the user discarded.
		let mut e = Editor::new();
		let mut swp = std::env::temp_dir();
		swp.push(format!("dan_p4m_close_{}.swp", std::process::id()));
		std::fs::write(&swp, "swap").unwrap();
		e.buffer_mut().swp_path = Some(swp.clone());
		e.push_new_untitled(); // second buffer so we close a non-last one
		e.close_buffer(0).expect("close");
		assert!(!swp.exists(), "closing a buffer must remove its swap file");
		std::fs::remove_file(&swp).ok();
	}

	#[test]
	fn force_quit_cleans_up_swap_file() {
		// P4-M: ForceQuit discards the active buffer; its swap must go too.
		use crate::editor::commands::Command;
		use crate::editor::mode::Mode;
		let mut e = Editor::new();
		let mut swp = std::env::temp_dir();
		swp.push(format!("dan_p4m_fq_{}.swp", std::process::id()));
		std::fs::write(&swp, "swap").unwrap();
		e.buffer_mut().dirty = true;
		e.buffer_mut().swp_path = Some(swp.clone());
		e.quit_cycle_idx = Some(0);
		e.mode = Mode::ConfirmQuit;
		e.execute(Command::ForceQuit);
		assert!(!swp.exists(), "force-quit discard must remove the swap file");
		std::fs::remove_file(&swp).ok();
	}

	#[test]
	fn is_formatting_without_rx_clears_on_poll() {
		// If is_formatting is set without a receiver, poll_async_tasks must
		// self-heal so the main loop doesn't tight-poll at 25 ms forever.
		let mut e = Editor::new();
		e.buffer_mut().is_formatting = true;
		e.poll_async_tasks();
		assert!(!e.buffer().is_formatting);
	}

	#[test]
	fn project_index_is_bounded() {
		// P3-G: the index walker pushes every project file; cap it and stop
		// draining (drop the receiver) once the cap is hit.
		use std::sync::mpsc;
		let mut e = Editor::new();
		let (tx, rx) = mpsc::channel();
		for i in 0..(MAX_PROJECT_INDEX + 100) {
			tx.send(std::path::PathBuf::from(format!("f{}", i))).unwrap();
		}
		e.project_index_rx = Some(rx);
		while e.project_index_rx.is_some() {
			e.poll_async_tasks();
		}
		assert!(
			e.project_index.len() <= MAX_PROJECT_INDEX,
			"project_index grew to {}",
			e.project_index.len()
		);
		assert!(e.project_index_rx.is_none(), "receiver must be dropped at the cap");
		drop(tx);
	}

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

	#[test]
	fn refresh_search_regex_finds_matches() {
		let mut e = Editor::new();
		e.buffer_mut().text = crate::buffer::rope::TextRope::from_str("foo bar foo");
		e.search_query = "/foo/".into();
		e.refresh_search_matches();
		assert!(e.search_is_regex);
		assert!(!e.search_regex_error);
		assert_eq!(e.buffer().search_matches, vec![(0, 3), (8, 11)]);
	}

	#[test]
	fn refresh_search_invalid_regex_clears_matches() {
		let mut e = Editor::new();
		e.buffer_mut().text = crate::buffer::rope::TextRope::from_str("foo");
		e.search_query = "/foo/".into();
		e.refresh_search_matches();
		assert_eq!(e.buffer().search_matches.len(), 1);

		e.search_query = "/foo(/".into();
		e.refresh_search_matches();
		assert!(e.search_is_regex);
		assert!(e.search_regex_error);
		assert!(e.buffer().search_matches.is_empty());
	}

	#[test]
	fn refresh_search_literal_unchanged() {
		let mut e = Editor::new();
		e.buffer_mut().text = crate::buffer::rope::TextRope::from_str("Hello");
		e.search_query = "hello".into();
		e.refresh_search_matches();
		assert!(!e.search_is_regex);
		assert!(!e.search_regex_error);
		assert_eq!(e.buffer().search_matches, vec![(0, 5)]);
	}

	#[test]
	fn regex_replace_yes_expands_capture() {
		let mut e = Editor::new();
		e.buffer_mut().text = crate::buffer::rope::TextRope::from_str("foo_bar");
		e.search_query = "/(foo)_(bar)/".into();
		e.refresh_search_matches();
		assert_eq!(e.buffer().search_matches, vec![(0, 7)]);
		e.replace_with = "$2-$1".into();
		e.mode = crate::editor::mode::Mode::ReplacingStep;
		e.cmd_replace_action_yes();
		assert_eq!(e.buffer().text.to_string_full(), "bar-foo");
	}

	#[test]
	fn literal_replace_does_not_expand_dollar() {
		let mut e = Editor::new();
		e.buffer_mut().text = crate::buffer::rope::TextRope::from_str("foo");
		e.search_query = "foo".into();
		e.refresh_search_matches();
		e.replace_with = "$1".into();
		e.mode = crate::editor::mode::Mode::ReplacingStep;
		e.cmd_replace_action_yes();
		assert_eq!(e.buffer().text.to_string_full(), "$1");
	}

	#[test]
	fn regex_replace_all_expands_each_match() {
		let mut e = Editor::new();
		e.buffer_mut().text = crate::buffer::rope::TextRope::from_str("a1 a2");
		e.search_query = "/a(\\d)/".into();
		e.refresh_search_matches();
		assert_eq!(e.buffer().search_matches.len(), 2);
		e.replace_with = "X$1".into();
		e.mode = crate::editor::mode::Mode::ReplacingStep;
		e.buffer_mut().search_match_idx = 0;
		e.cmd_replace_action_all();
		assert_eq!(e.buffer().text.to_string_full(), "X1 X2");
	}

	#[test]
	fn regex_replace_named_group_and_dollar_escape() {
		let mut e = Editor::new();
		e.buffer_mut().text = crate::buffer::rope::TextRope::from_str("ab");
		e.search_query = "/(?P<x>a)(b)/".into();
		e.refresh_search_matches();
		e.replace_with = "$$-$x".into();
		e.mode = crate::editor::mode::Mode::ReplacingStep;
		e.cmd_replace_action_yes();
		assert_eq!(e.buffer().text.to_string_full(), "$-a");
	}

	#[test]
	fn toggle_comment_line_style_round_trips() {
		let mut e = Editor::new();
		e.buffer_mut().file_path = Some(std::path::PathBuf::from("main.rs"));
		e.buffer_mut().text = crate::buffer::rope::TextRope::from_str("  let x = 1;\n");
		e.buffer_mut().cursors.set_cursor(0, 2);
		e.toggle_comment();
		assert_eq!(e.buffer().text.to_string_full(), "  // let x = 1;\n");
		e.toggle_comment();
		assert_eq!(e.buffer().text.to_string_full(), "  let x = 1;\n");
	}

	#[test]
	fn toggle_comment_html_wraps_with_suffix() {
		let mut e = Editor::new();
		e.buffer_mut().file_path = Some(std::path::PathBuf::from("index.html"));
		e.buffer_mut().text = crate::buffer::rope::TextRope::from_str("  <div></div>\n");
		e.buffer_mut().cursors.set_cursor(0, 2);
		e.toggle_comment();
		assert_eq!(
			e.buffer().text.to_string_full(),
			"  <!-- <div></div> -->\n"
		);
		e.toggle_comment();
		assert_eq!(e.buffer().text.to_string_full(), "  <div></div>\n");
	}

	#[test]
	fn toggle_comment_css_wraps_with_suffix() {
		let mut e = Editor::new();
		e.buffer_mut().file_path = Some(std::path::PathBuf::from("style.css"));
		e.buffer_mut().text = crate::buffer::rope::TextRope::from_str("color: red;\n");
		e.buffer_mut().cursors.set_cursor(0, 0);
		e.toggle_comment();
		assert_eq!(e.buffer().text.to_string_full(), "/* color: red; */\n");
		e.toggle_comment();
		assert_eq!(e.buffer().text.to_string_full(), "color: red;\n");
	}

	#[test]
	fn paste_in_search_goes_to_query_not_document() {
		use crate::editor::commands::Command;
		use crate::editor::mode::Mode;
		use crate::buffer::rope::TextRope;
		let mut e = Editor::new();
		e.buffer_mut().text = TextRope::from_str("DOC\n");
		e.buffer_mut().cursors.set_cursor(0, 0);
		e.mode = Mode::Searching;
		e.search_query.clear();
		e.prompt_cursor = 0;
		e.execute(Command::InsertString("needle".into()));
		assert_eq!(e.search_query, "needle");
		assert_eq!(e.prompt_cursor, 6);
		assert_eq!(e.buffer().text.to_string_full(), "DOC\n", "document must stay untouched");
	}

	#[test]
	fn paste_command_in_save_as_goes_to_prompt() {
		use crate::editor::commands::Command;
		use crate::editor::mode::Mode;
		use crate::buffer::rope::TextRope;
		let mut e = Editor::new();
		e.buffer_mut().text = TextRope::from_str("DOC\n");
		e.mode = Mode::SaveAs;
		e.save_as_input.clear();
		e.prompt_cursor = 0;
		e.internal_clipboard = "/tmp/out.txt".into();
		e.execute(Command::Paste);
		assert_eq!(e.save_as_input, "/tmp/out.txt");
		assert_eq!(e.buffer().text.to_string_full(), "DOC\n");
	}

	#[test]
	fn paste_in_goto_line_keeps_digits_only() {
		use crate::editor::commands::Command;
		use crate::editor::mode::Mode;
		let mut e = Editor::new();
		e.mode = Mode::GoToLine;
		e.goto_line_input.clear();
		e.prompt_cursor = 0;
		e.execute(Command::InsertString("12ab34\n56".into()));
		assert_eq!(e.goto_line_input, "123456");
	}

	#[test]
	fn paste_in_confirm_quit_does_not_touch_document() {
		use crate::editor::commands::Command;
		use crate::editor::mode::Mode;
		use crate::buffer::rope::TextRope;
		let mut e = Editor::new();
		e.buffer_mut().text = TextRope::from_str("DOC\n");
		e.mode = Mode::ConfirmQuit;
		e.execute(Command::InsertString("pwned".into()));
		assert_eq!(e.buffer().text.to_string_full(), "DOC\n");
	}

	#[test]
	fn paste_into_search_sanitizes_escape_injection() {
		use crate::editor::commands::Command;
		use crate::editor::mode::Mode;
		let mut e = Editor::new();
		e.mode = Mode::Searching;
		e.search_query.clear();
		e.prompt_cursor = 0;
		e.execute(Command::InsertString("x\x1b[2Jy".into()));
		assert!(!e.search_query.contains('\x1b'));
		assert!(e.search_query.contains('\u{241B}'));
		assert!(e.search_query.starts_with('x'));
		assert!(e.search_query.ends_with('y'));
	}

	#[test]
	fn paste_into_replace_with_goes_to_prompt() {
		use crate::editor::commands::Command;
		use crate::editor::mode::Mode;
		use crate::buffer::rope::TextRope;
		let mut e = Editor::new();
		e.buffer_mut().text = TextRope::from_str("DOC\n");
		e.mode = Mode::ReplacingWith;
		e.replace_with.clear();
		e.prompt_cursor = 0;
		e.execute(Command::InsertString("repl\x1baced".into()));
		assert_eq!(e.replace_with, "repl\u{241B}aced");
		assert_eq!(e.buffer().text.to_string_full(), "DOC\n");
	}

	#[test]
	fn paste_into_palette_goes_to_query() {
		use crate::editor::commands::Command;
		use crate::editor::mode::Mode;
		use crate::buffer::rope::TextRope;
		let mut e = Editor::new();
		e.buffer_mut().text = TextRope::from_str("DOC\n");
		e.execute(Command::PaletteOpen);
		assert_eq!(e.mode, Mode::Palette);
		e.execute(Command::InsertString("save".into()));
		assert_eq!(e.palette.query, "save");
		assert_eq!(e.buffer().text.to_string_full(), "DOC\n");
	}
}

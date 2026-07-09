// Syntax highlighting powered by syntect.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use syntect::highlighting::{
	FontStyle, HighlightIterator, HighlightState, Highlighter as SyntectHighlighter, Style, Theme,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};
use syntect_assets::assets::HighlightingAssets;

/// Per-char style packed for the line highlight cache (avoids re-lexing the
/// same buffer line every frame while soft-wrap scrolling).
pub type CachedCharStyle = (u8, u8, u8, bool, bool, bool);

struct LineHlEntry {
	colors: Vec<CachedCharStyle>,
	/// Parse/highlight state immediately **after** this line was lexed.
	post: ParseSnapshot,
}

/// Cap so a long scroll session cannot grow without bound.
const LINE_HL_CACHE_CAP: usize = 32;
/// Only cache lines at least this many bytes — short lines are cheap to
/// re-lex; soft-wrap lag comes from long Markdown/code lines.
const LINE_HL_CACHE_MIN_BYTES: usize = 200;

/// Snapshot of syntect's two stateful pieces taken at a line boundary.
#[derive(Clone)]
pub struct ParseSnapshot {
	pub parse: ParseState,
	pub highlight: HighlightState,
}

/// Lines per snapshot. 200 means we replay at most ~200 lines on a cache
/// hit, and store ~50 snapshots per 10 000-line file (~50 KB).
const SNAPSHOT_INTERVAL: usize = 200;

/// Caches syntect parse state every `SNAPSHOT_INTERVAL` lines so the
/// per-frame pre-roll doesn't grow with file length (P1.1).
pub struct HighlightCache {
	pub interval: usize,
	/// `snapshots[i]` is the state at the **start** of line `i * interval`.
	/// Always non-empty after the first prime: `snapshots[0]` is the fresh
	/// state, kept so we can resume from line 0 without rebuilding it.
	pub snapshots: Vec<ParseSnapshot>,
	/// Buffer version this cache is valid for.
	pub buffer_version: u64,
	/// Syntax name (e.g. "Rust") this cache was built against.
	pub syntax_name: String,
	/// Theme name this cache was built against.
	pub theme_name: String,
	/// Sticky resume point: state at the **start** of `last_prime_line`.
	/// Lets consecutive frames that prime at the same (or later) scroll
	/// line skip replaying Markdown/syntect from the nearest 200-line
	/// snapshot — Markdown lexing is ~100× slower than Plain Text.
	pub last_prime_line: usize,
	pub last_prime_state: Option<ParseSnapshot>,
	/// Per-line highlight results for the current buffer version.
	/// Soft-wrap scrolling re-renders the same `scroll_y` line every frame
	/// (only `scroll_vrow` changes); without this, Markdown re-lexes that
	/// whole line on every arrow key (~0.4ms for a ~800-char README line).
	line_hl: HashMap<usize, LineHlEntry>,
}

impl HighlightCache {
	pub fn new() -> Self {
		Self {
			interval: SNAPSHOT_INTERVAL,
			snapshots: Vec::new(),
			buffer_version: 0,
			syntax_name: String::new(),
			theme_name: String::new(),
			last_prime_line: 0,
			last_prime_state: None,
			line_hl: HashMap::new(),
		}
	}

	pub fn invalidate(&mut self) {
		self.snapshots.clear();
		self.buffer_version = 0;
		self.syntax_name.clear();
		self.theme_name.clear();
		self.last_prime_line = 0;
		self.last_prime_state = None;
		self.line_hl.clear();
	}

	pub fn is_valid_for(&self, version: u64, syntax_name: &str, theme_name: &str) -> bool {
		!self.snapshots.is_empty()
			&& self.buffer_version == version
			&& self.syntax_name == syntax_name
			&& self.theme_name == theme_name
	}
}

impl Default for HighlightCache {
	fn default() -> Self {
		Self::new()
	}
}

/// Line-by-line highlighter that mirrors `syntect::easy::HighlightLines`'s
/// behaviour but exposes its internal `ParseState` and `HighlightState` so
/// snapshots can be cloned in and out for the cache.
pub struct LineHighlighter<'a> {
	syntect_hi: SyntectHighlighter<'a>,
	parse_state: ParseState,
	highlight_state: HighlightState,
}

impl<'a> LineHighlighter<'a> {
	pub fn new(syntax: &SyntaxReference, theme: &'a Theme) -> Self {
		let syntect_hi = SyntectHighlighter::new(theme);
		let parse_state = ParseState::new(syntax);
		let highlight_state = HighlightState::new(&syntect_hi, ScopeStack::new());
		Self { syntect_hi, parse_state, highlight_state }
	}

	pub fn highlight_line<'b>(
		&mut self,
		line: &'b str,
		syntax_set: &SyntaxSet,
	) -> Vec<(Style, &'b str)> {
		let ops = self.parse_state.parse_line(line, syntax_set).unwrap_or_default();
		HighlightIterator::new(&mut self.highlight_state, &ops, line, &self.syntect_hi).collect()
	}

	/// Highlight `line_idx`, reusing a cached result when this buffer line was
	/// already lexed for the current cache generation. On a hit, restores the
	/// highlighter to the post-line state so subsequent lines stay consistent.
	pub fn highlight_line_cached(
		&mut self,
		cache: &mut HighlightCache,
		line_idx: usize,
		line: &str,
		syntax_set: &SyntaxSet,
	) -> Vec<CachedCharStyle> {
		if let Some(entry) = cache.line_hl.get(&line_idx) {
			let colors = entry.colors.clone();
			self.restore(&entry.post);
			return colors;
		}

		let ranges = self.highlight_line(line, syntax_set);
		let mut colors: Vec<CachedCharStyle> = Vec::with_capacity(line.len());
		for (style, fragment) in &ranges {
			let fg = style.foreground;
			let bold = style.font_style.contains(FontStyle::BOLD);
			let italic = style.font_style.contains(FontStyle::ITALIC);
			let underline = style.font_style.contains(FontStyle::UNDERLINE);
			for _ in fragment.chars() {
				colors.push((fg.r, fg.g, fg.b, bold, italic, underline));
			}
		}
		if line.len() >= LINE_HL_CACHE_MIN_BYTES {
			let post = self.snapshot();
			if cache.line_hl.len() >= LINE_HL_CACHE_CAP {
				cache.line_hl.clear();
			}
			cache.line_hl.insert(
				line_idx,
				LineHlEntry {
					colors: colors.clone(),
					post,
				},
			);
		}
		colors
	}

	pub fn snapshot(&self) -> ParseSnapshot {
		ParseSnapshot {
			parse: self.parse_state.clone(),
			highlight: self.highlight_state.clone(),
		}
	}

	pub fn restore(&mut self, snap: &ParseSnapshot) {
		self.parse_state = snap.parse.clone();
		self.highlight_state = snap.highlight.clone();
	}
}

/// Holds loaded syntax definitions and themes for highlighting.
///
/// Note: this struct intentionally does **not** own the `HighlightCache` —
/// `ParseState` contains non-`Send` Oniguruma pointers, and `Highlighter`
/// itself is constructed inside a spawned thread (see `Editor::new`'s
/// 32 MB-stack workaround). The cache lives on `Editor` instead.
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

		let active_theme_name = if let Some(matched) =
			assets.themes().find(|name| name.eq_ignore_ascii_case(theme_name))
		{
			matched
		} else {
			if cfg!(debug_assertions) {
				eprintln!("[DEBUG] Theme '{}' not found, falling back to OneHalfDark", theme_name);
			}
			"OneHalfDark"
		};

		let theme = assets.get_theme(active_theme_name).clone();

		Self { syntax_set, theme }
	}

	/// Detect the appropriate syntax for a file path (by filename or extension).
	/// Falls back to plain-text if the syntax is unknown or path is None.
	pub fn detect_syntax(&self, path: Option<&Path>) -> &SyntaxReference {
		let name_str = path.and_then(|p| p.file_name()).and_then(|name| name.to_str());
		let ext_str = path.and_then(|p| p.extension()).and_then(|ext| ext.to_str());

		name_str
			.and_then(|n| self.syntax_set.find_syntax_by_extension(n))
			.or_else(|| ext_str.and_then(|e| self.syntax_set.find_syntax_by_extension(e)))
			.unwrap_or_else(|| self.syntax_set.find_syntax_plain_text())
	}

	/// Build a `LineHighlighter` primed at the start of `target_line`.
	///
	/// On a cache hit (same buffer version, syntax, theme), restores from
	/// the best available resume point ≤ `target_line` and only replays
	/// the gap:
	/// 1. Sticky `last_prime_state` when it is at or before `target_line`
	///    (typical case: same `scroll_y` every frame while editing)
	/// 2. Otherwise the nearest interval snapshot
	///
	/// On miss, rebuilds the cache from line 0, populating snapshots every
	/// `cache.interval` lines as it goes. Either way the returned
	/// highlighter is in the exact state that a fresh `LineHighlighter` +
	/// `highlight_line` for each line `0..target_line` would produce.
	pub fn primed<'a>(
		&'a self,
		cache: &RefCell<HighlightCache>,
		syntax: &'a SyntaxReference,
		target_line: usize,
		buffer_version: u64,
		mut get_line: impl FnMut(usize) -> String,
	) -> LineHighlighter<'a> {
		let syntax_name = syntax.name.clone();
		let theme_name = self.theme.name.clone().unwrap_or_default();

		let mut lh = LineHighlighter::new(syntax, &self.theme);
		let mut cache = cache.borrow_mut();

		let cache_valid = cache.is_valid_for(buffer_version, &syntax_name, &theme_name);
		if !cache_valid {
			cache.invalidate();
			cache.buffer_version = buffer_version;
			cache.syntax_name = syntax_name;
			cache.theme_name = theme_name;
			cache.snapshots.push(lh.snapshot());
		}

		// Prefer sticky resume (same/later scroll) over the coarse snapshot
		// grid — Markdown syntect is expensive enough that replaying 50 lines
		// every frame is user-visible lag.
		let mut current_line = if let Some(ref sticky) = cache.last_prime_state {
			if cache.last_prime_line <= target_line {
				lh.restore(sticky);
				cache.last_prime_line
			} else {
				let usable_idx = (target_line / cache.interval)
					.min(cache.snapshots.len().saturating_sub(1));
				lh.restore(&cache.snapshots[usable_idx]);
				usable_idx * cache.interval
			}
		} else {
			let usable_idx =
				(target_line / cache.interval).min(cache.snapshots.len().saturating_sub(1));
			lh.restore(&cache.snapshots[usable_idx]);
			usable_idx * cache.interval
		};

		while current_line < target_line {
			let line_text = get_line(current_line);
			let _ = lh.highlight_line(&line_text, &self.syntax_set);
			current_line += 1;

			if current_line % cache.interval == 0 {
				let snap_idx = current_line / cache.interval;
				if snap_idx >= cache.snapshots.len() {
					cache.snapshots.push(lh.snapshot());
				}
			}
		}

		cache.last_prime_line = target_line;
		cache.last_prime_state = Some(lh.snapshot());

		lh
	}
}

impl Default for Highlighter {
	fn default() -> Self {
		Self::new("OneHalfDark")
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn make_highlighter() -> Highlighter {
		Highlighter::new("OneHalfDark")
	}

	fn make_cache() -> RefCell<HighlightCache> {
		RefCell::new(HighlightCache::new())
	}

	#[test]
	fn primed_at_zero_returns_fresh_state() {
		let h = make_highlighter();
		let cache = make_cache();
		let syntax = h.detect_syntax(None);
		let _lh = h.primed(&cache, syntax, 0, 1, |_| String::new());
		assert_eq!(cache.borrow().snapshots.len(), 1);
	}

	#[test]
	fn primed_on_long_input_populates_snapshots() {
		let h = make_highlighter();
		let cache = make_cache();
		let syntax = h.detect_syntax(None);
		let target = 500; // 2.5× the default interval
		let _lh = h.primed(&cache, syntax, target, 1, |_| "fn x() {}\n".to_string());
		// Snapshots at lines 0, 200, 400.
		assert_eq!(cache.borrow().snapshots.len(), 3);
	}

	#[test]
	fn primed_reuses_cache_on_repeat() {
		let h = make_highlighter();
		let cache = make_cache();
		let syntax = h.detect_syntax(None);
		let _lh = h.primed(&cache, syntax, 500, 1, |_| "fn x() {}\n".to_string());
		let n1 = cache.borrow().snapshots.len();

		let _lh = h.primed(&cache, syntax, 300, 1, |_| "fn x() {}\n".to_string());
		assert_eq!(cache.borrow().snapshots.len(), n1);
	}

	#[test]
	fn buffer_version_change_invalidates_cache() {
		let h = make_highlighter();
		let cache = make_cache();
		let syntax = h.detect_syntax(None);
		let _lh = h.primed(&cache, syntax, 500, 1, |_| "fn x() {}\n".to_string());
		assert_eq!(cache.borrow().buffer_version, 1);

		let _lh = h.primed(&cache, syntax, 200, 2, |_| "fn x() {}\n".to_string());
		assert_eq!(cache.borrow().buffer_version, 2);
	}

	#[test]
	fn sticky_prime_avoids_replay_on_same_target() {
		use std::cell::Cell;
		let h = make_highlighter();
		let cache = make_cache();
		let syntax = h.detect_syntax(Some(Path::new("README.md")));
		let calls = Cell::new(0usize);
		let _ = h.primed(&cache, syntax, 50, 1, |_| {
			calls.set(calls.get() + 1);
			"# heading\n".to_string()
		});
		assert_eq!(calls.get(), 50, "cold prime replays 0..50");
		assert_eq!(cache.borrow().last_prime_line, 50);

		calls.set(0);
		let _ = h.primed(&cache, syntax, 50, 1, |_| {
			calls.set(calls.get() + 1);
			"# heading\n".to_string()
		});
		assert_eq!(calls.get(), 0, "sticky resume at same line must not re-lex");

		calls.set(0);
		let _ = h.primed(&cache, syntax, 52, 1, |_| {
			calls.set(calls.get() + 1);
			"# heading\n".to_string()
		});
		assert_eq!(calls.get(), 2, "scroll down one/two lines only lexes the gap");
	}

	#[test]
	fn sticky_markdown_prime_is_fast_when_warm() {
		use std::time::Instant;
		let h = make_highlighter();
		let cache = make_cache();
		let syntax = h.detect_syntax(Some(Path::new("README.md")));
		let md = std::fs::read_to_string("README.md").unwrap_or_else(|_| "# x\n".repeat(100));
		let lines: Vec<String> = md.lines().map(|l| format!("{l}\n")).collect();

		let _ = h.primed(&cache, syntax, 50, 1, |i| {
			lines.get(i).cloned().unwrap_or_default()
		});

		let t0 = Instant::now();
		for _ in 0..50 {
			let _ = h.primed(&cache, syntax, 50, 1, |i| {
				lines.get(i).cloned().unwrap_or_default()
			});
		}
		let avg = t0.elapsed() / 50;
		// Warm sticky resume should be well under a millisecond; cold Markdown
		// replay of 50 lines was ~50ms before this fix.
		assert!(
			avg.as_millis() < 5,
			"warm primed(50) avg too slow: {:?}",
			avg
		);
	}
	#[test]
	fn line_hl_cache_avoids_relex_on_same_line() {
		use std::time::Instant;
		let h = make_highlighter();
		let cache = make_cache();
		let syntax = h.detect_syntax(Some(Path::new("x.md")));
		let line = format!("{}\n", "word ".repeat(400)); // ~2k chars

		// Prime sticky state at line 0, then highlight once (cold).
		let mut lh = h.primed(&cache, syntax, 0, 1, |_| String::new());
		{
			let mut c = cache.borrow_mut();
			let t0 = Instant::now();
			let _ = lh.highlight_line_cached(&mut c, 0, &line, &h.syntax_set);
			let cold = t0.elapsed();
			assert!(
				cold.as_micros() > 50,
				"expected cold markdown lex to be measurable, got {:?}",
				cold
			);

			let t1 = Instant::now();
			for _ in 0..50 {
				let _ = lh.highlight_line_cached(&mut c, 0, &line, &h.syntax_set);
			}
			let warm_avg = t1.elapsed() / 50;
			assert!(
				warm_avg < cold / 5,
				"warm cache should be much faster than cold: warm={:?} cold={:?}",
				warm_avg,
				cold
			);
		}
	}
}

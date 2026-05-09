// Syntax highlighting powered by syntect.

use std::cell::RefCell;
use std::path::Path;

use syntect::highlighting::{
	HighlightIterator, HighlightState, Highlighter as SyntectHighlighter, Style, Theme,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};
use syntect_assets::assets::HighlightingAssets;

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
}

impl HighlightCache {
	pub fn new() -> Self {
		Self {
			interval: SNAPSHOT_INTERVAL,
			snapshots: Vec::new(),
			buffer_version: 0,
			syntax_name: String::new(),
			theme_name: String::new(),
		}
	}

	pub fn invalidate(&mut self) {
		self.snapshots.clear();
		self.buffer_version = 0;
		self.syntax_name.clear();
		self.theme_name.clear();
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
	/// the nearest snapshot ≤ `target_line` and only replays the gap.
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
		get_line: impl Fn(usize) -> String,
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

		let usable_idx =
			(target_line / cache.interval).min(cache.snapshots.len().saturating_sub(1));
		lh.restore(&cache.snapshots[usable_idx]);
		let mut current_line = usable_idx * cache.interval;

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
}

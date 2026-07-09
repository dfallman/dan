use ropey::Rope;
use std::ops::Range;

/// A text container backed by `ropey::Rope`.
///
/// All positional operations (line_to_char, char_at, etc.) are O(log n)
/// instead of the O(n) full-scan that a plain String requires.
#[derive(Debug, Clone)]
pub struct TextRope {
	rope: Rope,
}

impl TextRope {
	/// Create an empty text rope.
	pub fn new() -> Self {
		Self { rope: Rope::new() }
	}

	/// Create from a string slice.
	pub fn from_str(s: &str) -> Self {
		Self {
			rope: Rope::from_str(s),
		}
	}

	/// Total number of characters — O(1).
	pub fn len_chars(&self) -> usize {
		self.rope.len_chars()
	}

	/// Total number of lines (always at least 1) — O(1).
	pub fn len_lines(&self) -> usize {
		self.rope.len_lines()
	}

	/// Get the char at a given char index — O(log n).
	pub fn char_at(&self, char_idx: usize) -> char {
		if char_idx < self.rope.len_chars() {
			self.rope.char(char_idx)
		} else {
			'\0'
		}
	}

	/// Insert a string at a char position — O(log n + len).
	pub fn insert_str(&mut self, char_pos: usize, s: &str) {
		self.rope.insert(char_pos, s);
	}

	/// Insert a char at a given char offset — O(log n).
	pub fn insert_char(&mut self, char_pos: usize, ch: char) {
		self.rope.insert_char(char_pos, ch);
	}

	/// Remove a range of characters — O(log n).
	pub fn remove(&mut self, range: Range<usize>) {
		self.rope.remove(range);
	}

	/// Get the char offset of the start of a line — O(log n).
	pub fn line_to_char(&self, line_idx: usize) -> usize {
		let clamped = line_idx.min(self.rope.len_lines().saturating_sub(1));
		self.rope.line_to_char(clamped)
	}

	/// Get the line number that contains a given char offset — O(log n).
	pub fn char_to_line(&self, char_idx: usize) -> usize {
		let clamped = char_idx.min(self.rope.len_chars());
		self.rope.char_to_line(clamped)
	}

	/// Get the number of chars in a given line (including trailing newline) — O(log n).
	pub fn line_len_chars(&self, line_idx: usize) -> usize {
		if line_idx >= self.rope.len_lines() {
			return 0;
		}
		self.rope.line(line_idx).len_chars()
	}

	/// Get a line as a String (including trailing newline if present).
	///
	/// Prefer `line_slice()` when you only need to iterate chars.
	pub fn line(&self, line_idx: usize) -> String {
		if line_idx >= self.rope.len_lines() {
			return String::new();
		}
		let slice = self.rope.line(line_idx);
		slice.to_string()
	}

	/// Get a line as a `ropey::RopeSlice` — zero-allocation.
	///
	/// Out-of-range indices return an empty slice rather than panicking,
	/// matching the bounds behaviour of `line()` and `line_len_chars()`. A
	/// stale cursor or scroll offset (e.g. after an async formatter shrinks the
	/// document) must degrade to an empty render, never crash the editor and
	/// strand the user's unsaved work.
	pub fn line_slice(&self, line_idx: usize) -> ropey::RopeSlice<'_> {
		if line_idx >= self.rope.len_lines() {
			let end = self.rope.len_chars();
			return self.rope.slice(end..end);
		}
		self.rope.line(line_idx)
	}

	/// Extract a range of characters as a String.
	///
	/// Out-of-range or inverted ranges are clamped to the document rather than
	/// panicking: a stale or miscomputed selection (e.g. an inflated cursor
	/// column) must degrade to a clamped read, never crash the editor and
	/// strand the user's unsaved work. Mirrors `line_slice`/`char_at`.
	pub fn slice_to_string(&self, range: Range<usize>) -> String {
		let len = self.rope.len_chars();
		let start = range.start.min(len);
		let end = range.end.min(len).max(start);
		self.rope.slice(start..end).to_string()
	}

	/// Get the full text as a String.
	///
	/// Prefer streaming APIs (`line_slice`, `chars`, `replace_with`) for
	/// whole-buffer transforms — this allocates O(n).
	pub fn to_string_full(&self) -> String {
		self.rope.to_string()
	}

	/// Replace the entire rope contents. O(1) pointer swap after the caller
	/// has built `new` (typically via [`from_builder`] / `RopeBuilder`).
	pub fn replace_with(&mut self, new: TextRope) {
		self.rope = new.rope;
	}

	/// Finish a [`ropey::RopeBuilder`] into a `TextRope`.
	pub fn from_builder(builder: ropey::RopeBuilder) -> Self {
		Self {
			rope: builder.finish(),
		}
	}

	/// True if the document ends with a newline (or is empty → false).
	pub fn ends_with_newline(&self) -> bool {
		let len = self.rope.len_chars();
		len > 0 && self.rope.char(len - 1) == '\n'
	}

	/// Collect each logical line's content **without** its trailing newline.
	///
	/// Ropey's final empty line (present when the file ends in `\n`) is omitted
	/// so the returned vec matches `text.split('\n')` after dropping a trailing
	/// empty segment. Use [`ends_with_newline`] to restore the terminator.
	pub fn lines_without_newline(&self) -> Vec<String> {
		let n = self.rope.len_lines();
		let mut out = Vec::with_capacity(n);
		for i in 0..n {
			let slice = self.rope.line(i);
			// Final empty line = file ended with \n; skip it.
			if i + 1 == n && slice.len_chars() == 0 {
				break;
			}
			if slice.len_chars() > 0 && slice.char(slice.len_chars() - 1) == '\n' {
				out.push(slice.slice(..slice.len_chars() - 1).to_string());
			} else {
				out.push(slice.to_string());
			}
		}
		out
	}

	/// Find all non-overlapping case-insensitive occurrences of `needle` in
	/// the rope. Returns `(start_char, end_char)` pairs in char-offset units.
	///
	/// Uses a streaming sliding-window matcher over `rope.chars()` so chunk
	/// boundaries are transparent and Unicode needles are byte-safe.
	/// Case folding is the legacy single-char approximation
	/// (`ch.to_lowercase().next()`) — consistent with the rest of the editor.
	pub fn find_all(&self, needle: &str) -> Vec<(usize, usize)> {
		if needle.is_empty() {
			return Vec::new();
		}
		let needle_lower: Vec<char> = needle
			.chars()
			.map(|c| c.to_lowercase().next().unwrap_or(c))
			.collect();
		let needle_len = needle_lower.len();
		if needle_len > self.rope.len_chars() {
			return Vec::new();
		}

		// Sliding window of the last `needle_len` lowercased chars seen.
		let mut window: std::collections::VecDeque<char> =
			std::collections::VecDeque::with_capacity(needle_len);
		let mut results = Vec::new();
		let mut pos = 0usize;

		for raw_ch in self.rope.chars() {
			let ch = raw_ch.to_lowercase().next().unwrap_or(raw_ch);
			if window.len() == needle_len {
				window.pop_front();
			}
			window.push_back(ch);
			pos += 1;

			if window.len() == needle_len
				&& window.iter().zip(needle_lower.iter()).all(|(a, b)| a == b)
			{
				let start = pos - needle_len;
				results.push((start, start + needle_len));
				// Non-overlapping: skip past this match before resuming.
				window.clear();
			}
		}

		results
	}

	/// Regex search over a fully materialized UTF-8 haystack.
	/// Returns char-offset `(start, end)` pairs. Zero-width matches are skipped
	/// (advance one char past the empty match so collection cannot loop forever).
	pub fn find_all_regex(&self, re: &regex::Regex) -> Vec<(usize, usize)> {
		let haystack = self.to_string_full();
		let mut results = Vec::new();
		let mut search_from = 0usize; // byte offset
		while search_from <= haystack.len() {
			let Some(m) = re.find_at(&haystack, search_from) else {
				break;
			};
			let byte_start = m.start();
			let byte_end = m.end();
			if byte_start == byte_end {
				// Skip zero-width: advance one char (or one byte at EOF).
				let advance = haystack[byte_start..]
					.chars()
					.next()
					.map(|c| c.len_utf8())
					.unwrap_or(1);
				search_from = byte_start.saturating_add(advance);
				if search_from == byte_start {
					break;
				}
				continue;
			}
			let start_char = haystack[..byte_start].chars().count();
			let end_char = start_char + haystack[byte_start..byte_end].chars().count();
			results.push((start_char, end_char));
			search_from = byte_end;
		}
		results
	}
}
impl Default for TextRope {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_empty() {
		let r = TextRope::new();
		assert_eq!(r.len_chars(), 0);
		assert_eq!(r.len_lines(), 1);
	}

	#[test]
	fn test_from_str() {
		let r = TextRope::from_str("hello\nworld\n");
		assert_eq!(r.len_chars(), 12);
		assert_eq!(r.len_lines(), 3); // "hello\n", "world\n", ""
	}

	#[test]
	fn test_insert_char() {
		let mut r = TextRope::new();
		r.insert_str(0, "a");
		r.insert_str(1, "b");
		r.insert_str(2, "c");
		assert_eq!(r.to_string_full(), "abc");
	}

	#[test]
	fn test_insert_str() {
		let mut r = TextRope::new();
		r.insert_str(0, "hello");
		assert_eq!(r.to_string_full(), "hello");
		r.insert_str(5, " world");
		assert_eq!(r.to_string_full(), "hello world");
	}

	#[test]
	fn test_remove() {
		let mut r = TextRope::from_str("hello world");
		r.remove(5..11);
		assert_eq!(r.to_string_full(), "hello");
	}

	#[test]
	fn test_line_to_char() {
		let r = TextRope::from_str("hello\nworld\n");
		assert_eq!(r.line_to_char(0), 0);
		assert_eq!(r.line_to_char(1), 6);
		assert_eq!(r.line_to_char(2), 12);
	}

	#[test]
	fn test_char_to_line() {
		let r = TextRope::from_str("hello\nworld\n");
		assert_eq!(r.char_to_line(0), 0);
		assert_eq!(r.char_to_line(5), 0);
		assert_eq!(r.char_to_line(6), 1);
	}

	#[test]
	fn test_line_content() {
		let r = TextRope::from_str("hello\nworld\n");
		assert_eq!(r.line(0), "hello\n");
		assert_eq!(r.line(1), "world\n");
	}

	#[test]
	fn test_char_at() {
		let r = TextRope::from_str("abc");
		assert_eq!(r.char_at(0), 'a');
		assert_eq!(r.char_at(1), 'b');
		assert_eq!(r.char_at(2), 'c');
	}

	#[test]
	fn line_slice_out_of_bounds_returns_empty() {
		// Regression: a line index at/past the end must not panic. ropey's
		// `Rope::line` panics "Attempt to index past end of Rope" — `line()`
		// and `line_len_chars()` guard against it, but `line_slice()` did not,
		// so a stale cursor/scroll index crashed the whole editor.
		let r = TextRope::from_str("a\nb"); // lines: "a\n", "b" -> len_lines == 2
		assert_eq!(r.len_lines(), 2);
		assert_eq!(r.line_slice(2).len_chars(), 0, "index == len_lines");
		assert_eq!(r.line_slice(99).len_chars(), 0, "index well past end");
	}

	#[test]
	fn slice_to_string_clamps_out_of_range() {
		// Backstop for P1-D: a stale/inflated selection range must degrade to a
		// clamped slice, never panic the editor and strand unsaved work.
		let r = TextRope::from_str("abc");
		assert_eq!(r.slice_to_string(0..99), "abc");
		assert_eq!(r.slice_to_string(2..99), "c");
		assert_eq!(r.slice_to_string(5..2), ""); // start > end
	}

	#[test]
	fn find_all_empty_needle() {
		let r = TextRope::from_str("hello");
		assert!(r.find_all("").is_empty());
	}

	#[test]
	fn find_all_basic() {
		let r = TextRope::from_str("hello world hello");
		assert_eq!(r.find_all("hello"), vec![(0, 5), (12, 17)]);
	}

	#[test]
	fn find_all_case_insensitive() {
		let r = TextRope::from_str("Hello HELLO hello");
		assert_eq!(r.find_all("hello"), vec![(0, 5), (6, 11), (12, 17)]);
	}

	#[test]
	fn find_all_multiple_matches_in_one_chunk() {
		// P1.3 regression: the memmem-based v0.2.60 implementation found at
		// most one match per rope chunk. A short string is a single chunk.
		let r = TextRope::from_str("foo foo foo");
		assert_eq!(r.find_all("foo"), vec![(0, 3), (4, 7), (8, 11)]);
	}

	#[test]
	fn find_all_non_ascii_needle_correct_offsets() {
		// P1.3 regression: end_char was needle_bytes.len() (a byte count),
		// breaking selection ranges and replace targets for any multi-byte needle.
		// "weiß" is 4 chars; "ß" is 1 char (2 UTF-8 bytes).
		let r = TextRope::from_str("weiß weiß");
		let hits = r.find_all("ß");
		assert_eq!(hits.len(), 2);
		assert_eq!(hits[0].1 - hits[0].0, 1, "match span should be 1 char, not 2 bytes");
		assert_eq!(hits[1].1 - hits[1].0, 1);
	}

	#[test]
	fn find_all_cross_chunk_match() {
		// P1.3 regression: a needle straddling a chunk boundary was missed entirely.
		// Build a rope big enough to span multiple internal chunks (>1KB),
		// arrange a needle near the boundary by inserting the needle at a
		// large offset.
		let mut r = TextRope::from_str(&"a".repeat(2048));
		r.insert_str(1000, "needle");
		let hits = r.find_all("needle");
		assert_eq!(hits, vec![(1000, 1006)]);
	}

	#[test]
	fn find_all_no_overlap_self_repeating() {
		// "aa" in "aaaa" yields non-overlapping matches at 0 and 2.
		let r = TextRope::from_str("aaaa");
		assert_eq!(r.find_all("aa"), vec![(0, 2), (2, 4)]);
	}

	#[test]
	fn find_all_regex_basic() {
		let r = TextRope::from_str("foo bar foo");
		let re = regex::Regex::new("foo").unwrap();
		assert_eq!(r.find_all_regex(&re), vec![(0, 3), (8, 11)]);
	}

	#[test]
	fn find_all_regex_case_sensitive_by_default() {
		let r = TextRope::from_str("Foo foo");
		let re = regex::Regex::new("foo").unwrap();
		assert_eq!(r.find_all_regex(&re), vec![(4, 7)]);
	}

	#[test]
	fn find_all_regex_inline_case_insensitive() {
		let r = TextRope::from_str("Foo foo");
		let re = regex::Regex::new("(?i)foo").unwrap();
		assert_eq!(r.find_all_regex(&re), vec![(0, 3), (4, 7)]);
	}

	#[test]
	fn find_all_regex_non_ascii_char_spans() {
		let r = TextRope::from_str("weiß weiß");
		let re = regex::Regex::new("ß").unwrap();
		let hits = r.find_all_regex(&re);
		assert_eq!(hits.len(), 2);
		assert_eq!(hits[0].1 - hits[0].0, 1);
	}

	#[test]
	fn find_all_regex_skips_zero_width() {
		let r = TextRope::from_str("aa");
		let re = regex::Regex::new("a*").unwrap();
		let hits = r.find_all_regex(&re);
		assert!(hits.iter().all(|&(s, e)| s < e), "no zero-width: {:?}", hits);
		assert_eq!(hits, vec![(0, 2)]);
	}

	#[test]
	fn find_all_literal_still_case_insensitive() {
		let r = TextRope::from_str("Hello");
		assert_eq!(r.find_all("hello"), vec![(0, 5)]);
	}
}

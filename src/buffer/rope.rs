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
    pub fn line_slice(&self, line_idx: usize) -> ropey::RopeSlice<'_> {
        self.rope.line(line_idx)
    }

    /// Extract a range of characters as a String.
    pub fn slice_to_string(&self, range: Range<usize>) -> String {
        self.rope.slice(range).to_string()
    }

    /// Get the full text as a String.
    pub fn to_string_full(&self) -> String {
        self.rope.to_string()
    }

    /// Find all case-insensitive occurrences of `needle` in the rope.
    /// Returns `(start_char, end_char)` pairs.
    pub fn find_all(&self, needle: &str) -> Vec<(usize, usize)> {
        if needle.is_empty() {
            return Vec::new();
        }
        let needle_lower: String = needle.to_lowercase();
        let needle_chars: Vec<char> = needle_lower.chars().collect();
        let needle_len = needle_chars.len();
        let total = self.rope.len_chars();

        let mut results = Vec::new();
        let mut i: usize = 0;

        while i + needle_len <= total {
            let mut matched = true;
            for j in 0..needle_len {
                let ch = self.rope.char(i + j);
                // Compare lowercased (handles basic case folding)
                let lower_ch = ch.to_lowercase().next().unwrap_or(ch);
                if lower_ch != needle_chars[j] {
                    matched = false;
                    break;
                }
            }
            if matched {
                results.push((i, i + needle_len));
                i += needle_len; // skip past this match
            } else {
                i += 1;
            }
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
}

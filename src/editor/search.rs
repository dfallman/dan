use crate::editor::Editor;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ParsedSearch<'a> {
	Literal(&'a str),
	RegexPattern(&'a str),
}

/// `/pattern/` with non-empty interior → regex; otherwise literal (incl. `//`).
pub(crate) fn parse_search_query(query: &str) -> ParsedSearch<'_> {
	let bytes = query.as_bytes();
	if bytes.len() >= 3 && bytes[0] == b'/' && bytes[bytes.len() - 1] == b'/' {
		let interior = &query[1..query.len() - 1];
		if !interior.is_empty() {
			return ParsedSearch::RegexPattern(interior);
		}
	}
	ParsedSearch::Literal(query)
}

impl Editor {
	/// Re-run the search against the buffer and jump to the nearest match.
	pub(crate) fn refresh_search_matches(&mut self) {
		let matches = self.buffer().text.find_all(&self.search_query);
		self.buffer_mut().search_matches = matches;
		if self.buffer().search_matches.is_empty() {
			self.clear_status();
			return;
		}
		// Find the match nearest (at or after) the saved cursor position.
		let anchor_pos = if let Some((line, col)) = self.buffer().search_saved_cursor {
			self.buffer().text.line_to_char(line) + col
		} else {
			0
		};
		let idx = self
			.buffer()
			.search_matches
			.iter()
			.position(|&(start, _)| start >= anchor_pos)
			.unwrap_or(0);
		self.buffer_mut().search_match_idx = idx;
		self.jump_to_search_match();
	}

	/// Jump the cursor to the currently-highlighted search match.
	pub(crate) fn jump_to_search_match(&mut self) {
		let buf = self.buffer();
		if let Some(&(start, _end)) = buf.search_matches.get(buf.search_match_idx) {
			let line = buf.text.char_to_line(start);
			let col = start - buf.text.line_to_char(line);
			self.buffer_mut().cursors.set_cursor(line, col);

		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_literal_plain() {
		assert!(matches!(parse_search_query("foo"), ParsedSearch::Literal("foo")));
	}

	#[test]
	fn parse_regex_wrapped() {
		assert!(matches!(parse_search_query("/foo/"), ParsedSearch::RegexPattern("foo")));
		assert!(matches!(parse_search_query("/a|b/"), ParsedSearch::RegexPattern("a|b")));
	}

	#[test]
	fn parse_not_regex_edge_cases() {
		assert!(matches!(parse_search_query("//"), ParsedSearch::Literal("//")));
		assert!(matches!(parse_search_query("/"), ParsedSearch::Literal("/")));
		assert!(matches!(parse_search_query("/foo"), ParsedSearch::Literal("/foo")));
		assert!(matches!(parse_search_query("foo/"), ParsedSearch::Literal("foo/")));
		assert!(matches!(parse_search_query(""), ParsedSearch::Literal("")));
	}
}

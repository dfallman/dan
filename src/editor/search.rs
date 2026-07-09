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
	pub(crate) fn clear_search_regex_state(&mut self) {
		self.search_is_regex = false;
		self.cached_regex = None;
		self.search_regex_error = false;
	}

	/// Re-run the search against the buffer and jump to the nearest match.
	pub(crate) fn refresh_search_matches(&mut self) {
		self.search_regex_error = false;
		let matches = match parse_search_query(&self.search_query) {
			ParsedSearch::Literal(needle) => {
				self.search_is_regex = false;
				self.cached_regex = None;
				self.buffer().text.find_all(needle)
			}
			ParsedSearch::RegexPattern(pattern) => {
				self.search_is_regex = true;
				let need_compile = self
					.cached_regex
					.as_ref()
					.map(|re| re.as_str() != pattern)
					.unwrap_or(true);
				if need_compile {
					match regex::Regex::new(pattern) {
						Ok(re) => self.cached_regex = Some(re),
						Err(_) => {
							self.cached_regex = None;
							self.search_regex_error = true;
							self.buffer_mut().search_matches.clear();
							self.buffer_mut().search_match_idx = 0;
							self.clear_status();
							return;
						}
					}
				}
				let re = self.cached_regex.as_ref().unwrap();
				self.buffer().text.find_all_regex(re)
			}
		};

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

	/// Expand `replace_with` for a match char range. Literal mode returns
	/// `replace_with` unchanged (no `$` expansion).
	pub(crate) fn expand_replacement_for_match(&self, start: usize, end: usize) -> String {
		if !self.search_is_regex {
			return self.replace_with.clone();
		}
		let Some(re) = self.cached_regex.as_ref() else {
			return self.replace_with.clone();
		};
		let matched = self.buffer().text.slice_to_string(start..end);
		let Some(caps) = re.captures(&matched) else {
			return self.replace_with.clone();
		};
		let mut out = String::new();
		caps.expand(&self.replace_with, &mut out);
		out
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

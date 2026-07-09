//! Indent/encoding transforms and text case/sort operations.
//!
//! Whole-buffer transforms rebuild via `RopeBuilder` / `Buffer::replace_text`
//! so we never allocate a second full-document `String` (the previous
//! `to_string_full` + join path doubled peak memory on large logs).

use crate::buffer::rope::TextRope;
use crate::editor::Editor;

impl Editor {
	pub(crate) fn cmd_indent_spaces(&mut self) {
		self.config.expand_tab = true;
		self.set_status("Indent: spaces");
	}

	pub(crate) fn cmd_indent_tabs(&mut self) {
		self.config.expand_tab = false;
		self.set_status("Indent: tabs");
	}

	pub(crate) fn cmd_tab_width(&mut self, w: usize) {
		self.config.tab_width = w;
		self.set_status(format!("Tab width: {}", w));
	}

	pub(crate) fn cmd_line_endings_l_f(&mut self) {
		self.config.end_of_line = Some("lf".into());
		self.buffer_mut().dirty = true;
		self.set_status("Line endings: LF");
	}

	pub(crate) fn cmd_line_endings_c_r_l_f(&mut self) {
		self.config.end_of_line = Some("crlf".into());
		self.buffer_mut().dirty = true;
		self.set_status("Line endings: CRLF");
	}

	pub(crate) fn cmd_trim_trailing_whitespace_now(&mut self) {
		let trailing_nl = self.buffer().text.ends_with_newline();
		let mut lines = self.buffer().text.lines_without_newline();
		for line in &mut lines {
			let trimmed = line.trim_end_matches([' ', '\t']);
			line.truncate(trimmed.len());
		}
		self.replace_lines(lines, trailing_nl);
		self.set_status("Trimmed trailing whitespace");
	}

	pub(crate) fn cmd_convert_tabs_to_spaces(&mut self) {
		// Convert ONLY leading-whitespace tabs — touching mid-line tabs
		// (e.g. column-aligned data, doc-comments) would silently
		// corrupt formatting.
		let tw = self.tab_width();
		let spaces = " ".repeat(tw);
		let trailing_nl = self.buffer().text.ends_with_newline();
		let mut lines = self.buffer().text.lines_without_newline();
		for line in &mut lines {
			*line = expand_leading_tabs(line, &spaces);
		}
		self.replace_lines(lines, trailing_nl);
		self.set_status(format!("Converted leading tabs to {} spaces", tw));
	}

	pub(crate) fn cmd_convert_spaces_to_tabs(&mut self) {
		// Same constraint as the inverse: only collapse leading-whitespace
		// space-runs, never anything mid-line.
		let tw = self.tab_width();
		let spaces = " ".repeat(tw);
		let trailing_nl = self.buffer().text.ends_with_newline();
		let mut lines = self.buffer().text.lines_without_newline();
		for line in &mut lines {
			*line = collapse_leading_spaces(line, &spaces);
		}
		self.replace_lines(lines, trailing_nl);
		self.set_status("Converted leading spaces to tabs");
	}

	pub(crate) fn cmd_sort_lines_asc(&mut self) {
		sort_lines(self, false);
	}

	pub(crate) fn cmd_sort_lines_desc(&mut self) {
		sort_lines(self, true);
	}

	pub(crate) fn cmd_dedup_adjacent(&mut self) {
		let trailing_nl = self.buffer().text.ends_with_newline();
		let lines = self.buffer().text.lines_without_newline();
		let mut out: Vec<String> = Vec::with_capacity(lines.len());
		for line in lines {
			if out.last().map(String::as_str) != Some(line.as_str()) {
				out.push(line);
			}
		}
		self.replace_lines(out, trailing_nl);
		self.set_status("Deduplicated adjacent lines");
	}

	pub(crate) fn cmd_convert_upper(&mut self) {
		transform_case_linewise(self, |s| s.to_uppercase());
	}

	pub(crate) fn cmd_convert_lower(&mut self) {
		transform_case_linewise(self, |s| s.to_lowercase());
	}

	pub(crate) fn cmd_convert_title(&mut self) {
		// Title-case normalizes whitespace across the whole span (joins with
		// single spaces), so it cannot be applied line-wise. Selection → span
		// string; whole buffer → one contiguous transform then rope swap.
		let (s, e) = match self.selection_range() {
			Some(r) => r,
			None => (0, self.buffer().text.len_chars()),
		};
		let span = self.buffer().text.slice_to_string(s..e);
		let new = title_case(&span);
		if s == 0 && e == self.buffer().text.len_chars() {
			self.buffer_mut().replace_text(TextRope::from_str(&new));
			self.buffer_mut().commit_edits();
		} else {
			self.buffer_mut().delete_range(s, e);
			self.buffer_mut().insert_str(s, &new);
			self.buffer_mut().commit_edits();
		}
		self.clamp_cursors();
	}

	pub(crate) fn cmd_reverse_selection(&mut self) {
		let range = self.selection_range();
		let (s, e) = match range {
			Some(r) => r,
			None => {
				let line = self.buffer().cursors.cursor().line;
				let line_start = self.buffer().text.line_to_char(line);
				let line_len = self.buffer().text.line_len_chars(line);
				(line_start, line_start + line_len.saturating_sub(1))
			}
		};
		let span = self.buffer().text.slice_to_string(s..e);
		let reversed: String = span.chars().rev().collect();
		self.buffer_mut().delete_range(s, e);
		self.buffer_mut().insert_str(s, &reversed);
		self.buffer_mut().commit_edits();
		self.clamp_cursors();
		self.set_status("Reversed");
	}

	/// Rebuild the buffer from logical lines (no trailing `\n` in each entry).
	fn replace_lines(&mut self, lines: Vec<String>, trailing_nl: bool) {
		let new = join_lines_rope(lines, trailing_nl);
		self.buffer_mut().replace_text(new);
		self.buffer_mut().commit_edits();
		self.clamp_cursors();
	}
}

fn join_lines_rope(lines: Vec<String>, trailing_nl: bool) -> TextRope {
	let mut builder = ropey::RopeBuilder::new();
	let n = lines.len();
	for (i, line) in lines.into_iter().enumerate() {
		builder.append(&line);
		if i + 1 < n || trailing_nl {
			builder.append("\n");
		}
	}
	TextRope::from_builder(builder)
}

fn expand_leading_tabs(line: &str, spaces: &str) -> String {
	let leading_end = line
		.find(|c: char| c != '\t' && c != ' ')
		.unwrap_or(line.len());
	let mut leading = String::new();
	for c in line[..leading_end].chars() {
		if c == '\t' {
			leading.push_str(spaces);
		} else {
			leading.push(c);
		}
	}
	leading.push_str(&line[leading_end..]);
	leading
}

fn collapse_leading_spaces(line: &str, spaces: &str) -> String {
	let leading_end = line
		.find(|c: char| c != ' ' && c != '\t')
		.unwrap_or(line.len());
	let leading_spaces = &line[..leading_end];
	let tabs = leading_spaces.replace(spaces, "\t");
	format!("{}{}", tabs, &line[leading_end..])
}

fn sort_lines(editor: &mut Editor, descending: bool) {
	let trailing_nl = editor.buffer().text.ends_with_newline();
	let mut lines = editor.buffer().text.lines_without_newline();
	if descending {
		lines.sort_by(|a, b| b.cmp(a));
	} else {
		lines.sort();
	}
	editor.replace_lines(lines, trailing_nl);
	editor.set_status(if descending {
		"Sorted lines descending"
	} else {
		"Sorted lines ascending"
	});
}

fn title_case(s: &str) -> String {
	s.split_whitespace()
		.map(|w| {
			let mut c = w.chars();
			match c.next() {
				None => String::new(),
				Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
			}
		})
		.collect::<Vec<_>>()
		.join(" ")
}

/// Upper/lower on selection or whole buffer. Whole-buffer path maps each line
/// independently into a rebuilt rope (no second full-document `String`).
fn transform_case_linewise(editor: &mut Editor, f: impl Fn(&str) -> String) {
	let (s, e) = match editor.selection_range() {
		Some(r) => r,
		None => (0, editor.buffer().text.len_chars()),
	};

	if s == 0 && e == editor.buffer().text.len_chars() {
		let trailing_nl = editor.buffer().text.ends_with_newline();
		let lines = editor.buffer().text.lines_without_newline();
		let transformed: Vec<String> = lines.into_iter().map(|l| f(&l)).collect();
		editor.replace_lines(transformed, trailing_nl);
		return;
	}

	let span = editor.buffer().text.slice_to_string(s..e);
	let new = f(&span);
	editor.buffer_mut().delete_range(s, e);
	editor.buffer_mut().insert_str(s, &new);
	editor.buffer_mut().commit_edits();
	editor.clamp_cursors();
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::editor::commands::Command;

	#[test]
	fn sort_lines_preserves_trailing_newline() {
		let mut e = Editor::new();
		e.buffer_mut().text = TextRope::from_str("c\na\nb\n");
		e.execute(Command::SortLinesAsc);
		assert_eq!(e.buffer().text.to_string_full(), "a\nb\nc\n");
	}

	#[test]
	fn sort_lines_without_trailing_newline() {
		let mut e = Editor::new();
		e.buffer_mut().text = TextRope::from_str("c\na\nb");
		e.execute(Command::SortLinesAsc);
		assert_eq!(e.buffer().text.to_string_full(), "a\nb\nc");
	}

	#[test]
	fn dedup_adjacent_streams() {
		let mut e = Editor::new();
		e.buffer_mut().text = TextRope::from_str("a\na\nb\nb\na\n");
		e.execute(Command::DedupAdjacent);
		assert_eq!(e.buffer().text.to_string_full(), "a\nb\na\n");
	}

	#[test]
	fn trim_trailing_ws_line_by_line() {
		let mut e = Editor::new();
		e.buffer_mut().text = TextRope::from_str("hi  \nthere\t\n");
		e.execute(Command::TrimTrailingWhitespaceNow);
		assert_eq!(e.buffer().text.to_string_full(), "hi\nthere\n");
	}

	#[test]
	fn convert_tabs_leading_only() {
		let mut e = Editor::new();
		e.config.tab_width = 4;
		e.buffer_mut().text = TextRope::from_str("\tcode\tok\n");
		e.execute(Command::ConvertTabsToSpaces);
		assert_eq!(e.buffer().text.to_string_full(), "    code\tok\n");
	}

	#[test]
	fn upper_selection_only() {
		let mut e = Editor::new();
		e.buffer_mut().text = TextRope::from_str("hello world\n");
		e.buffer_mut().cursors.primary_mut().anchor =
			crate::editor::cursor::Cursor::new(0, 0);
		e.buffer_mut().cursors.primary_mut().head =
			crate::editor::cursor::Cursor::new(0, 5);
		e.execute(Command::ConvertUpper);
		assert_eq!(e.buffer().text.to_string_full(), "HELLO world\n");
	}

	#[test]
	fn upper_whole_buffer_line_wise() {
		let mut e = Editor::new();
		e.buffer_mut().text = TextRope::from_str("ab\ncd\n");
		e.execute(Command::ConvertUpper);
		assert_eq!(e.buffer().text.to_string_full(), "AB\nCD\n");
	}

	#[test]
	fn replace_text_is_one_undo_step() {
		let mut e = Editor::new();
		e.buffer_mut().text = TextRope::from_str("c\na\nb\n");
		e.buffer_mut().commit_edits();
		e.execute(Command::SortLinesAsc);
		assert_eq!(e.buffer().text.to_string_full(), "a\nb\nc\n");
		e.execute(Command::Undo);
		assert_eq!(e.buffer().text.to_string_full(), "c\na\nb\n");
	}

	#[test]
	fn lines_without_newline_matches_split() {
		let r = TextRope::from_str("a\nb\nc\n");
		assert_eq!(r.lines_without_newline(), vec!["a", "b", "c"]);
		assert!(r.ends_with_newline());

		let r2 = TextRope::from_str("a\nb\nc");
		assert_eq!(r2.lines_without_newline(), vec!["a", "b", "c"]);
		assert!(!r2.ends_with_newline());

		let empty = TextRope::from_str("");
		assert_eq!(empty.lines_without_newline(), Vec::<String>::new());
		assert!(!empty.ends_with_newline());
	}
}

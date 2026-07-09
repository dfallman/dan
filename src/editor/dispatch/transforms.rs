//! Indent/encoding transforms and text case/sort operations.
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
		let text = self.buffer().text.to_string_full();
		let trimmed: String = text
			.lines()
			.map(|l| l.trim_end_matches([' ', '\t']))
			.collect::<Vec<_>>()
			.join("\n");
		let final_text = if text.ends_with('\n') {
			trimmed + "\n"
		} else {
			trimmed
		};
		let len = self.buffer().text.len_chars();
		self.buffer_mut().delete_range(0, len);
		self.buffer_mut().insert_str(0, &final_text);
		self.buffer_mut().commit_edits();
		self.clamp_cursors();
		self.set_status("Trimmed trailing whitespace");
	}

	pub(crate) fn cmd_convert_tabs_to_spaces(&mut self) {
		// Convert ONLY leading-whitespace tabs — touching mid-line tabs
		// (e.g. column-aligned data, doc-comments) would silently
		// corrupt formatting.
		let tw = self.tab_width();
		let spaces = " ".repeat(tw);
		let text = self.buffer().text.to_string_full();
		let out: String = text
			.split('\n')
			.map(|line| {
				let leading_end = line
					.find(|c: char| c != '\t' && c != ' ')
					.unwrap_or(line.len());
				let leading: String = line[..leading_end]
					.chars()
					.map(|c| if c == '\t' { spaces.clone() } else { c.to_string() })
					.collect();
				format!("{}{}", leading, &line[leading_end..])
			})
			.collect::<Vec<_>>()
			.join("\n");
		let len = self.buffer().text.len_chars();
		self.buffer_mut().delete_range(0, len);
		self.buffer_mut().insert_str(0, &out);
		self.buffer_mut().commit_edits();
		self.clamp_cursors();
		self.set_status(format!("Converted leading tabs to {} spaces", tw));
	}

	pub(crate) fn cmd_convert_spaces_to_tabs(&mut self) {
		// Same constraint as the inverse: only collapse leading-whitespace
		// space-runs, never anything mid-line.
		let tw = self.tab_width();
		let spaces = " ".repeat(tw);
		let text = self.buffer().text.to_string_full();
		let out: String = text
			.split('\n')
			.map(|line| {
				let leading_end = line
					.find(|c: char| c != ' ' && c != '\t')
					.unwrap_or(line.len());
				let leading_spaces = &line[..leading_end];
				let tabs = leading_spaces.replace(&spaces, "\t");
				format!("{}{}", tabs, &line[leading_end..])
			})
			.collect::<Vec<_>>()
			.join("\n");
		let len = self.buffer().text.len_chars();
		self.buffer_mut().delete_range(0, len);
		self.buffer_mut().insert_str(0, &out);
		self.buffer_mut().commit_edits();
		self.clamp_cursors();
		self.set_status("Converted leading spaces to tabs");
	}

	pub(crate) fn cmd_sort_lines_asc(&mut self) {
		sort_lines(self, false);
	}

	pub(crate) fn cmd_sort_lines_desc(&mut self) {
		sort_lines(self, true);
	}

	pub(crate) fn cmd_dedup_adjacent(&mut self) {
		let text = self.buffer().text.to_string_full();
		let mut last: Option<String> = None;
		let mut out = String::with_capacity(text.len());
		for line in text.split_inclusive('\n') {
			let trimmed = line.trim_end_matches('\n').to_string();
			if Some(&trimmed) != last.as_ref() {
				out.push_str(line);
				last = Some(trimmed);
			}
		}
		let len = self.buffer().text.len_chars();
		self.buffer_mut().delete_range(0, len);
		self.buffer_mut().insert_str(0, &out);
		self.buffer_mut().commit_edits();
		self.clamp_cursors();
		self.set_status("Deduplicated adjacent lines");
	}

	pub(crate) fn cmd_convert_upper(&mut self) {
		transform_text(self, |s| s.to_uppercase());
	}

	pub(crate) fn cmd_convert_lower(&mut self) {
		transform_text(self, |s| s.to_lowercase());
	}

	pub(crate) fn cmd_convert_title(&mut self) {
		transform_text(self, |s| {
			s.split_whitespace()
				.map(|w| {
					let mut c = w.chars();
					match c.next() {
						None => String::new(),
						Some(f) => {
							f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()
						}
					}
				})
				.collect::<Vec<_>>()
				.join(" ")
		});
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
}

fn sort_lines(editor: &mut crate::editor::Editor, descending: bool) {
	let text = editor.buffer().text.to_string_full();
	let mut lines: Vec<&str> = text.split('\n').collect();
	let trailing_nl = text.ends_with('\n');
	if trailing_nl {
		lines.pop();
	}
	if descending {
		lines.sort_by(|a, b| b.cmp(a));
	} else {
		lines.sort();
	}
	let mut out = lines.join("\n");
	if trailing_nl {
		out.push('\n');
	}
	let len = editor.buffer().text.len_chars();
	editor.buffer_mut().delete_range(0, len);
	editor.buffer_mut().insert_str(0, &out);
	editor.buffer_mut().commit_edits();
	editor.clamp_cursors();
	editor.set_status(if descending {
		"Sorted lines descending"
	} else {
		"Sorted lines ascending"
	});
}

fn transform_text(editor: &mut crate::editor::Editor, f: impl Fn(&str) -> String) {
	let (s, e) = match editor.selection_range() {
		Some(r) => r,
		None => (0, editor.buffer().text.len_chars()),
	};
	let span = editor.buffer().text.slice_to_string(s..e);
	let new = f(&span);
	editor.buffer_mut().delete_range(s, e);
	editor.buffer_mut().insert_str(s, &new);
	editor.buffer_mut().commit_edits();
	editor.clamp_cursors();
}

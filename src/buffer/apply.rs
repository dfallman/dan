//! Applying on-disk changes to a buffer: one undo step, cursor and scroll
//! kept on the same content, gutter marks updated.

use std::collections::BTreeMap;

use super::marks::{map_line, LineEdit, MarkKind};
use super::merge::Change;
use super::Buffer;

impl Buffer {
	/// Apply `changes` (ascending char ranges into the current text) as a
	/// single undo step and mark the replaced lines with `kind`.
	pub(crate) fn apply_disk_changes(&mut self, changes: &[Change], kind: MarkKind) {
		if changes.is_empty() {
			return;
		}
		self.sync_change_marks();
		// Close the user's in-progress group so their typing stays its own step.
		self.commit_edits();
		let mut edits: Vec<LineEdit> = Vec::with_capacity(changes.len());
		for c in changes.iter().rev() {
			let start = self.text.char_to_line(c.mine.start);
			let old_end = self.text.char_to_line(c.mine.end);
			self.delete_range(c.mine.start, c.mine.end);
			self.insert_str(c.mine.start, &c.theirs);
			let new_end = self.text.char_to_line(c.mine.start + c.theirs.chars().count());
			edits.push(LineEdit {
				start,
				old_lines: old_end - start,
				new_lines: new_end - start,
			});
		}
		self.commit_edits();
		edits.reverse();

		let sel = *self.cursors.primary();
		let (head_line, _) = map_line(&edits, sel.head.line);
		let (anchor_line, _) = map_line(&edits, sel.anchor.line);
		let p = self.cursors.primary_mut();
		p.head.line = head_line;
		p.anchor.line = anchor_line;
		self.clamp_cursors();
		self.scroll_y = map_line(&edits, self.scroll_y).0;

		let mut marks: BTreeMap<usize, MarkKind> = BTreeMap::new();
		for (&line, &k) in &self.change_marks {
			let (l, inside) = map_line(&edits, line);
			if !inside {
				marks.insert(l, k);
			}
		}
		let mut delta: isize = 0;
		for e in &edits {
			let new_start = (e.start as isize + delta) as usize;
			for l in new_start..new_start + e.new_lines.max(1) {
				marks.insert(l, kind);
			}
			delta += e.new_lines as isize - e.old_lines as isize;
		}
		let last = self.text.len_lines().saturating_sub(1);
		self.change_marks = marks.into_iter().map(|(l, k)| (l.min(last), k)).collect();
		self.marks_aligned();
	}

	/// Mark the lines spanned by `ranges` without changing the text.
	pub(crate) fn mark_ranges(&mut self, ranges: &[Change], kind: MarkKind) {
		self.sync_change_marks();
		for c in ranges {
			let first = self.text.char_to_line(c.mine.start);
			let end = self.text.char_to_line(c.mine.end).max(first + 1);
			for l in first..end {
				self.change_marks.insert(l, kind);
			}
		}
		self.marks_aligned();
	}
}

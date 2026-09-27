//! Gutter marks for lines changed on disk, kept aligned with the text as
//! the user edits.

use std::collections::BTreeMap;

use super::rope::TextRope;
use super::Buffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKind {
	/// Changed by another program.
	External,
	/// Changed on disk and in the buffer; the user deferred the choice.
	Conflict,
}

/// `old_lines` lines starting at `start` (pre-edit coordinates) became
/// `new_lines` lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineEdit {
	pub start: usize,
	pub old_lines: usize,
	pub new_lines: usize,
}

/// Where pre-edit `line` lands after `edits` (ascending, non-overlapping,
/// pre-edit coordinates). The flag is true when the line was inside a
/// replaced range; it is then clamped into the replacement.
pub fn map_line(edits: &[LineEdit], line: usize) -> (usize, bool) {
	let mut delta: isize = 0;
	for e in edits {
		if line < e.start {
			break;
		}
		if line < e.start + e.old_lines {
			let new_start = (e.start as isize + delta) as usize;
			let offset = (line - e.start).min(e.new_lines.saturating_sub(1));
			return (new_start + offset, true);
		}
		delta += e.new_lines as isize - e.old_lines as isize;
	}
	((line as isize + delta) as usize, false)
}

/// Re-align marks after an arbitrary edit (typing, undo, line moves,
/// formatter) by comparing the old and new text from both ends. Marks in
/// the changed middle survive only if it kept its line count.
pub fn remap_by_diff(
	marks: &BTreeMap<usize, MarkKind>,
	old: &TextRope,
	new: &TextRope,
) -> BTreeMap<usize, MarkKind> {
	let (on, nn) = (old.len_lines(), new.len_lines());
	// Find the changed byte span first (memcmp speed), then settle the
	// line boundaries with a line compare or two at each edge.
	let (pb, sb) = old.common_affix_bytes(new);
	let mut p = old.byte_to_line(pb).min(nn);
	while p < on && p < nn && old.line_slice(p) == new.line_slice(p) {
		p += 1;
	}
	// Lines that start strictly inside the common suffix are identical.
	let mut s = (on - 1 - old.byte_to_line(old.len_bytes() - sb)).min(on - p).min(nn - p);
	while s < on - p && s < nn - p && old.line_slice(on - 1 - s) == new.line_slice(nn - 1 - s) {
		s += 1;
	}
	let old_mid_end = on - s;
	let same_len = on - s - p == nn - s - p;
	marks
		.iter()
		.filter_map(|(&line, &kind)| {
			if line < p {
				Some((line, kind))
			} else if line >= old_mid_end {
				Some((line + nn - on, kind))
			} else if same_len {
				Some((line, kind))
			} else {
				None
			}
		})
		.collect()
}

impl Buffer {
	/// Bring `change_marks` in line with edits made since they were last
	/// aligned. Cheap when nothing changed or there are no marks.
	pub(crate) fn sync_change_marks(&mut self) {
		if self.marks_version == self.version {
			return;
		}
		if !self.change_marks.is_empty() {
			if let Some(old) = self.marks_text.as_ref() {
				self.change_marks = remap_by_diff(&self.change_marks, old, &self.text);
			}
		}
		self.marks_aligned();
	}

	/// Record the current text as the one `change_marks` refer to.
	pub(crate) fn marks_aligned(&mut self) {
		self.marks_text = if self.change_marks.is_empty() {
			None
		} else {
			Some(self.text.clone())
		};
		self.marks_version = self.version;
	}

	pub fn clear_change_marks(&mut self) {
		self.change_marks.clear();
		self.marks_aligned();
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn marks(lines: &[usize]) -> BTreeMap<usize, MarkKind> {
		lines.iter().map(|&l| (l, MarkKind::External)).collect()
	}
	fn keys(m: &BTreeMap<usize, MarkKind>) -> Vec<usize> {
		m.keys().copied().collect()
	}
	fn remap(ls: &[usize], old: &str, new: &str) -> Vec<usize> {
		keys(&remap_by_diff(&marks(ls), &TextRope::from_str(old), &TextRope::from_str(new)))
	}

	#[test]
	fn edit_below_leaves_marks() {
		assert_eq!(remap(&[1], "a\nb\nc\n", "a\nb\nc\nd\n"), vec![1]);
	}

	#[test]
	fn lines_added_above_shift_marks() {
		assert_eq!(remap(&[1], "a\nb\nc\n", "x\ny\na\nb\nc\n"), vec![3]);
	}

	#[test]
	fn same_line_count_edit_keeps_mark() {
		assert_eq!(remap(&[1], "a\nb\nc\n", "a\nbXYZ\nc\n"), vec![1]);
	}

	#[test]
	fn deleting_the_marked_line_drops_it() {
		assert_eq!(remap(&[1, 2], "a\nb\nc\n", "a\nc\n"), vec![1]);
	}

	#[test]
	fn map_line_shifts_and_clamps() {
		let edits = [LineEdit { start: 2, old_lines: 2, new_lines: 5 }];
		assert_eq!(map_line(&edits, 1), (1, false));
		assert_eq!(map_line(&edits, 3), (3, true));
		assert_eq!(map_line(&edits, 4), (7, false));
		let shrink = [LineEdit { start: 2, old_lines: 3, new_lines: 1 }];
		assert_eq!(map_line(&shrink, 4), (2, true));
		let insert = [LineEdit { start: 2, old_lines: 0, new_lines: 2 }];
		assert_eq!(map_line(&insert, 2), (4, false));
	}

	#[test]
	fn buffer_marks_follow_undo() {
		let mut b = Buffer::new();
		b.insert_str(0, "a\nb\nc\n");
		b.commit_edits();
		b.change_marks = marks(&[1]);
		b.marks_aligned();
		b.insert_str(0, "x\n");
		b.commit_edits();
		b.sync_change_marks();
		assert_eq!(keys(&b.change_marks), vec![2]);
		b.undo();
		b.sync_change_marks();
		assert_eq!(keys(&b.change_marks), vec![1]);
	}

	#[test]
	fn record_saved_clears_marks() {
		let p = std::env::temp_dir().join(format!("dan_marks_{}_save.txt", std::process::id()));
		let mut b = Buffer::new();
		b.insert_str(0, "a\n");
		b.change_marks = marks(&[0]);
		b.marks_aligned();
		b.record_saved("a\n", &p);
		assert!(b.change_marks.is_empty());
	}
}

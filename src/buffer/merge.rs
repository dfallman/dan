//! Three-way line merge for folding an on-disk change into a buffer that
//! may hold unsaved edits. Pure: no editor or buffer state.

use std::ops::Range;
use std::time::Instant;

use similar::{capture_diff_slices_deadline, Algorithm, DiffTag};

/// Replace chars `mine` of the mine text with `theirs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
	pub mine: Range<usize>,
	pub theirs: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Merge {
	/// Disk-only changes, safe to apply. Ascending, non-overlapping char
	/// ranges into the original mine text — apply bottom-up.
	pub auto: Vec<Change>,
	/// Sections both sides changed differently. Ascending char ranges into
	/// mine *after* every `auto` change has been applied.
	pub conflicts: Vec<Change>,
	/// The merge base to keep once `auto` is applied: theirs, except that
	/// each conflict section (and each mine-only section) stays at the base
	/// text. While a conflict is unanswered this lets a newer disk version
	/// re-merge without treating the already-applied changes as the user's.
	pub resolved_base: String,
}

/// Lines with their terminators kept, so joining them restores the text
/// exactly (CRLF and a missing final newline included).
fn lines(s: &str) -> Vec<&str> {
	s.split_inclusive('\n').collect()
}

/// A changed base line range and the side's replacement line range.
#[derive(Debug, Clone)]
struct Hunk {
	base: Range<usize>,
	side: Range<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
	Mine,
	Theirs,
}

fn hunks(base: &[&str], side: &[&str], deadline: Instant) -> Vec<Hunk> {
	// On deadline `similar` returns a coarser but still correct diff.
	let ops = capture_diff_slices_deadline(Algorithm::Myers, base, side, Some(deadline));
	let mut out: Vec<Hunk> = Vec::new();
	for op in ops {
		let (tag, b, s) = op.as_tag_tuple();
		if tag == DiffTag::Equal {
			continue;
		}
		if let Some(last) = out.last_mut() {
			if last.base.end == b.start && last.side.end == s.start {
				last.base.end = b.end;
				last.side.end = s.end;
				continue;
			}
		}
		out.push(Hunk { base: b, side: s });
	}
	out
}

/// Whether two base ranges touch the same lines. Adjacent ranges do not
/// overlap; two insertions at the same point do; an insertion strictly
/// inside the other range does.
fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
	match (a.is_empty(), b.is_empty()) {
		(true, true) => a.start == b.start,
		(true, false) => b.start < a.start && a.start < b.end,
		(false, true) => a.start < b.start && b.start < a.end,
		(false, false) => a.start < b.end && b.start < a.end,
	}
}

/// One side's text for base lines `lo..hi`, given that side's hunks inside
/// the range (ascending).
fn side_text(base: &[&str], side: &[&str], hunks: &[&Hunk], lo: usize, hi: usize) -> String {
	let mut out = String::new();
	let mut b = lo;
	for h in hunks {
		base[b..h.base.start].iter().for_each(|l| out.push_str(l));
		side[h.side.clone()].iter().for_each(|l| out.push_str(l));
		b = h.base.end;
	}
	base[b..hi].iter().for_each(|l| out.push_str(l));
	out
}

pub fn merge3(base: &str, mine: &str, theirs: &str, deadline: Instant) -> Merge {
	let b = lines(base);
	let m = lines(mine);
	let t = lines(theirs);
	let mine_hunks = hunks(&b, &m, deadline);
	let their_hunks = hunks(&b, &t, deadline);

	let mut all: Vec<(Side, &Hunk)> = mine_hunks
		.iter()
		.map(|h| (Side::Mine, h))
		.chain(their_hunks.iter().map(|h| (Side::Theirs, h)))
		.collect();
	all.sort_by_key(|(_, h)| (h.base.start, h.base.end));

	// Char offset of each mine line start, plus one past the end.
	let mut mine_off = Vec::with_capacity(m.len() + 1);
	let mut acc = 0;
	mine_off.push(0);
	for l in &m {
		acc += l.chars().count();
		mine_off.push(acc);
	}

	let mut out = Merge::default();
	// Mine line index minus base line index, before the current cluster.
	let mut mine_delta: isize = 0;
	// Char growth of the auto changes that precede the current cluster.
	let mut auto_delta: isize = 0;
	let mut bpos = 0;
	let mut i = 0;
	while i < all.len() {
		let lo = all[i].1.base.start;
		let mut hi = all[i].1.base.end;
		let mut j = i + 1;
		while j < all.len() && overlaps(&(lo..hi), &all[j].1.base) {
			hi = hi.max(all[j].1.base.end);
			j += 1;
		}
		let pick = |side: Side| -> Vec<&Hunk> {
			all[i..j].iter().filter(|(s, _)| *s == side).map(|(_, h)| *h).collect()
		};
		let mh = pick(Side::Mine);
		let th = pick(Side::Theirs);
		let growth: isize = mh
			.iter()
			.map(|h| h.side.len() as isize - h.base.len() as isize)
			.sum();
		let m_start = (lo as isize + mine_delta) as usize;
		let m_end = (hi as isize + mine_delta + growth) as usize;
		let chars = mine_off[m_start]..mine_off[m_end];

		b[bpos..lo].iter().for_each(|l| out.resolved_base.push_str(l));
		if th.is_empty() {
			b[lo..hi].iter().for_each(|l| out.resolved_base.push_str(l));
		} else {
			let theirs_text = side_text(&b, &t, &th, lo, hi);
			if mh.is_empty() {
				auto_delta += theirs_text.chars().count() as isize - chars.len() as isize;
				out.resolved_base.push_str(&theirs_text);
				out.auto.push(Change { mine: chars, theirs: theirs_text });
			} else if side_text(&b, &m, &mh, lo, hi) != theirs_text {
				let s = (chars.start as isize + auto_delta) as usize;
				let e = (chars.end as isize + auto_delta) as usize;
				b[lo..hi].iter().for_each(|l| out.resolved_base.push_str(l));
				out.conflicts.push(Change { mine: s..e, theirs: theirs_text });
			} else {
				out.resolved_base.push_str(&theirs_text);
			}
		}
		bpos = hi;
		mine_delta += growth;
		i = j;
	}
	b[bpos..].iter().for_each(|l| out.resolved_base.push_str(l));
	out
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::time::Duration;

	fn m(base: &str, mine: &str, theirs: &str) -> Merge {
		merge3(base, mine, theirs, Instant::now() + Duration::from_secs(5))
	}

	fn nothing_to_do(r: Merge) {
		assert!(r.auto.is_empty() && r.conflicts.is_empty(), "{r:?}");
	}

	fn ch(mine: Range<usize>, theirs: &str) -> Change {
		Change { mine, theirs: theirs.to_string() }
	}

	/// Apply `auto` bottom-up to `mine` (char offsets).
	fn apply(mine: &str, changes: &[Change]) -> String {
		let mut chars: Vec<char> = mine.chars().collect();
		for c in changes.iter().rev() {
			chars.splice(c.mine.clone(), c.theirs.chars());
		}
		chars.into_iter().collect()
	}

	#[test]
	fn no_change_is_empty() {
		nothing_to_do(m("a\nb\n", "a\nb\n", "a\nb\n"));
	}

	#[test]
	fn theirs_only_change_is_auto() {
		let r = m("a\nb\nc\n", "a\nb\nc\n", "a\nB\nc\n");
		assert_eq!(r.auto, vec![ch(2..4, "B\n")]);
		assert!(r.conflicts.is_empty());
	}

	#[test]
	fn mine_only_change_needs_nothing() {
		nothing_to_do(m("a\nb\nc\n", "a\nX\nc\n", "a\nb\nc\n"));
	}

	#[test]
	fn non_overlapping_changes_merge() {
		let r = m("a\nb\nc\nd\n", "A\nb\nc\nd\n", "a\nb\nc\nD\n");
		assert_eq!(r.auto, vec![ch(6..8, "D\n")]);
		assert!(r.conflicts.is_empty());
		assert_eq!(apply("A\nb\nc\nd\n", &r.auto), "A\nb\nc\nD\n");
	}

	#[test]
	fn adjacent_line_edits_do_not_conflict() {
		let r = m("a\nb\nc\n", "A\nb\nc\n", "a\nB\nc\n");
		assert_eq!(r.auto, vec![ch(2..4, "B\n")]);
		assert!(r.conflicts.is_empty());
	}

	#[test]
	fn mine_insertion_shifts_theirs_position() {
		let r = m("a\nb\n", "x\na\nb\n", "a\nB\n");
		assert_eq!(r.auto, vec![ch(4..6, "B\n")]);
	}

	#[test]
	fn overlapping_edits_conflict() {
		let r = m("a\nb\nc\n", "a\nMINE\nc\n", "a\nDISK\nc\n");
		assert!(r.auto.is_empty());
		assert_eq!(r.conflicts, vec![ch(2..7, "DISK\n")]);
	}

	#[test]
	fn identical_edits_on_both_sides_are_not_a_conflict() {
		nothing_to_do(m("a\nb\n", "a\nX\n", "a\nX\n"));
	}

	#[test]
	fn insertions_at_same_point_conflict() {
		let r = m("a\n", "a\nm\n", "a\nt\n");
		assert_eq!(r.conflicts, vec![ch(2..4, "t\n")]);
	}

	#[test]
	fn conflict_ranges_are_after_auto_changes() {
		let mine = "a\nb\nc\nMINE\n";
		let r = m("a\nb\nc\nd\n", mine, "A!\nb\nc\nDISK\n");
		assert_eq!(r.auto, vec![ch(0..2, "A!\n")]);
		assert_eq!(r.conflicts, vec![ch(7..12, "DISK\n")]);
		let after: Vec<char> = apply(mine, &r.auto).chars().collect();
		let s: String = after[7..12].iter().collect();
		assert_eq!(s, "MINE\n");
	}

	#[test]
	fn resolved_base_keeps_conflicts_at_base_and_takes_the_rest() {
		let r = m("a\nb\nc\nd\n", "a\nX\nc\nMINE\n", "A!\nb\nc\nDISK\n");
		assert_eq!(r.resolved_base, "A!\nb\nc\nd\n");
		let clean = m("a\nb\n", "a\nb\n", "a\nB\n");
		assert_eq!(clean.resolved_base, "a\nB\n");
	}

	#[test]
	fn trailing_newline_added_on_disk() {
		let r = m("a", "a", "a\n");
		assert_eq!(apply("a", &r.auto), "a\n");
	}

	#[test]
	fn empty_base_and_mine() {
		let r = m("", "", "x\n");
		assert_eq!(r.auto, vec![ch(0..0, "x\n")]);
	}

	#[test]
	fn char_offsets_count_chars_not_bytes() {
		let r = m("é\nb\n", "é\nb\n", "é\nB\n");
		assert_eq!(r.auto, vec![ch(2..4, "B\n")]);
	}

	#[test]
	fn crlf_lines_merge_cleanly() {
		let base = "a\r\nb\r\nc\r\n";
		let r = m(base, "A\r\nb\r\nc\r\n", "a\r\nb\r\nC\r\n");
		assert_eq!(r.auto, vec![ch(6..9, "C\r\n")]);
		assert!(r.conflicts.is_empty());
	}

	#[test]
	fn expired_deadline_still_merges_correctly() {
		let base: String = (0..2000).map(|i| format!("line {i}\n")).collect();
		let theirs = base.replace("line 1500\n", "changed\n").replace("line 10\n", "");
		let r = merge3(&base, &base, &theirs, Instant::now());
		assert!(r.conflicts.is_empty());
		assert_eq!(apply(&base, &r.auto), theirs);
	}
}

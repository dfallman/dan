/// Helpers for converting between char indices and visual (display) columns.
///
/// A visual column counts terminal columns from the left margin.  It differs
/// from a char index whenever the line contains tabs (variable-width) or wide
/// characters (CJK, fullwidth).

use crate::utils::char_width;

/// Compute the visual column at a given char index within a line of text.
///
/// Tabs use the tabstop formula `tab_w - (vc % tab_w)` so that they snap to
/// the next tabstop boundary.
pub(crate) fn visual_col_at(text: &str, char_idx: usize, tab_w: usize) -> usize {
	let mut vc: usize = 0;
	for (i, ch) in text.chars().enumerate() {
		if i >= char_idx {
			break;
		}
		if ch == '\n' || ch == '\r' {
			break;
		}
		if ch == '\t' {
			vc += tab_w - (vc % tab_w);
		} else {
			vc += char_width(ch, tab_w);
		}
	}
	vc
}

/// Given a target visual column, find the best char index within a visual row
/// (char range `[row_start..row_end)`) of `line_text`.
///
/// Snaps to the start of the character whose visual span covers `target_vcol`.
/// If the target is beyond the end of the row, returns the appropriate cap:
/// - `line_len` on the last visual row (cursor-past-end-of-line)
/// - `row_end - 1` on non-last rows (don't spill into next visual row)
pub(crate) fn char_idx_for_visual_col(
	line_text: &str,
	line_len: usize,
	row_start: usize,
	row_end: usize,
	target_vcol: usize,
	tab_w: usize,
	is_last_row: bool,
) -> usize {
	let mut vcol: usize = 0;
	let mut best_col = row_start;

	for (i, ch) in line_text.chars().enumerate() {
		if i < row_start {
			continue;
		}
		if i >= row_end {
			break;
		}
		if ch == '\n' || ch == '\r' {
			break;
		}

		if vcol >= target_vcol {
			return i.min(line_len);
		}

		let ch_w = if ch == '\t' {
			tab_w - (vcol % tab_w)
		} else {
			char_width(ch, tab_w)
		};
		vcol += ch_w;
		best_col = i + 1;
	}

	let max_col = if is_last_row {
		line_len
	} else {
		// Cap at the last char that belongs to *this* visual row so the
		// cursor doesn't spill into the next visual row's domain.
		row_end.saturating_sub(1)
	};
	best_col.min(max_col)
}

#[cfg(test)]
mod tests {
	use super::*;

	// ── visual_col_at ─────────────────────────────────────────────
	#[test]
	fn ascii_visual_col() {
		assert_eq!(visual_col_at("hello", 0, 4), 0);
		assert_eq!(visual_col_at("hello", 3, 4), 3);
		assert_eq!(visual_col_at("hello", 5, 4), 5);
	}

	#[test]
	fn tab_visual_col() {
		// "\thello": tab at col 0 → occupies cols 0-3, 'h' at vcol 4
		assert_eq!(visual_col_at("\thello", 0, 4), 0);
		assert_eq!(visual_col_at("\thello", 1, 4), 4); // past the tab
		assert_eq!(visual_col_at("\thello", 2, 4), 5); // 'e'
	}

	#[test]
	fn cjk_visual_col() {
		// "ab龙cd": a=1, b=1, 龙=2, c=1, d=1
		assert_eq!(visual_col_at("ab龙cd", 0, 4), 0);
		assert_eq!(visual_col_at("ab龙cd", 2, 4), 2); // before 龙
		assert_eq!(visual_col_at("ab龙cd", 3, 4), 4); // after 龙
		assert_eq!(visual_col_at("ab龙cd", 4, 4), 5); // after c
	}

	#[test]
	fn mid_tab_visual_col() {
		// "a\tb": 'a' at vcol 0..1, tab at vcol 1..4, 'b' at vcol 4
		assert_eq!(visual_col_at("a\tb", 1, 4), 1); // at the tab char
		assert_eq!(visual_col_at("a\tb", 2, 4), 4); // past the tab
	}

	// ── char_idx_for_visual_col ───────────────────────────────────
	#[test]
	fn ascii_char_idx() {
		assert_eq!(char_idx_for_visual_col("hello", 5, 0, 5, 3, 4, true), 3);
	}

	#[test]
	fn tab_snap_start() {
		// "\thello" (len=6): target vcol 2 → inside the tab → snap to idx 1 (after tab)
		assert_eq!(char_idx_for_visual_col("\thello", 6, 0, 6, 2, 4, true), 1);
	}

	#[test]
	fn tab_exact_boundary() {
		// target vcol 4 → exactly at the end of the tab → idx 1
		assert_eq!(char_idx_for_visual_col("\thello", 6, 0, 6, 4, 4, true), 1);
	}

	#[test]
	fn cjk_snap() {
		// "ab龙cd": a=vcol0, b=vcol1, 龙=vcol2..3 (2-wide), c=vcol4, d=vcol5
		// target vcol 3 → inside right half of 龙 → snaps to idx 3 ('c')
		assert_eq!(char_idx_for_visual_col("ab龙cd", 5, 0, 5, 3, 4, true), 3);
		// target vcol 2 → exactly at start of 龙 → returns idx 2 (龙)
		assert_eq!(char_idx_for_visual_col("ab龙cd", 5, 0, 5, 2, 4, true), 2);
	}

	#[test]
	fn short_line_clamp() {
		assert_eq!(char_idx_for_visual_col("hi", 2, 0, 2, 10, 4, true), 2);
	}

	#[test]
	fn non_last_row_clamp() {
		// On a non-last row, cap at row_end - 1
		assert_eq!(char_idx_for_visual_col("hello world", 11, 0, 5, 99, 4, false), 4);
	}

	#[test]
	fn sticky_column_roundtrip() {
		// Simulate: long line → short line → long line
		let long = "hello world, this is a test";
		let short = "hi";
		let tab_w = 4;

		// Start at col 15 on the long line
		let start_vcol = visual_col_at(long, 15, tab_w);

		// Move down to short line — clamps
		let short_col = char_idx_for_visual_col(short, 2, 0, 2, start_vcol, tab_w, true);
		assert_eq!(short_col, 2); // at end

		// Move down to long line again — restores to visual column
		let restored_col = char_idx_for_visual_col(long, long.chars().count(), 0, long.chars().count(), start_vcol, tab_w, true);
		assert_eq!(restored_col, 15); // back to original
	}
}

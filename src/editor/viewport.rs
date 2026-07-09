use crate::editor::Editor;

impl Editor {
	/// Compute the gutter width (line numbers) for the current buffer.
	pub(crate) fn gutter_width(&self) -> usize {
		if !self.config.line_numbers {
			return 0;
		}
		let lc = self.buffer().line_count();
		if lc == 0 {
			1
		} else {
			(lc as f64).log10().floor() as usize + 1
		}
	}

	/// Compute the text-area width (terminal width minus gutter and separator).
	pub(crate) fn text_area_width(&self) -> usize {
		(self.terminal_width as usize).saturating_sub(self.gutter_width() + 1)
	}
}

/// Build visual row breaks for a line of text.
/// Returns a Vec of (start_char_idx, end_char_idx) for each visual row.
pub(crate) fn visual_rows_for<I: IntoIterator<Item = char>>(
	chars: I,
	tab_w: usize,
	text_area_width: usize,
) -> Vec<(usize, usize)> {
	let mut rows: Vec<(usize, usize)> = Vec::new();
	let mut row_start: usize = 0;
	let mut screen_col: usize = 0;
	let mut char_idx: usize = 0;

	for ch in chars.into_iter() {
		if ch == '\n' || ch == '\r' {
			char_idx += 1;
			continue;
		}
		let ch_w = if ch == '\t' {
			tab_w - (screen_col % tab_w)
		} else {
			crate::utils::char_width(ch, tab_w)
		};

		if screen_col + ch_w > text_area_width && screen_col > 0 {
			rows.push((row_start, char_idx));
			row_start = char_idx;
			screen_col = 0;
		}
		screen_col += ch_w;
		char_idx += 1;
	}
	rows.push((row_start, char_idx));
	rows
}

/// Number of visual rows for a line — same as `visual_rows_for(...).len()`
/// but without allocating the break list.
pub(crate) fn visual_row_count<I: IntoIterator<Item = char>>(
	chars: I,
	tab_w: usize,
	text_area_width: usize,
) -> usize {
	if text_area_width == 0 {
		return 1;
	}
	let mut rows = 1usize;
	let mut screen_col: usize = 0;

	for ch in chars.into_iter() {
		if ch == '\n' || ch == '\r' {
			continue;
		}
		let ch_w = if ch == '\t' {
			tab_w - (screen_col % tab_w)
		} else {
			crate::utils::char_width(ch, tab_w)
		};
		if screen_col + ch_w > text_area_width && screen_col > 0 {
			rows += 1;
			screen_col = 0;
		}
		screen_col += ch_w;
	}
	rows
}

/// Char index where visual row `target_row` (0-based) begins.
/// If `target_row` is past the last row, returns the line's content length
/// (same end index `visual_rows_for` would use for the final row).
pub(crate) fn visual_row_start_char<I: IntoIterator<Item = char>>(
	chars: I,
	tab_w: usize,
	text_area_width: usize,
	target_row: usize,
) -> usize {
	if target_row == 0 || text_area_width == 0 {
		return 0;
	}
	let mut current_vrow: usize = 0;
	let mut screen_col: usize = 0;
	let mut char_idx: usize = 0;
	let mut row_start: usize = 0;

	for ch in chars.into_iter() {
		if ch == '\n' || ch == '\r' {
			char_idx += 1;
			continue;
		}
		let ch_w = if ch == '\t' {
			tab_w - (screen_col % tab_w)
		} else {
			crate::utils::char_width(ch, tab_w)
		};
		if screen_col + ch_w > text_area_width && screen_col > 0 {
			current_vrow += 1;
			row_start = char_idx;
			screen_col = 0;
			if current_vrow == target_row {
				return row_start;
			}
		}
		screen_col += ch_w;
		char_idx += 1;
	}
	row_start
}

/// Visual-row index containing `char_col`, and that row's start char index.
pub(crate) fn visual_row_for_col<I: IntoIterator<Item = char>>(
	chars: I,
	tab_w: usize,
	text_area_width: usize,
	char_col: usize,
) -> (usize, usize) {
	if text_area_width == 0 {
		return (0, 0);
	}
	let mut current_vrow: usize = 0;
	let mut screen_col: usize = 0;
	let mut char_idx: usize = 0;
	let mut row_start: usize = 0;

	for ch in chars.into_iter() {
		if ch == '\n' || ch == '\r' {
			char_idx += 1;
			continue;
		}
		let ch_w = if ch == '\t' {
			tab_w - (screen_col % tab_w)
		} else {
			crate::utils::char_width(ch, tab_w)
		};
		// Wrap before the early return so a char that starts a new visual
		// row is attributed to that row (matches `visual_rows_for`).
		if screen_col + ch_w > text_area_width && screen_col > 0 {
			current_vrow += 1;
			row_start = char_idx;
			screen_col = 0;
		}
		if char_idx >= char_col {
			return (current_vrow, row_start);
		}
		screen_col += ch_w;
		char_idx += 1;
	}
	(current_vrow, row_start)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn row_count_matches_visual_rows_for_len() {
		let s = "word ".repeat(40);
		let tab_w = 4;
		let width = 20;
		assert_eq!(
			visual_row_count(s.chars(), tab_w, width),
			visual_rows_for(s.chars(), tab_w, width).len()
		);
	}

	#[test]
	fn row_start_matches_visual_rows_for() {
		let s = "abcdefghij".repeat(10); // 100 chars
		let tab_w = 4;
		let width = 10;
		let rows = visual_rows_for(s.chars(), tab_w, width);
		for (i, &(start, _)) in rows.iter().enumerate() {
			assert_eq!(
				visual_row_start_char(s.chars(), tab_w, width, i),
				start,
				"row {i}"
			);
		}
	}

	#[test]
	fn row_for_col_matches_visual_rows_for() {
		let s = "abcdefghij".repeat(5);
		let tab_w = 4;
		let width = 10;
		let rows = visual_rows_for(s.chars(), tab_w, width);
		for (i, &(start, end)) in rows.iter().enumerate() {
			let col = start;
			let (vrow, rstart) = visual_row_for_col(s.chars(), tab_w, width, col);
			assert_eq!(vrow, i);
			assert_eq!(rstart, start);
			if end > start {
				let (vrow2, _) = visual_row_for_col(s.chars(), tab_w, width, end - 1);
				assert_eq!(vrow2, i);
			}
		}
	}
}

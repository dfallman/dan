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

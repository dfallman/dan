use crate::editor::layout::WrapOptions;
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

	/// Columns reserved on the right edge for the scrollbar (0 or 1). The column
	/// is reserved whenever the scrollbar is enabled, even while a `"scrolling"`
	/// bar is hidden, so soft-wrap never reflows on show/hide.
	pub(crate) fn scrollbar_columns(&self) -> usize {
		match self.config.scrollbar_mode() {
			crate::config::ScrollbarMode::None => 0,
			_ => 1,
		}
	}

	/// Compute the text-area width (terminal width minus gutter, separator,
	/// and any reserved scrollbar column).
	pub(crate) fn text_area_width(&self) -> usize {
		(self.terminal_width as usize).saturating_sub(self.gutter_width() + 1 + self.scrollbar_columns())
	}

	/// Wrap options for the current editor settings.
	pub(crate) fn wrap_opts(&self) -> WrapOptions {
		WrapOptions::new(self.tab_width(), self.text_area_width())
			.with_breakindent(self.config.breakindent)
	}

	/// Cached visual height of a logical line (wrap mode).
	pub(crate) fn cached_visual_height(&mut self, line_idx: usize) -> usize {
		let opts = self.wrap_opts();
		let buf = self.buffer_mut();
		buf.sync_wrap_cache();
		let text = buf.text.line(line_idx);
		buf.wrap_cache.wrap_points_cached(line_idx, &text, opts).len()
	}
}

#[cfg(test)]
mod tests {
	use crate::editor::Editor;

	fn editor(width: u16) -> Editor {
		let mut e = Editor::new();
		e.terminal_width = width;
		e.terminal_height = 10;
		e.config.line_numbers = false;
		// Editor::new() loads the user's real config; start from a known mode.
		e.config.scrollbar = "none".to_string();
		e
	}

	#[test]
	fn scrollbar_reserves_rightmost_column() {
		let mut e = editor(40);
		assert_eq!(e.text_area_width(), 39);
		assert_eq!(e.scrollbar_columns(), 0);
		e.config.scrollbar = "always".to_string();
		assert_eq!(e.scrollbar_columns(), 1);
		assert_eq!(e.text_area_width(), 38);
		// The column stays reserved in scrolling mode even while the bar is hidden,
		// so soft-wrap never reflows on show/hide.
		e.config.scrollbar = "scrolling".to_string();
		assert_eq!(e.text_area_width(), 38);
	}
}

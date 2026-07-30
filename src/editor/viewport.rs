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

	/// Compute the text-area width (terminal width minus gutter and separator).
	pub(crate) fn text_area_width(&self) -> usize {
		(self.terminal_width as usize).saturating_sub(self.gutter_width() + 1)
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

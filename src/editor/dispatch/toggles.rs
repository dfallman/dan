//! Display toggles, format, and misc status commands.
use crate::editor::Editor;

impl Editor {
	pub(crate) fn cmd_toggle_help(&mut self) {
		self.show_help = !self.show_help;
	}

	pub(crate) fn cmd_toggle_wrap(&mut self) {
		self.config.wrap_lines = !self.config.wrap_lines;
		self.buffer_mut().scroll_vrow = 0;
		self.wrap_cache.clear();
	}

	pub(crate) fn cmd_toggle_whitespace(&mut self) {
		self.config.show_whitespace = !self.config.show_whitespace;
		let status = if self.config.show_whitespace {
			"Whitespace markers on"
		} else {
			"Whitespace markers off"
		};
		self.set_status(status);
	}

	pub(crate) fn cmd_format_document(&mut self) {
		// Don't stack formatter runs: a second spawn would orphan the
		// first worker + child and overwrite fmt_rx (P3-J).
		if self.buffer().is_formatting {
			return;
		}
		let ext_str = self
			.buffer()
			.file_path
			.as_ref()
			.and_then(|p| p.extension())
			.and_then(|s| s.to_str())
			.unwrap_or("js")
			.to_string();

		let content = self.buffer().text.to_string_full();
		let (tx, rx) = std::sync::mpsc::channel();
		let child_pid = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
		crate::editor::formatter::spawn_formatter(
			ext_str,
			content,
			tx,
			std::sync::Arc::clone(&child_pid),
		);

		let baseline = self.buffer().version;
		let buf = self.buffer_mut();
		buf.fmt_rx = Some(rx);
		buf.fmt_child_pid = Some(child_pid);
		buf.fmt_baseline_version = Some(baseline);
		buf.is_formatting = true;
		self.set_status("Formatting...");
	}

	pub(crate) fn cmd_toggle_comment(&mut self) {
		self.toggle_comment();
	}

	pub(crate) fn cmd_toggle_syntax(&mut self) {
		self.config.syntax_highlight = !self.config.syntax_highlight;
		let status = if self.config.syntax_highlight {
			"Syntax highlighting enabled"
		} else {
			"Syntax highlighting disabled"
		};
		self.set_status(status);
	}

	pub(crate) fn cmd_toggle_line_numbers(&mut self) {
		self.config.line_numbers = !self.config.line_numbers;
		self.set_status(if self.config.line_numbers { "Line numbers on" } else { "Line numbers off" });
	}

	pub(crate) fn cmd_reload_configuration(&mut self) {
		self.config = crate::config::Config::load();
		if let Some(p) = self.buffer().file_path.clone() {
			self.config.apply_editorconfig(&p);
		}
		self.set_status("Configuration reloaded");
	}

	pub(crate) fn cmd_show_version(&mut self) {
		self.set_status(format!("dan {} ({})", crate::VERSION.trim(), crate::GIT_HASH));
	}
}

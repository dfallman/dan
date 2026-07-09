//! RAII guard that restores the terminal from raw mode + alt screen on drop.
//!
//! Ensures cleanup on early `?` returns, panics (alongside the panic hook), and
//! normal exit — any path where `main` unwinds while modes were enabled.

use crossterm::event::{
	DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use std::io::{self, BufWriter, Write};

/// Tracks which terminal modes were successfully enabled and restores them in
/// reverse order on drop.
pub struct TerminalGuard {
	writer: BufWriter<io::Stdout>,
	raw_mode: bool,
	alt_screen: bool,
	bracketed_paste: bool,
	mouse: bool,
}

impl TerminalGuard {
	/// Enter raw mode, the alternate screen, bracketed-paste, and optionally mouse capture.
	pub fn enter(mouse: bool) -> io::Result<Self> {
		let stdout = io::stdout();
		let writer = BufWriter::with_capacity(64 * 1024, stdout);
		let mut guard = Self {
			writer,
			raw_mode: false,
			alt_screen: false,
			bracketed_paste: false,
			mouse: false,
		};

		terminal::enable_raw_mode()?;
		guard.raw_mode = true;

		guard.writer.get_mut().execute(EnterAlternateScreen)?;
		guard.alt_screen = true;

		guard.writer.get_mut().execute(EnableBracketedPaste)?;
		guard.bracketed_paste = true;

		if mouse {
			guard.writer.get_mut().execute(EnableMouseCapture)?;
			guard.mouse = true;
		}

		Ok(guard)
	}

	pub fn writer_mut(&mut self) -> &mut BufWriter<io::Stdout> {
		&mut self.writer
	}

	/// Restore terminal modes and flush pending output. Called from `Drop` and
	/// may be called explicitly before returning from `main`.
	pub fn restore(&mut self) {
		if self.mouse {
			let _ = self.writer.get_mut().execute(DisableMouseCapture);
			self.mouse = false;
		}
		if self.bracketed_paste {
			let _ = self.writer.get_mut().execute(DisableBracketedPaste);
			self.bracketed_paste = false;
		}
		if self.alt_screen {
			let _ = self
				.writer
				.get_mut()
				.execute(crossterm::style::ResetColor);
			let _ = self.writer.get_mut().execute(
				crossterm::style::SetAttribute(crossterm::style::Attribute::Reset),
			);
			let _ = self.writer.get_mut().execute(crossterm::cursor::Show);
			let _ = self.writer.get_mut().execute(
				crossterm::cursor::SetCursorStyle::DefaultUserShape,
			);
			let _ = self.writer.get_mut().execute(LeaveAlternateScreen);
			self.alt_screen = false;
		}
		if self.raw_mode {
			let _ = terminal::disable_raw_mode();
			self.raw_mode = false;
		}
		let _ = self.writer.flush();
	}
}

impl Drop for TerminalGuard {
	fn drop(&mut self) {
		self.restore();
	}
}

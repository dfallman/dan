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
use std::sync::atomic::{AtomicBool, Ordering};

/// True after OSC 12 was sent this process; panic/emergency paths check this
/// so they can emit OSC 112 without holding the guard.
static CURSOR_COLOR_APPLIED: AtomicBool = AtomicBool::new(false);

/// Whether a custom cursor color was applied via OSC 12 this process.
pub fn cursor_color_was_applied() -> bool {
	CURSOR_COLOR_APPLIED.load(Ordering::Relaxed)
}

/// Tracks which terminal modes were successfully enabled and restores them in
/// reverse order on drop.
pub struct TerminalGuard {
	writer: BufWriter<io::Stdout>,
	raw_mode: bool,
	alt_screen: bool,
	bracketed_paste: bool,
	mouse: bool,
	cursor_color_set: bool,
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
			cursor_color_set: false,
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

	/// Set the terminal cursor color via OSC 12. Records that restore must emit OSC 112.
	pub fn apply_cursor_color(&mut self, rgb: [u8; 3]) -> io::Result<()> {
		let seq = format!("\x1b]12;#{:02x}{:02x}{:02x}\x07", rgb[0], rgb[1], rgb[2]);
		self.writer.get_mut().write_all(seq.as_bytes())?;
		self.writer.flush()?;
		self.cursor_color_set = true;
		CURSOR_COLOR_APPLIED.store(true, Ordering::Relaxed);
		Ok(())
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
			if self.cursor_color_set {
				let _ = self.writer.get_mut().write_all(b"\x1b]112\x07");
				self.cursor_color_set = false;
				CURSOR_COLOR_APPLIED.store(false, Ordering::Relaxed);
			}
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

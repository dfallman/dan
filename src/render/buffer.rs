use crossterm::{
	cursor::{self, SetCursorStyle},
	style::{self, Attribute, Color, SetAttribute, SetBackgroundColor, SetForegroundColor},
	QueueableCommand,
};
use std::io::{self, Write};

use crate::sanitize::sanitize_char;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
	pub ch: char,
	pub fg: Color,
	pub bg: Color,
	pub underline: bool,
	pub bold: bool,
	pub italic: bool,
	/// True if this cell contains a sanitized control character
	pub sanitized: bool,
}

impl Default for Cell {
	fn default() -> Self {
		Self {
			ch: ' ',
			fg: Color::Reset,
			bg: Color::Reset,
			underline: false,
			bold: false,
			italic: false,
			sanitized: false,
		}
	}
}

pub struct ScreenBuffer {
	pub width: u16,
	pub height: u16,
	pub grid: Vec<Cell>,

	pub cursor_x: u16,
	pub cursor_y: u16,

	pub fg: Color,
	pub bg: Color,
	pub underline: bool,
	pub bold: bool,
	pub italic: bool,

	pub hide_cursor: bool,
	pub term_cursor_x: u16,
	pub term_cursor_y: u16,
	pub cursor_style: SetCursorStyle,
}

impl ScreenBuffer {
	pub fn new(width: u16, height: u16) -> Self {
		Self {
			width,
			height,
			grid: vec![Cell::default(); (width as usize) * (height as usize)],
			cursor_x: 0,
			cursor_y: 0,
			fg: Color::Reset,
			bg: Color::Reset,
			underline: false,
			bold: false,
			italic: false,
			hide_cursor: false,
			term_cursor_x: 0,
			term_cursor_y: 0,
			cursor_style: SetCursorStyle::SteadyBlock,
		}
	}



	pub fn set_fg(&mut self, fg: Color) {
		self.fg = fg;
	}
	pub fn set_bg(&mut self, bg: Color) {
		self.bg = bg;
	}

	pub fn set_underline(&mut self, underline: bool) {
		self.underline = underline;
	}

	pub fn set_bold(&mut self, bold: bool) {
		self.bold = bold;
	}

	/// Clear text attributes (bold/italic/underline) to off. Gutter, padding,
	/// and other "plain" cells must call this before writing: the buffer carries
	/// sticky attribute state, so syntax styling from the previous line's text
	/// (e.g. an italic comment) leaks in otherwise — italic line numbers being
	/// the classic symptom.
	pub fn clear_attrs(&mut self) {
		self.bold = false;
		self.italic = false;
		self.underline = false;
	}

	pub fn mov_to(&mut self, x: u16, y: u16) {
		self.cursor_x = x;
		self.cursor_y = y;
	}

	pub fn put_char(&mut self, ch: char) {
		let (sanitized_ch, was_sanitized) = sanitize_char(ch);
		if self.cursor_y < self.height && self.cursor_x < self.width {
			let idx = (self.cursor_y as usize) * (self.width as usize) + (self.cursor_x as usize);
			if idx < self.grid.len() {
				self.grid[idx] = Cell {
					ch: sanitized_ch,
					fg: self.fg,
					bg: self.bg,
					underline: self.underline,
					bold: self.bold,
					italic: self.italic,
					sanitized: was_sanitized,
				};
			}
		}
		self.cursor_x = self.cursor_x.saturating_add(1);
	}

	pub fn put_str(&mut self, s: &str) {
		for ch in s.chars() {
			self.put_char(ch);
		}
	}

	#[allow(unused_assignments)]
	pub fn diff<W: Write>(&self, old: &ScreenBuffer, w: &mut W) -> io::Result<()> {
		// Assemble the entire frame into a local Vec before writing to `w`.
		// A worst-case repaint on a typical terminal (e.g. 100x30 with RGB
		// colours and per-cell style changes) is ~190 KB — well past the
		// 64 KB capacity of the outer BufWriter. Queueing directly to `w`
		// would trigger mid-frame auto-flushes, sending a partial frame to
		// the terminal and showing the user incomplete renders. In release
		// builds the main loop drives renders many times per second, so the
		// partial-frame windows pile up into visible corruption ("rendering
		// broken when editing"). Assembling the frame in a Vec and writing
		// it once with `write_all` makes each frame an atomic transfer.
		let mut buf: Vec<u8> = Vec::with_capacity(
			(self.width as usize) * (self.height as usize) * 8 + 256,
		);
		let frame = &mut buf;

		// Diagnostic dump. When DAN_RENDER_LOG is set, every frame is
		// appended to that file as a delimited record: header, grid dump,
		// raw ANSI bytes. Lets the user share a reproduction.
		let log_path = std::env::var("DAN_RENDER_LOG").ok();
		let mut grid_dump = String::new();
		if log_path.is_some() {
			use std::fmt::Write as _;
			let _ = writeln!(grid_dump, "--- FRAME {}x{} ---", self.width, self.height);
			for y in 0..self.height {
				let mut row = String::new();
				for x in 0..self.width {
					let idx = (y as usize) * (self.width as usize) + (x as usize);
					row.push(self.grid[idx].ch);
				}
				let _ = writeln!(grid_dump, "{:3}|{}|", y, row);
			}
		}

		let mut last_fg = Color::Reset;
		let mut last_bg = Color::Reset;
		let mut last_bold = false;
		let mut last_underline = false;
		let mut last_italic = false;

		let mut current_x: Option<u16> = None;
		let mut current_y: Option<u16> = None;

		// Force reset style at the start just in case terminal state is dirty
		frame.queue(SetForegroundColor(Color::Reset))?;
		frame.queue(SetBackgroundColor(Color::Reset))?;
		frame.queue(SetAttribute(Attribute::Reset))?;

		for y in 0..self.height {
			let mut changed_run = false;
			for x in 0..self.width {
				let idx = (y as usize) * (self.width as usize) + (x as usize);
				let new_cell = &self.grid[idx];
				let old_cell = if self.width == old.width && self.height == old.height {
					Some(&old.grid[idx])
				} else {
					None
				};

				if let Some(old_c) = old_cell {
					if new_cell == old_c {
						changed_run = false;
						continue;
					}
				}

				if !changed_run || current_x != Some(x) || current_y != Some(y) {
					frame.queue(cursor::MoveTo(x, y))?;
					current_x = Some(x);
					current_y = Some(y);
					changed_run = true;
				}

				if new_cell.fg != last_fg {
					frame.queue(SetForegroundColor(new_cell.fg))?;
					last_fg = new_cell.fg;
				}
				if new_cell.bg != last_bg {
					frame.queue(SetBackgroundColor(new_cell.bg))?;
					last_bg = new_cell.bg;
				}

				if new_cell.bold != last_bold {
					if new_cell.bold {
						frame.queue(SetAttribute(Attribute::Bold))?;
					} else {
						frame.queue(SetAttribute(Attribute::NormalIntensity))?;
					}
					last_bold = new_cell.bold;
				}
				if new_cell.underline != last_underline {
					if new_cell.underline {
						frame.queue(SetAttribute(Attribute::Underlined))?;
					} else {
						frame.queue(SetAttribute(Attribute::NoUnderline))?;
					}
					last_underline = new_cell.underline;
				}

				if new_cell.italic != last_italic {
					if new_cell.italic {
						frame.queue(SetAttribute(Attribute::Italic))?;
					} else {
						frame.queue(SetAttribute(Attribute::NoItalic))?;
					}
					last_italic = new_cell.italic;
				}

				// Highlight sanitized (control char) cells differently
				if new_cell.sanitized {
					// Use purple foreground to indicate sanitized content
					if last_fg != Color::Magenta {
						frame.queue(SetForegroundColor(Color::Magenta))?;
						last_fg = Color::Magenta;
					}
					// Also bold to make it more visible
					if !new_cell.bold {
						frame.queue(SetAttribute(Attribute::Bold))?;
						last_bold = true;
					}
				}

				frame.queue(style::Print(new_cell.ch))?;
				current_x = Some(x.saturating_add(1));
			}
		}

		if self.hide_cursor {
			frame.queue(cursor::Hide)?;
		} else {
			frame.queue(cursor::MoveTo(self.term_cursor_x, self.term_cursor_y))?;
			frame.queue(self.cursor_style)?;
			frame.queue(cursor::Show)?;
		}

		// Single write — outer BufWriter still sees one slice, and even if
		// that slice exceeds its capacity, `BufWriter::write_all` bypasses
		// the buffer and writes the full payload to the underlying stdout
		// in one syscall. Either way the terminal receives the frame
		// atomically.
		w.write_all(&buf)?;
		w.flush()?;

		if let Some(path) = log_path {
			use std::io::Write as _;
			if let Ok(mut f) = std::fs::OpenOptions::new()
				.create(true).append(true).open(&path)
			{
				let _ = f.write_all(grid_dump.as_bytes());
				let _ = writeln!(f, "--- BYTES ({} bytes) ---", buf.len());
				// Escape the raw ANSI for readability: ESC → "\\e", other
				// control chars → "\\xNN", printable chars verbatim.
				let mut escaped = String::with_capacity(buf.len() * 2);
				for &b in &buf {
					match b {
						0x1b => escaped.push_str("\\e"),
						b'\n' => escaped.push_str("\\n\n"),
						0x20..=0x7e => escaped.push(b as char),
						_ => {
							use std::fmt::Write as _;
							let _ = write!(escaped, "\\x{:02x}", b);
						}
					}
				}
				let _ = f.write_all(escaped.as_bytes());
				let _ = writeln!(f);
				let _ = writeln!(f, "--- END FRAME ---\n");
			}
		}

		Ok(())
	}
}

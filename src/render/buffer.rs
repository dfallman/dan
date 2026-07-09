use crossterm::{
	cursor::{self, SetCursorStyle},
	style::{self, Attribute, Color, SetAttribute, SetBackgroundColor, SetForegroundColor},
	QueueableCommand,
};
use std::io::{self, Write};
use unicode_width::UnicodeWidthChar;

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
	/// True if this cell is the trailing half of a wide (width-2) grapheme
	/// written into the previous column. Never printed to the terminal —
	/// the wide char already advanced the cursor past this column.
	pub wide_cont: bool,
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
			wide_cont: false,
		}
	}
}

/// Terminal columns occupied by a single `char` when printed.
/// Tabs are always 1 here — callers expand tabs into multiple cells.
fn cell_print_width(ch: char) -> u16 {
	match ch {
		'\t' => 1,
		c => match UnicodeWidthChar::width(c) {
			Some(0) | None => 0,
			Some(n) => (n as u16).max(1),
		},
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
		let w = cell_print_width(sanitized_ch);
		// Zero-width (combining marks, variation selectors): do not consume a
		// column. Skipping avoids desync; terminals attach these to the prior
		// base character when present in the output stream as a grapheme.
		if w == 0 {
			return;
		}

		if self.cursor_y < self.height && self.cursor_x < self.width {
			let row_base = (self.cursor_y as usize) * (self.width as usize);
			let x = self.cursor_x as usize;
			let idx = row_base + x;
			if idx < self.grid.len() {
				self.grid[idx] = Cell {
					ch: sanitized_ch,
					fg: self.fg,
					bg: self.bg,
					underline: self.underline,
					bold: self.bold,
					italic: self.italic,
					sanitized: was_sanitized,
					wide_cont: false,
				};
			}
			// Reserve the trailing column(s) of a wide glyph so layout and the
			// grid agree. Diff skips these cells when printing.
			for dx in 1..w as usize {
				let cx = x + dx;
				if cx >= self.width as usize {
					break;
				}
				let cidx = row_base + cx;
				if cidx < self.grid.len() {
					self.grid[cidx] = Cell {
						ch: ' ',
						fg: self.fg,
						bg: self.bg,
						underline: self.underline,
						bold: self.bold,
						italic: self.italic,
						sanitized: false,
						wide_cont: true,
					};
				}
			}
		}
		self.cursor_x = self.cursor_x.saturating_add(w);
	}

	pub fn put_str(&mut self, s: &str) {
		for ch in s.chars() {
			self.put_char(ch);
		}
	}

	/// Mute cells outside `rect` (x, y, width, height) — used to dim the editor
	/// behind the command palette modal.
	pub fn dim_outside_rect(&mut self, rect: (u16, u16, u16, u16), dim_fg: Color) {
		let (rx, ry, rw, rh) = rect;
		let x_end = rx.saturating_add(rw);
		let y_end = ry.saturating_add(rh);
		for cy in 0..self.height {
			for cx in 0..self.width {
				if cx >= rx && cx < x_end && cy >= ry && cy < y_end {
					continue;
				}
				let idx = (cy as usize) * (self.width as usize) + (cx as usize);
				let cell = &mut self.grid[idx];
				cell.fg = dim_fg;
				cell.bold = false;
				cell.italic = false;
				cell.underline = false;
			}
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
			let mut x: u16 = 0;
			while x < self.width {
				let idx = (y as usize) * (self.width as usize) + (x as usize);
				let new_cell = &self.grid[idx];
				let old_cell = if self.width == old.width && self.height == old.height {
					Some(&old.grid[idx])
				} else {
					None
				};

				// Continuation columns are covered by the preceding wide
				// glyph. Never Print them — a space here would overwrite the
				// right half of the emoji. Also never re-Print the primary
				// glyph from here: the primary cell is visited first (LTR),
				// and re-emitting on cont-change was double-painting every
				// emoji whenever active-row / selection restyled the line
				// (terminals are ~10× slower at emoji than ASCII).
				if new_cell.wide_cont {
					x = x.saturating_add(1);
					continue;
				}

				if let Some(old_c) = old_cell {
					if new_cell == old_c {
						changed_run = false;
						x = x.saturating_add(1);
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

				let print_w = cell_print_width(new_cell.ch).max(1);
				frame.queue(style::Print(new_cell.ch))?;
				// Terminal advances by the glyph's display width, not by 1.
				// Skip continuation columns in the scan so we don't revisit them.
				current_x = Some(x.saturating_add(print_w));
				x = x.saturating_add(print_w);
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn put_char_ascii_advances_one() {
		let mut s = ScreenBuffer::new(10, 1);
		s.put_char('a');
		s.put_char('b');
		assert_eq!(s.cursor_x, 2);
		assert_eq!(s.grid[0].ch, 'a');
		assert_eq!(s.grid[1].ch, 'b');
		assert!(!s.grid[0].wide_cont);
		assert!(!s.grid[1].wide_cont);
	}

	#[test]
	fn put_char_emoji_reserves_continuation() {
		let mut s = ScreenBuffer::new(10, 1);
		s.put_char('✅');
		assert_eq!(s.cursor_x, 2, "emoji must advance two columns");
		assert_eq!(s.grid[0].ch, '✅');
		assert!(!s.grid[0].wide_cont);
		assert!(s.grid[1].wide_cont, "trailing half must be marked");
		assert_eq!(s.grid[1].ch, ' ');
		s.put_char('x');
		assert_eq!(s.cursor_x, 3);
		assert_eq!(s.grid[2].ch, 'x');
	}

	#[test]
	fn put_char_cjk_reserves_continuation() {
		let mut s = ScreenBuffer::new(10, 1);
		s.put_char('中');
		assert_eq!(s.cursor_x, 2);
		assert!(s.grid[1].wide_cont);
	}

	#[test]
	fn put_str_with_emoji_keeps_columns_aligned() {
		let mut s = ScreenBuffer::new(20, 1);
		s.put_str("✅|❌|ok");
		// ✅(2) +(1) ❌(2) +(1) o(1) k(1) = 8
		assert_eq!(s.cursor_x, 8);
		assert_eq!(s.grid[0].ch, '✅');
		assert!(s.grid[1].wide_cont);
		assert_eq!(s.grid[2].ch, '|');
		assert_eq!(s.grid[3].ch, '❌');
		assert!(s.grid[4].wide_cont);
		assert_eq!(s.grid[5].ch, '|');
		assert_eq!(s.grid[6].ch, 'o');
		assert_eq!(s.grid[7].ch, 'k');
	}

	#[test]
	fn diff_skips_printing_wide_continuation() {
		let mut s = ScreenBuffer::new(6, 1);
		s.hide_cursor = true; // avoid cursor Show/Hide noise in the byte stream
		s.put_str("✅x");
		let empty = ScreenBuffer::new(0, 0);
		let mut out = Vec::new();
		s.diff(&empty, &mut out).unwrap();
		// Count how many times we Print a space: continuation cells must not
		// be printed. The grid has a space at col 1 (wide_cont) — if diff
		// wrongly emitted it, we'd see an extra U+0020 between ✅ and x.
		let text = String::from_utf8_lossy(&out);
		assert!(text.contains('✅'));
		assert!(text.contains('x'));
		let emoji_pos = text.find('✅').unwrap();
		let x_pos = text.find('x').unwrap();
		assert!(x_pos > emoji_pos);
		let between = &text[emoji_pos + '✅'.len_utf8()..x_pos];
		assert!(
			!between.contains(' '),
			"continuation space must not be printed between emoji and next char; got {between:?}"
		);
	}

	#[test]
	fn diff_prints_each_emoji_once_on_style_change() {
		// Active-row / selection restyles both the primary and wide_cont
		// cells. Diff must Print the emoji once, not twice.
		let mut old = ScreenBuffer::new(8, 1);
		old.hide_cursor = true;
		old.put_str("✅❌");

		let mut new = ScreenBuffer::new(8, 1);
		new.hide_cursor = true;
		new.set_bg(Color::DarkGrey);
		new.put_str("✅❌");

		let mut out = Vec::new();
		new.diff(&old, &mut out).unwrap();
		let text = String::from_utf8_lossy(&out);
		assert_eq!(text.matches('✅').count(), 1, "double-painted ✅: {text:?}");
		assert_eq!(text.matches('❌').count(), 1, "double-painted ❌: {text:?}");
	}
}

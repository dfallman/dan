use crossterm::{
	cursor::{self, SetCursorStyle},
	style::{self, Attribute, Color, SetAttribute, SetBackgroundColor, SetForegroundColor},
	QueueableCommand,
};
use std::io::{self, Write};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
	pub ch: char,
	pub fg: Color,
	pub bg: Color,
	pub underline: bool,
	pub bold: bool,
	pub italic: bool,
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

	pub fn mov_to(&mut self, x: u16, y: u16) {
		self.cursor_x = x;
		self.cursor_y = y;
	}

	pub fn put_char(&mut self, ch: char) {
		if self.cursor_y < self.height && self.cursor_x < self.width {
			let idx = (self.cursor_y as usize) * (self.width as usize) + (self.cursor_x as usize);
			if idx < self.grid.len() {
				self.grid[idx] = Cell {
					ch,
					fg: self.fg,
					bg: self.bg,
					underline: self.underline,
					bold: self.bold,
					italic: self.italic,
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
		let mut last_fg = Color::Reset;
		let mut last_bg = Color::Reset;
		let mut last_bold = false;
		let mut last_underline = false;
		let mut last_italic = false;

		let mut current_x: Option<u16> = None;
		let mut current_y: Option<u16> = None;

		// Force reset style at the start just in case terminal state is dirty
		w.queue(SetForegroundColor(Color::Reset))?;
		w.queue(SetBackgroundColor(Color::Reset))?;
		w.queue(SetAttribute(Attribute::Reset))?;

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
					w.queue(cursor::MoveTo(x, y))?;
					current_x = Some(x);
					current_y = Some(y);
					changed_run = true;
				}

				if new_cell.fg != last_fg {
					w.queue(SetForegroundColor(new_cell.fg))?;
					last_fg = new_cell.fg;
				}
				if new_cell.bg != last_bg {
					w.queue(SetBackgroundColor(new_cell.bg))?;
					last_bg = new_cell.bg;
				}

				if new_cell.bold != last_bold {
					if new_cell.bold {
						w.queue(SetAttribute(Attribute::Bold))?;
					} else {
						w.queue(SetAttribute(Attribute::NormalIntensity))?;
					}
					last_bold = new_cell.bold;
				}
				if new_cell.underline != last_underline {
					if new_cell.underline {
						w.queue(SetAttribute(Attribute::Underlined))?;
					} else {
						w.queue(SetAttribute(Attribute::NoUnderline))?;
					}
					last_underline = new_cell.underline;
				}

				if new_cell.italic != last_italic {
					if new_cell.italic {
						w.queue(SetAttribute(Attribute::Italic))?;
					} else {
						w.queue(SetAttribute(Attribute::NoItalic))?;
					}
					last_italic = new_cell.italic;
				}

				w.queue(style::Print(new_cell.ch))?;
				current_x = Some(x.saturating_add(1));
			}
		}

		if self.hide_cursor {
			w.queue(cursor::Hide)?;
		} else {
			w.queue(cursor::MoveTo(self.term_cursor_x, self.term_cursor_y))?;
			w.queue(self.cursor_style.clone())?;
			w.queue(cursor::Show)?;
		}

		w.flush()?;
		Ok(())
	}
}

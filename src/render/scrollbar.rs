//! Vertical scrollbar geometry and painting for the rightmost column.
//!
//! Geometry works in *logical* lines in both wrap and no-wrap mode: the exact
//! visual-row total in wrap mode would mean walking the whole file every frame,
//! which the 100 MB use case rules out. The thumb is therefore a close
//! approximation on heavily wrapped files, and exact otherwise.

use std::time::Instant;

use super::buffer::ScreenBuffer;
use super::Viewport;
use crate::editor::Editor;

/// Thumb glyph: U+2588 Full Block.
pub const THUMB: char = '█';
/// Track glyph: U+2502 Light Vertical.
pub const TRACK: char = '│';

/// Paint the scrollbar into the rightmost column. Stamps scroll activity for
/// the "scrolling" mode and records what was drawn so the main loop can
/// schedule the auto-hide redraw. No-op when no column is reserved.
pub(crate) fn paint(editor: &mut Editor, screen: &mut ScreenBuffer, vp: &Viewport, now: Instant) {
	if editor.scrollbar_columns() == 0 || vp.width == 0 {
		return;
	}
	editor.note_scroll_activity(now);
	let visible = editor.scrollbar_visible_at(now);
	editor.scrollbar_drawn_visible = visible;
	if !visible {
		return;
	}
	// Same track/thumb the mouse hit-test uses, so drags line up with pixels.
	let track = editor.scrollbar_track_rows();
	let (top, len) = editor.scrollbar_thumb();
	let x = vp.width - 1;
	let thumb_fg = editor.theme.status_bg;
	let track_fg = editor.theme.line_nr;
	for row in 0..track {
		screen.mov_to(x, row as u16);
		screen.clear_attrs();
		screen.set_bg(crossterm::style::Color::Reset);
		if row >= top && row < top + len {
			screen.set_fg(thumb_fg);
			screen.put_char(THUMB);
		} else {
			screen.set_fg(track_fg);
			screen.put_char(TRACK);
		}
	}
	screen.set_fg(crossterm::style::Color::Reset);
}

/// Integer `a * b / c` rounded to nearest (ties up). `c == 0` yields 0.
fn mul_div_round(a: usize, b: usize, c: usize) -> usize {
	if c == 0 {
		return 0;
	}
	let num = (a as u128) * (b as u128) * 2 + c as u128;
	(num / (2 * c as u128)) as usize
}

/// Lines the viewport can scroll through before the last line sits on the
/// bottom row. Scrolling further (which the editor permits) clamps here.
fn max_scroll(line_count: usize, visible: usize) -> usize {
	line_count.saturating_sub(visible)
}

/// Thumb `(top_row, len_rows)` within a `track`-row track for a viewport
/// showing `visible` rows of `line_count` lines, scrolled to `scroll_y`.
///
/// The thumb fills the track when everything fits and never shrinks below one
/// row. An empty track yields `(0, 0)`.
pub fn thumb_geometry(scroll_y: usize, line_count: usize, visible: usize, track: usize) -> (usize, usize) {
	if track == 0 {
		return (0, 0);
	}
	if line_count <= visible || line_count == 0 {
		return (0, track);
	}
	let len = (track * visible / line_count).clamp(1, track);
	let free = track - len;
	let max = max_scroll(line_count, visible);
	let top = mul_div_round(scroll_y.min(max), free, max);
	(top, len)
}

/// Inverse of [`thumb_geometry`]: the `scroll_y` that puts the thumb's top at
/// `top` (clamped to the free range).
pub fn scroll_for_thumb_top(top: usize, line_count: usize, visible: usize, track: usize) -> usize {
	let (_, len) = thumb_geometry(0, line_count, visible, track);
	let free = track.saturating_sub(len);
	let max = max_scroll(line_count, visible);
	if free == 0 || max == 0 {
		return 0;
	}
	mul_div_round(top.min(free), max, free)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::editor::Editor;

	fn editor_with_lines(n: usize, mode: &str) -> Editor {
		let mut e = Editor::new();
		e.terminal_width = 40;
		e.terminal_height = 10;
		e.show_help = false;
		e.config.wrap_lines = false;
		e.config.line_numbers = false;
		e.config.scroll_off = 0;
		e.config.scrollbar = mode.to_string();
		let body: Vec<String> = (0..n).map(|i| format!("line{i}")).collect();
		e.buffer_mut().insert_str(0, &body.join("\n"));
		e
	}

	/// Characters in the rightmost column of every text row after one frame.
	fn last_column(e: &mut Editor) -> Vec<char> {
		let mut out: Vec<u8> = Vec::new();
		crate::render::render(e, &mut out).unwrap();
		let screen = e.last_screen.as_ref().unwrap();
		let w = screen.width as usize;
		let text_rows = screen.height.saturating_sub(1) as usize;
		(0..text_rows).map(|y| screen.grid[y * w + (w - 1)].ch).collect()
	}

	/// Split a column into (leading thumb run, trailing track run) lengths.
	fn runs(col: &[char]) -> (usize, usize) {
		let thumb = col.iter().take_while(|&&c| c == THUMB).count();
		let track = col[thumb..].iter().take_while(|&&c| c == TRACK).count();
		(thumb, track)
	}

	#[test]
	fn always_mode_paints_thumb_then_track() {
		// render() syncs size from the real tty when one exists, so assert on
		// shape (thumb at top, track below) rather than on exact row counts.
		let mut e = editor_with_lines(1000, "always");
		let col = last_column(&mut e);
		let (thumb, track) = runs(&col);
		assert!(thumb >= 1 && thumb < col.len(), "{col:?}");
		assert_eq!(thumb + track, col.len(), "{col:?}");
	}

	#[test]
	fn always_mode_thumb_reaches_bottom_when_scrolled_to_end() {
		let mut e = editor_with_lines(1000, "always");
		e.buffer_mut().scroll_y = 999;
		e.pin_viewport = true; // like a wheel pan: render must not yank back to the cursor
		let col = last_column(&mut e);
		assert_eq!(*col.last().unwrap(), THUMB, "{col:?}");
		assert_eq!(col[0], TRACK, "{col:?}");
	}

	#[test]
	fn cursor_at_end_of_exactly_full_wrapped_row_sits_after_last_char() {
		// The only slot after a full row's last character is the scrollbar
		// column. Clamping short of it drew the cursor over the last character,
		// so Right from there looked like a dead keypress.
		let mut e = editor_with_lines(0, "always");
		e.config.wrap_lines = true;
		let _ = last_column(&mut e); // syncs terminal size from the tty if any
		let taw = e.text_area_width();
		e.buffer_mut().insert_str(0, &"x".repeat(taw));
		e.buffer_mut().cursors.set_cursor(0, taw);
		let _ = last_column(&mut e);
		let screen = e.last_screen.as_ref().unwrap();
		assert_eq!(e.line_len_no_newline(0), taw, "precondition: row exactly full");
		assert_eq!(
			screen.term_cursor_x,
			screen.width - 1,
			"end-of-row cursor must sit just after the last character"
		);
	}

	#[test]
	fn none_mode_leaves_column_to_text() {
		let mut e = editor_with_lines(100, "none");
		let col = last_column(&mut e);
		assert!(col.iter().all(|&c| c == ' '), "{col:?}");
	}

	#[test]
	fn scrolling_mode_blank_until_viewport_moves() {
		let mut e = editor_with_lines(100, "scrolling");
		let col = last_column(&mut e);
		assert!(col.iter().all(|&c| c == ' '), "{col:?}");
		e.buffer_mut().scroll_y = 5;
		e.pin_viewport = true;
		let col = last_column(&mut e);
		assert!(col.contains(&THUMB), "{col:?}");
	}

	#[test]
	fn thumb_fills_track_when_everything_fits() {
		assert_eq!(thumb_geometry(0, 5, 10, 10), (0, 10));
		assert_eq!(thumb_geometry(0, 0, 10, 10), (0, 10));
	}

	#[test]
	fn thumb_is_proportional_with_min_one_row() {
		// 100 lines, 10 visible, 10-row track → 1 row thumb.
		assert_eq!(thumb_geometry(0, 100, 10, 10), (0, 1));
		// 20 lines, 10 visible, 10-row track → 5 row thumb.
		assert_eq!(thumb_geometry(0, 20, 10, 10), (0, 5));
		// Huge file never rounds the thumb down to zero.
		assert_eq!(thumb_geometry(0, 1_000_000, 10, 10).1, 1);
	}

	#[test]
	fn thumb_top_tracks_scroll_position() {
		// 100 lines, 10 visible → max_scroll 90. Track 10, thumb 1 → 9 free rows.
		assert_eq!(thumb_geometry(0, 100, 10, 10).0, 0);
		assert_eq!(thumb_geometry(44, 100, 10, 10).0, 4);
		assert_eq!(thumb_geometry(50, 100, 10, 10).0, 5);
		assert_eq!(thumb_geometry(90, 100, 10, 10).0, 9);
		// Scrolled past the "last line at bottom" point still clamps to the end.
		assert_eq!(thumb_geometry(99, 100, 10, 10).0, 9);
	}

	#[test]
	fn empty_track_is_empty() {
		assert_eq!(thumb_geometry(3, 100, 10, 0), (0, 0));
	}

	#[test]
	fn scroll_for_thumb_top_hits_ends() {
		assert_eq!(scroll_for_thumb_top(0, 100, 10, 10), 0);
		assert_eq!(scroll_for_thumb_top(9, 100, 10, 10), 90);
		// Past the free range clamps to max_scroll.
		assert_eq!(scroll_for_thumb_top(40, 100, 10, 10), 90);
		// Nothing to scroll.
		assert_eq!(scroll_for_thumb_top(3, 5, 10, 10), 0);
	}

	#[test]
	fn thumb_top_roundtrips_through_scroll() {
		for (lines, visible, track) in [(100, 10, 10), (1000, 24, 23), (37, 10, 10), (5000, 40, 39)] {
			let (_, len) = thumb_geometry(0, lines, visible, track);
			for top in 0..=(track - len) {
				let s = scroll_for_thumb_top(top, lines, visible, track);
				assert_eq!(
					thumb_geometry(s, lines, visible, track).0,
					top,
					"lines={lines} visible={visible} track={track} top={top} -> scroll {s}"
				);
			}
		}
	}
}

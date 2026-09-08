//! Scrollbar visibility state ("scrolling" auto-hide) and drag tracking.

use std::time::{Duration, Instant};

use crate::config::ScrollbarMode;
use crate::editor::Editor;
use crate::render::overlay_rows_for;
use crate::render::scrollbar::{scroll_for_thumb_top, thumb_geometry};

/// How long a `"scrolling"` bar stays visible after the viewport stops moving.
pub(crate) const SCROLLBAR_LINGER: Duration = Duration::from_secs(2);

impl Editor {
	/// Rows the scrollbar track spans: text rows not covered by the status bar
	/// or overlays. Uses the editor's cached size (mouse hit-testing must not
	/// query the terminal).
	pub(crate) fn scrollbar_track_rows(&self) -> usize {
		let overlay = overlay_rows_for(self, self.terminal_width, self.terminal_height);
		self.terminal_height.saturating_sub(1 + overlay) as usize
	}

	/// Thumb `(top, len)` for the current viewport.
	pub(crate) fn scrollbar_thumb(&self) -> (usize, usize) {
		let track = self.scrollbar_track_rows();
		thumb_geometry(self.buffer().scroll_y, self.buffer().line_count(), track, track)
	}

	/// True when `(col, row)` lands on the scrollbar column within the track.
	pub(crate) fn scrollbar_hit(&self, col: u16, row: u16) -> bool {
		self.scrollbar_columns() > 0
			&& self.terminal_width > 0
			&& col == self.terminal_width - 1
			&& (row as usize) < self.scrollbar_track_rows()
	}

	/// Start a scrollbar drag at `row`. Clicking the thumb grabs it in place;
	/// clicking the track centres the thumb on the click first.
	pub(crate) fn scrollbar_drag_start(&mut self, row: u16) {
		let row = row as usize;
		let (top, len) = self.scrollbar_thumb();
		let offset = if row >= top && row < top + len {
			row - top
		} else {
			let grab = len / 2;
			self.scrollbar_scroll_to_thumb_top(row.saturating_sub(grab));
			grab
		};
		self.scrollbar_drag = Some(offset);
		self.pin_viewport = true;
	}

	/// Continue an active drag: move the thumb so the grab point follows `row`.
	/// Returns false when no drag is active.
	pub(crate) fn scrollbar_drag_to(&mut self, row: u16) -> bool {
		let Some(offset) = self.scrollbar_drag else {
			return false;
		};
		self.scrollbar_scroll_to_thumb_top((row as usize).saturating_sub(offset));
		self.pin_viewport = true;
		true
	}

	pub(crate) fn scrollbar_drag_end(&mut self) {
		self.scrollbar_drag = None;
	}

	fn scrollbar_scroll_to_thumb_top(&mut self, top: usize) {
		let track = self.scrollbar_track_rows();
		let line_count = self.buffer().line_count();
		let scroll_y = scroll_for_thumb_top(top, line_count, track, track);
		let buf = self.buffer_mut();
		buf.scroll_y = scroll_y;
		buf.scroll_vrow = 0;
	}

	/// Pointer hover: resting on the scrollbar column counts as scroll activity
	/// in `"scrolling"` mode, so the bar appears and lingers after the pointer
	/// leaves. Other modes ignore hover.
	pub(crate) fn scrollbar_hover_at(&mut self, col: u16, row: u16, now: Instant) {
		if self.config.scrollbar_mode() == ScrollbarMode::Scrolling && self.scrollbar_hit(col, row) {
			self.scrollbar_last_scroll = Some(now);
		}
	}

	/// Record viewport movement. Called once per frame; re-stamps whenever the
	/// scroll position (or active buffer) differs from the last call, so wheel,
	/// keyboard, search, goto, and drag all count as scrolling.
	pub(crate) fn note_scroll_activity(&mut self, now: Instant) {
		let buf = self.buffer();
		let pos = (self.active_buffer, buf.scroll_y, buf.scroll_vrow);
		if pos != self.scrollbar_seen_pos {
			self.scrollbar_seen_pos = pos;
			self.scrollbar_last_scroll = Some(now);
		}
	}

	/// Whether the scrollbar should be painted at `now`.
	pub(crate) fn scrollbar_visible_at(&self, now: Instant) -> bool {
		match self.config.scrollbar_mode() {
			ScrollbarMode::None => false,
			ScrollbarMode::Always => true,
			ScrollbarMode::Scrolling => {
				self.scrollbar_drag.is_some() || self.scrollbar_hide_in(now).is_some()
			}
		}
	}

	/// True when the last frame showed the bar but it should now be hidden, so
	/// the main loop must render one more frame without waiting for input.
	pub(crate) fn scrollbar_needs_redraw(&self, now: Instant) -> bool {
		self.scrollbar_drawn_visible && !self.scrollbar_visible_at(now)
	}

	/// Time until a `"scrolling"` bar auto-hides, or `None` when no timed hide
	/// is pending (other modes, nothing stamped, drag in progress, or expired).
	pub(crate) fn scrollbar_hide_in(&self, now: Instant) -> Option<Duration> {
		if self.config.scrollbar_mode() != ScrollbarMode::Scrolling || self.scrollbar_drag.is_some() {
			return None;
		}
		let deadline = self.scrollbar_last_scroll? + SCROLLBAR_LINGER;
		if now < deadline {
			Some(deadline - now)
		} else {
			None
		}
	}
}

#[cfg(test)]
mod tests {
	use crate::editor::Editor;
	use std::time::{Duration, Instant};

	fn editor(mode: &str) -> Editor {
		let mut e = Editor::new();
		e.terminal_width = 40;
		e.terminal_height = 10;
		e.config.scrollbar = mode.to_string();
		e
	}

	#[test]
	fn none_mode_is_never_visible() {
		let mut e = editor("none");
		let t0 = Instant::now();
		e.buffer_mut().scroll_y = 3;
		e.note_scroll_activity(t0);
		assert!(!e.scrollbar_visible_at(t0));
	}

	#[test]
	fn always_mode_is_visible_without_activity() {
		let e = editor("always");
		assert!(e.scrollbar_visible_at(Instant::now()));
	}

	#[test]
	fn scrolling_mode_hidden_until_viewport_moves() {
		let mut e = editor("scrolling");
		let t0 = Instant::now();
		e.note_scroll_activity(t0);
		assert!(!e.scrollbar_visible_at(t0), "no scroll yet → hidden");

		e.buffer_mut().scroll_y = 3;
		e.note_scroll_activity(t0);
		assert!(e.scrollbar_visible_at(t0));
		assert!(e.scrollbar_visible_at(t0 + Duration::from_millis(1900)));
		assert!(!e.scrollbar_visible_at(t0 + Duration::from_millis(2100)));
	}

	#[test]
	fn scrolling_mode_restamps_on_each_move() {
		let mut e = editor("scrolling");
		let t0 = Instant::now();
		e.buffer_mut().scroll_y = 1;
		e.note_scroll_activity(t0);
		let t1 = t0 + Duration::from_millis(1500);
		e.buffer_mut().scroll_y = 2;
		e.note_scroll_activity(t1);
		assert!(e.scrollbar_visible_at(t1 + Duration::from_millis(1900)));
		assert!(!e.scrollbar_visible_at(t1 + Duration::from_millis(2100)));
	}

	#[test]
	fn scrolling_mode_stays_visible_while_dragging() {
		let mut e = editor("scrolling");
		let t0 = Instant::now();
		e.scrollbar_drag = Some(0);
		assert!(e.scrollbar_visible_at(t0 + Duration::from_secs(60)));
	}

	#[test]
	fn hovering_the_column_shows_a_scrolling_bar_and_lingers() {
		let mut e = editor("scrolling");
		let t0 = Instant::now();
		let x = e.terminal_width - 1;
		e.scrollbar_hover_at(x - 1, 3, t0);
		assert!(!e.scrollbar_visible_at(t0), "hover off the column does nothing");
		e.scrollbar_hover_at(x, 3, t0);
		assert!(e.scrollbar_visible_at(t0));
		assert!(e.scrollbar_visible_at(t0 + Duration::from_millis(1900)));
		assert!(!e.scrollbar_visible_at(t0 + Duration::from_millis(2100)));
	}

	#[test]
	fn hovering_below_the_track_does_nothing() {
		let mut e = editor("scrolling");
		let t0 = Instant::now();
		let x = e.terminal_width - 1;
		let status_row = e.terminal_height - 1;
		e.scrollbar_hover_at(x, status_row, t0);
		assert!(!e.scrollbar_visible_at(t0));
	}

	#[test]
	fn hover_does_not_affect_none_mode() {
		let mut e = editor("none");
		let t0 = Instant::now();
		e.scrollbar_hover_at(e.terminal_width - 1, 0, t0);
		assert!(!e.scrollbar_visible_at(t0));
	}

	#[test]
	fn needs_redraw_once_a_drawn_bar_expires() {
		let mut e = editor("scrolling");
		let t0 = Instant::now();
		assert!(!e.scrollbar_needs_redraw(t0), "nothing drawn, nothing to hide");
		e.buffer_mut().scroll_y = 1;
		e.note_scroll_activity(t0);
		e.scrollbar_drawn_visible = true;
		assert!(!e.scrollbar_needs_redraw(t0 + Duration::from_secs(1)));
		assert!(e.scrollbar_needs_redraw(t0 + Duration::from_secs(3)));
		// An "always" bar never expires.
		let mut a = editor("always");
		a.scrollbar_drawn_visible = true;
		assert!(!a.scrollbar_needs_redraw(t0 + Duration::from_secs(3)));
	}

	#[test]
	fn hide_deadline_only_while_a_scrolling_bar_is_showing() {
		let mut e = editor("scrolling");
		let t0 = Instant::now();
		assert_eq!(e.scrollbar_hide_in(t0), None);
		e.buffer_mut().scroll_y = 1;
		e.note_scroll_activity(t0);
		let left = e.scrollbar_hide_in(t0 + Duration::from_millis(500)).unwrap();
		assert!(left > Duration::from_millis(1400) && left <= Duration::from_millis(1500), "{left:?}");
		assert_eq!(e.scrollbar_hide_in(t0 + Duration::from_secs(3)), None);

		let a = editor("always");
		assert_eq!(a.scrollbar_hide_in(t0), None);
	}
}

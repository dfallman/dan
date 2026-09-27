//! Detect open files changed on disk by other programs and fold the change
//! into the buffer. Spec:
//! docs/superpowers/specs/2026-09-27-external-change-detection-design.md

use std::time::{Duration, Instant};

use crate::buffer::disk::{read_decoded, DiskStamp, PendingConflict};
use crate::buffer::marks::MarkKind;
use crate::buffer::merge::merge3;
use crate::buffer::rope::TextRope;
use crate::editor::mode::Mode;
use crate::editor::Editor;

/// The idle main loop wakes every 500 ms; slightly less makes every wake
/// eligible.
pub(crate) const DISK_POLL_INTERVAL: Duration = Duration::from_millis(450);
/// Time budget per line diff; past it `similar` returns a coarser diff.
const DIFF_DEADLINE: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiskCheck {
	/// Untitled, or stamp unchanged.
	Unchanged,
	/// Stamp moved but the text is what we already have.
	Touched,
	/// Disk change applied, no conflicts.
	Merged,
	/// Non-conflicting parts applied; a conflict awaits the user.
	Conflict,
	/// File vanished (reported once).
	Deleted,
	/// Changed but unreadable (binary, too large, permissions).
	Unreadable,
}

impl Editor {
	/// Check every open file, at most once per `DISK_POLL_INTERVAL`.
	/// Returns true if anything needs a redraw.
	pub(crate) fn poll_disk_changes(&mut self, now: Instant) -> bool {
		if !self.config.watch_files
			|| now.saturating_duration_since(self.last_disk_poll) < DISK_POLL_INTERVAL
		{
			return false;
		}
		self.last_disk_poll = now;
		let mut changed = false;
		for i in 0..self.buffers.len() {
			changed |= self.check_buffer_on_disk(i) != DiskCheck::Unchanged;
		}
		changed
	}

	pub(crate) fn check_buffer_on_disk(&mut self, idx: usize) -> DiskCheck {
		let Some(path) = self.buffers[idx].file_path.clone() else {
			return DiskCheck::Unchanged;
		};
		let stamp = match DiskStamp::of(&path) {
			Ok(s) => s,
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
				let buf = &mut self.buffers[idx];
				if buf.disk_missing {
					return DiskCheck::Unchanged;
				}
				if buf.disk_text.is_none() {
					// Never on disk (a new file not yet saved): nothing was deleted.
					buf.disk_missing = true;
					return DiskCheck::Unchanged;
				}
				buf.disk_missing = true;
				buf.disk_stamp = None;
				buf.dirty = true;
				let name = buf.display_name();
				self.set_status(format!("{name}: file deleted on disk — save to recreate it"));
				return DiskCheck::Deleted;
			}
			// Transient (permissions, network share): try again next poll.
			Err(_) => return DiskCheck::Unchanged,
		};
		if self.buffers[idx].disk_stamp.as_ref() == Some(&stamp) {
			return DiskCheck::Unchanged;
		}
		let (theirs, encoding) = match read_decoded(&path) {
			Ok(r) => r,
			Err(e) => {
				self.buffers[idx].disk_stamp = Some(stamp);
				let name = self.buffers[idx].display_name();
				self.set_status(format!("{name} changed on disk but was not reloaded: {e}"));
				return DiskCheck::Unreadable;
			}
		};

		let buf = &mut self.buffers[idx];
		buf.disk_missing = false;
		buf.disk_stamp = Some(stamp);
		if buf.disk_text.as_ref().is_some_and(|t| t.eq_str(&theirs)) {
			return DiskCheck::Touched;
		}
		// A newer disk version supersedes an unanswered conflict; the base
		// was never advanced, so re-merging from it is correct.
		buf.pending_conflict = None;
		let was_clean = !buf.dirty;
		let base = buf.disk_base.as_ref().map(|b| b.to_string_full()).unwrap_or_default();
		let mine = buf.text.to_string_full();
		let merge = merge3(&base, &mine, &theirs, Instant::now() + DIFF_DEADLINE);
		buf.apply_disk_changes(&merge.auto, MarkKind::External);
		if was_clean {
			buf.encoding = encoding;
		}
		let name = buf.display_name();
		let theirs = TextRope::from_str(&theirs);
		buf.disk_text = Some(theirs.clone());

		if merge.conflicts.is_empty() {
			buf.dirty = !buf.text.same_text(&theirs);
			buf.disk_base = Some(theirs);
			self.set_status(format!("{name} changed on disk — updated"));
			return DiskCheck::Merged;
		}
		let n = merge.conflicts.len();
		buf.disk_base = Some(TextRope::from_str(&merge.resolved_base));
		buf.pending_conflict = Some(PendingConflict {
			version: buf.version,
			regions: merge.conflicts,
			theirs,
		});
		self.set_status(format!("{name}: {n} section(s) changed on disk and in your buffer"));
		DiskCheck::Conflict
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Resolution {
	Mine,
	Theirs,
	/// Keep mine, but mark the conflicting lines until save.
	Later,
}

impl Editor {
	/// Show the conflict prompt for the active buffer once the user is back
	/// to plain editing. Returns true if the prompt was raised.
	pub(crate) fn raise_conflict_prompt(&mut self) -> bool {
		if self.mode == Mode::Editing && self.buffer().pending_conflict.is_some() {
			self.mode = Mode::ConfirmExternalConflict;
			return true;
		}
		false
	}

	pub(crate) fn cmd_external_resolve(&mut self, choice: Resolution) {
		self.mode = Mode::Editing;
		self.clear_status();
		let buf = self.buffer_mut();
		let Some(p) = buf.pending_conflict.take() else {
			return;
		};
		if p.version != buf.version {
			// Edited under the prompt: re-merge from the unchanged base.
			buf.disk_stamp = None;
			return;
		}
		match choice {
			Resolution::Theirs => buf.apply_disk_changes(&p.regions, MarkKind::External),
			Resolution::Mine => {}
			Resolution::Later => buf.mark_ranges(&p.regions, MarkKind::Conflict),
		}
		buf.dirty = !buf.text.same_text(&p.theirs);
		buf.disk_base = Some(p.theirs);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::buffer::merge::Change;
	use crate::editor::commands::Command;
	use crate::editor::mode::Mode;
	use std::path::{Path, PathBuf};

	fn temp_file(name: &str, content: &str) -> PathBuf {
		let p = std::env::temp_dir().join(format!("dan_ext_{}_{}", std::process::id(), name));
		std::fs::write(&p, content).unwrap();
		p
	}

	fn open(path: &Path) -> Editor {
		let mut e = Editor::new();
		e.open_file(path).unwrap();
		e
	}

	/// Rewrite the file and force the next check to read it, independent of
	/// the file system's mtime granularity.
	fn rewrite(e: &mut Editor, path: &Path, content: &str) {
		std::fs::write(path, content).unwrap();
		let i = e.active_buffer;
		e.buffers[i].disk_stamp = None;
	}

	fn check(e: &mut Editor) -> DiskCheck {
		let i = e.active_buffer;
		e.check_buffer_on_disk(i)
	}

	fn text(e: &Editor) -> String {
		e.buffer().text.to_string_full()
	}

	#[test]
	fn clean_reload_applies_and_stays_clean() {
		let p = temp_file("clean.txt", "a\nb\nc\n");
		let mut e = open(&p);
		e.buffer_mut().cursors.set_cursor(2, 1);
		rewrite(&mut e, &p, "a\nB\nc\nd\n");
		assert_eq!(check(&mut e), DiskCheck::Merged);
		assert_eq!(text(&e), "a\nB\nc\nd\n");
		assert!(!e.buffer().dirty);
		let c = e.buffer().cursors.cursor();
		assert_eq!((c.line, c.col), (2, 1));
		let marked: Vec<usize> = e.buffer().change_marks.keys().copied().collect();
		assert_eq!(marked, vec![1, 3]);
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn dirty_non_overlapping_merge_keeps_user_edits() {
		let p = temp_file("dirty.txt", "a\nb\nc\nd\n");
		let mut e = open(&p);
		e.buffer_mut().insert_str(0, "X");
		e.buffer_mut().commit_edits();
		rewrite(&mut e, &p, "a\nb\nc\nD\n");
		assert_eq!(check(&mut e), DiskCheck::Merged);
		assert_eq!(text(&e), "Xa\nb\nc\nD\n");
		assert!(e.buffer().dirty);
		assert!(e.buffer().disk_base.as_ref().unwrap().eq_str("a\nb\nc\nD\n"));
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn touched_file_is_not_reloaded() {
		let p = temp_file("touch.txt", "a\n");
		let mut e = open(&p);
		let v = e.buffer().version;
		rewrite(&mut e, &p, "a\n");
		assert_eq!(check(&mut e), DiskCheck::Touched);
		assert_eq!(e.buffer().version, v);
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn own_save_is_not_a_change() {
		let p = temp_file("own.txt", "a\n");
		let mut e = open(&p);
		e.buffer_mut().insert_str(0, "b\n");
		let cfg = e.config.clone();
		e.buffer_mut().save(&cfg).unwrap();
		assert_eq!(check(&mut e), DiskCheck::Unchanged);
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn rename_over_is_detected() {
		let p = temp_file("rename.txt", "a\n");
		let mut e = open(&p);
		let tmp = temp_file("rename.tmp", "a\nfrom agent\n");
		std::fs::rename(&tmp, &p).unwrap();
		assert_eq!(check(&mut e), DiskCheck::Merged);
		assert_eq!(text(&e), "a\nfrom agent\n");
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn deleted_file_is_reported_once_and_recreation_merges() {
		let p = temp_file("deleted.txt", "a\n");
		let mut e = open(&p);
		std::fs::remove_file(&p).unwrap();
		assert_eq!(check(&mut e), DiskCheck::Deleted);
		assert!(e.buffer().dirty);
		assert_eq!(check(&mut e), DiskCheck::Unchanged);
		std::fs::write(&p, "a\nb\n").unwrap();
		assert_eq!(check(&mut e), DiskCheck::Merged);
		assert_eq!(text(&e), "a\nb\n");
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn external_change_is_one_undo_step() {
		let p = temp_file("undo.txt", "a\nb\nc\n");
		let mut e = open(&p);
		rewrite(&mut e, &p, "A\nb\nC\n");
		check(&mut e);
		e.buffer_mut().undo();
		assert_eq!(text(&e), "a\nb\nc\n");
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn conflict_applies_auto_and_stores_pending() {
		let p = temp_file("conflict.txt", "a\nb\nc\nd\n");
		let mut e = open(&p);
		e.buffer_mut().delete_range(2, 3);
		e.buffer_mut().insert_str(2, "MINE");
		e.buffer_mut().commit_edits();
		rewrite(&mut e, &p, "a\nDISK\nc\nD\n");
		assert_eq!(check(&mut e), DiskCheck::Conflict);
		assert_eq!(text(&e), "a\nMINE\nc\nD\n");
		let pc = e.buffer().pending_conflict.as_ref().unwrap();
		assert_eq!(pc.regions, vec![Change { mine: 2..7, theirs: "DISK\n".into() }]);
		assert!(e.buffer().disk_base.as_ref().unwrap().eq_str("a\nb\nc\nD\n"), "resolved base: auto applied, conflict kept at base");
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn unreadable_disk_content_leaves_buffer() {
		let p = temp_file("binary.txt", "a\n");
		let mut e = open(&p);
		std::fs::write(&p, b"a\0b").unwrap();
		e.buffer_mut().disk_stamp = None;
		assert_eq!(check(&mut e), DiskCheck::Unreadable);
		assert_eq!(text(&e), "a\n");
		assert_eq!(check(&mut e), DiskCheck::Unchanged, "not retried until it changes again");
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn cursor_past_shortened_file_is_clamped() {
		let p = temp_file("short.txt", "a\nb\nc\nd\ne\n");
		let mut e = open(&p);
		e.buffer_mut().cursors.set_cursor(4, 1);
		rewrite(&mut e, &p, "a\n");
		assert_eq!(check(&mut e), DiskCheck::Merged);
		let c = e.buffer().cursors.cursor();
		assert!(c.line < e.buffer().text.len_lines());
		let mut out: Vec<u8> = Vec::new();
		crate::render::render(&mut e, &mut out).unwrap();
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn background_buffer_is_merged() {
		let p1 = temp_file("bg1.txt", "one\n");
		let p2 = temp_file("bg2.txt", "two\n");
		let mut e = open(&p1);
		e.open_file(&p2).unwrap();
		let bg = e.buffers.iter().position(|b| b.file_path.as_deref() == Some(p1.as_path())).unwrap();
		std::fs::write(&p1, "one changed\n").unwrap();
		e.buffers[bg].disk_stamp = None;
		assert_eq!(e.check_buffer_on_disk(bg), DiskCheck::Merged);
		assert!(e.buffers[bg].text.eq_str("one changed\n"));
		assert_eq!(text(&e), "two\n");
		std::fs::remove_file(&p1).ok();
		std::fs::remove_file(&p2).ok();
	}

	#[test]
	fn poll_is_throttled_and_obeys_watch_files() {
		let p = temp_file("poll.txt", "a\n");
		let mut e = open(&p);
		rewrite(&mut e, &p, "b\n");
		let last = e.last_disk_poll;
		assert!(!e.poll_disk_changes(last), "within the interval");
		e.config.watch_files = false;
		assert!(!e.poll_disk_changes(last + DISK_POLL_INTERVAL));
		assert_eq!(text(&e), "a\n");
		e.config.watch_files = true;
		assert!(e.poll_disk_changes(last + DISK_POLL_INTERVAL));
		assert_eq!(text(&e), "b\n");
		std::fs::remove_file(&p).ok();
	}

	fn conflicted(name: &str) -> (Editor, PathBuf) {
		let p = temp_file(name, "a\nb\nc\nd\n");
		let mut e = open(&p);
		e.buffer_mut().delete_range(2, 3);
		e.buffer_mut().insert_str(2, "MINE");
		e.buffer_mut().commit_edits();
		rewrite(&mut e, &p, "a\nDISK\nc\nD\n");
		assert_eq!(check(&mut e), DiskCheck::Conflict);
		(e, p)
	}

	#[test]
	fn conflict_raises_prompt_only_while_editing() {
		let (mut e, p) = conflicted("prompt.txt");
		e.mode = Mode::Searching;
		assert!(!e.raise_conflict_prompt());
		assert_eq!(e.mode, Mode::Searching);
		e.mode = Mode::Editing;
		assert!(e.raise_conflict_prompt());
		assert_eq!(e.mode, Mode::ConfirmExternalConflict);
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn take_theirs_replaces_and_cleans() {
		let (mut e, p) = conflicted("theirs.txt");
		e.raise_conflict_prompt();
		e.execute(Command::ExternalTakeTheirs);
		assert_eq!(e.mode, Mode::Editing);
		assert_eq!(text(&e), "a\nDISK\nc\nD\n");
		assert!(!e.buffer().dirty);
		assert_eq!(e.buffer().change_marks.get(&1), Some(&MarkKind::External));
		assert!(e.buffer().pending_conflict.is_none());
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn keep_mine_keeps_text_and_advances_base() {
		let (mut e, p) = conflicted("mine.txt");
		e.raise_conflict_prompt();
		e.execute(Command::ExternalKeepMine);
		assert_eq!(text(&e), "a\nMINE\nc\nD\n");
		assert!(e.buffer().dirty);
		assert_eq!(e.buffer().change_marks.get(&1), None);
		assert!(e.buffer().disk_base.as_ref().unwrap().eq_str("a\nDISK\nc\nD\n"));
		e.buffer_mut().disk_stamp = None;
		assert_eq!(check(&mut e), DiskCheck::Touched, "same conflict is not raised again");
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn decide_later_marks_conflict_lines() {
		let (mut e, p) = conflicted("later.txt");
		e.raise_conflict_prompt();
		e.execute(Command::ExternalDecideLater);
		assert_eq!(e.mode, Mode::Editing);
		assert_eq!(text(&e), "a\nMINE\nc\nD\n");
		assert_eq!(e.buffer().change_marks.get(&1), Some(&MarkKind::Conflict));
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn conflict_prompt_waits_for_its_buffer() {
		let (mut e, p) = conflicted("bgprompt.txt");
		let other = temp_file("bgprompt_other.txt", "x\n");
		e.open_file(&other).unwrap();
		assert!(!e.raise_conflict_prompt());
		let back = e.buffers.iter().position(|b| b.file_path.as_deref() == Some(p.as_path())).unwrap();
		e.active_buffer = back;
		assert!(e.raise_conflict_prompt());
		std::fs::remove_file(&p).ok();
		std::fs::remove_file(&other).ok();
	}

	#[test]
	fn save_merges_unseen_disk_change_instead_of_overwriting() {
		let p = temp_file("race.txt", "a\nb\nc\nd\n");
		let mut e = open(&p);
		e.buffer_mut().insert_str(0, "X");
		e.buffer_mut().commit_edits();
		rewrite(&mut e, &p, "a\nb\nc\nD\n");
		e.execute(Command::Save);
		assert_eq!(std::fs::read_to_string(&p).unwrap(), "a\nb\nc\nD\n", "agent edit not overwritten");
		assert_eq!(text(&e), "Xa\nb\nc\nD\n");
		assert!(e.status_msg.as_deref().unwrap_or("").contains("save again"));
		e.execute(Command::Save);
		assert_eq!(std::fs::read_to_string(&p).unwrap(), "Xa\nb\nc\nD\n");
		std::fs::remove_file(&p).ok();
	}

	// --- final-review fixes ---

	#[test]
	fn new_unsaved_path_is_not_reported_deleted() {
		let p = std::env::temp_dir().join(format!("dan_ext_{}_never_existed.txt", std::process::id()));
		std::fs::remove_file(&p).ok();
		let mut e = Editor::new();
		e.buffer_mut().file_path = Some(p.clone());
		assert_eq!(check(&mut e), DiskCheck::Unchanged);
		assert!(!e.buffer().dirty);
		assert_eq!(e.status_msg, None);
	}

	#[test]
	fn pending_conflict_does_not_create_false_conflicts() {
		let p = temp_file("falseconf.txt", "a\nb\nc\nd\ne\nf\n");
		let mut e = open(&p);
		e.buffer_mut().insert_str(0, "X");
		e.buffer_mut().commit_edits();
		rewrite(&mut e, &p, "DISK\nb\nc\nd\ne\nF1\n");
		assert_eq!(check(&mut e), DiskCheck::Conflict);
		rewrite(&mut e, &p, "DISK\nb\nc\nd\ne\nF2\n");
		assert_eq!(check(&mut e), DiskCheck::Conflict);
		assert_eq!(text(&e), "Xa\nb\nc\nd\ne\nF2\n");
		assert_eq!(e.buffer().pending_conflict.as_ref().unwrap().regions.len(), 1, "only line 0 conflicts");
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn save_with_pending_conflict_raises_prompt_instead() {
		let (mut e, p) = conflicted("savepending.txt");
		e.execute(Command::Save);
		assert_eq!(std::fs::read_to_string(&p).unwrap(), "a\nDISK\nc\nD\n");
		assert_eq!(e.mode, Mode::ConfirmExternalConflict);
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn save_all_does_not_overwrite_pending_conflict() {
		let (mut e, p) = conflicted("saveall.txt");
		e.execute(Command::SaveAll);
		assert_eq!(std::fs::read_to_string(&p).unwrap(), "a\nDISK\nc\nD\n");
		assert!(e.buffer().pending_conflict.is_some());
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn save_and_quit_with_conflict_raises_prompt() {
		let (mut e, p) = conflicted("savequit.txt");
		e.mode = Mode::ConfirmQuit;
		e.execute(Command::SaveAndQuit);
		assert_eq!(std::fs::read_to_string(&p).unwrap(), "a\nDISK\nc\nD\n");
		assert!(!e.should_quit);
		assert_eq!(e.mode, Mode::ConfirmExternalConflict);
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn palette_close_save_does_not_overwrite_pending_conflict() {
		let (mut e, p) = conflicted("closesave.txt");
		let idx = e.active_buffer;
		e.palette.close_prompt_idx = Some(idx);
		e.execute(Command::PaletteClosePromptSave);
		assert_eq!(std::fs::read_to_string(&p).unwrap(), "a\nDISK\nc\nD\n");
		assert!(e.buffers.iter().any(|b| b.file_path.as_deref() == Some(p.as_path())), "buffer not closed");
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn trimmed_save_then_agent_edit_stays_clean() {
		let p = temp_file("trim.txt", "a\nb\nc\n");
		let mut e = open(&p);
		e.config.trim_trailing_whitespace = Some(true);
		e.buffer_mut().insert_str(1, "  ");
		e.execute(Command::Save);
		assert_eq!(std::fs::read_to_string(&p).unwrap(), "a\nb\nc\n");
		rewrite(&mut e, &p, "a\nb\nC\n");
		assert_eq!(check(&mut e), DiskCheck::Merged);
		assert!(!e.buffer().dirty);
		rewrite(&mut e, &p, "A\nb\nC\n");
		assert_eq!(check(&mut e), DiskCheck::Merged, "clean buffer never conflicts");
		assert_eq!(text(&e), "A\nb\nC\n");
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn crlf_save_then_agent_edit_is_not_a_conflict() {
		let p = temp_file("crlf.txt", "a\nb\nc\n");
		let mut e = open(&p);
		e.config.end_of_line = Some("crlf".into());
		e.buffer_mut().insert_str(0, "x");
		e.execute(Command::Save);
		assert_eq!(std::fs::read_to_string(&p).unwrap(), "xa\r\nb\r\nc\r\n");
		rewrite(&mut e, &p, "xa\r\nB\r\nc\r\n");
		assert_eq!(check(&mut e), DiskCheck::Merged);
		assert!(!e.buffer().dirty);
		std::fs::remove_file(&p).ok();
	}

	#[test]
	fn formatter_result_after_external_change_is_discarded() {
		let p = temp_file("fmt.txt", "a\n");
		let mut e = open(&p);
		let (tx, rx) = std::sync::mpsc::channel();
		let v = e.buffer().version;
		e.buffer_mut().fmt_rx = Some(rx);
		e.buffer_mut().fmt_baseline_version = Some(v);
		e.buffer_mut().is_formatting = true;
		rewrite(&mut e, &p, "b\n");
		check(&mut e);
		tx.send(Ok("formatted\n".to_string())).unwrap();
		e.poll_async_tasks();
		assert_eq!(text(&e), "b\n");
		std::fs::remove_file(&p).ok();
	}
}

//! File save/quit/recovery and path metadata commands.
use crate::editor::commands::Command;
use crate::editor::mode::Mode;
use crate::editor::Editor;

impl Editor {
	pub(crate) fn cmd_save(&mut self) {
		if self.buffer().file_path.is_none() {
			self.execute(Command::SaveAsOpen);
		} else {
			if self.disk_blocks_save(self.active_buffer) {
				return;
			}
			self.buffer_mut().commit_edits();
			let cfg = self.config.clone();
			match self.buffer_mut().save(&cfg) {
				Ok(()) => self.set_status("✓ Saved"),
				Err(e) => self.set_status(format!("Save failed: {}", e)),
			}
		}
	}

	/// Every save path calls this before writing buffer `idx`. It folds in
	/// any disk change not yet seen and refuses while a disk conflict is
	/// unanswered, so a save never overwrites another program's edit unseen.
	/// Returns true if the save must stop.
	pub(crate) fn disk_blocks_save(&mut self, idx: usize) -> bool {
		if !self.config.watch_files {
			return false;
		}
		let check = self.check_buffer_on_disk(idx);
		if self.buffers[idx].pending_conflict.is_some() {
			self.set_status("Changed on disk and in your buffer — choose ^K keep mine or ^T take theirs, then save");
			// Answer the conflict first; this also leaves a pending quit.
			if idx == self.active_buffer && matches!(self.mode, Mode::Editing | Mode::ConfirmQuit) {
				self.quit_cycle_idx = None;
				self.mode = Mode::Editing;
				self.raise_conflict_prompt();
			}
			return true;
		}
		if check == crate::editor::external::DiskCheck::Merged {
			self.set_status("File changed on disk — merged into your buffer; save again to write");
			return true;
		}
		false
	}

	pub(crate) fn cmd_quit(&mut self) {
		// Find the first dirty buffer and prompt for it; if none, exit.
		let dirty_idx = self.buffers.iter().position(|b| b.dirty);
		match dirty_idx {
			None => self.should_quit = true,
			Some(i) => {
				self.active_buffer = i;
				self.quit_cycle_idx = Some(i);
				self.mode = Mode::ConfirmQuit;
			}
		}
	}

	pub(crate) fn cmd_force_quit(&mut self) {
		// Discard the active buffer's dirty state and advance the cycle.
		// Remove its swap too so the discarded content isn't offered for
		// recovery on the next open (P4-M).
		if let Some(swp) = self.buffer().swp_path.clone() {
			crate::recovery::cleanup_swap(&swp);
		}
		self.buffer_mut().dirty = false;
		self.advance_quit_cycle();
	}

	pub(crate) fn cmd_force_quit_all(&mut self) {
		// Unconditional exit — no questions, no cycle, no save.
		self.should_quit = true;
	}

	pub(crate) fn cmd_save_and_quit(&mut self) {
		if self.buffer().file_path.is_none() {
			self.execute(Command::SaveAsOpen);
		} else {
			if self.disk_blocks_save(self.active_buffer) {
				return;
			}
			self.buffer_mut().commit_edits();
			let cfg = self.config.clone();
			match self.buffer_mut().save(&cfg) {
				Ok(()) => self.advance_quit_cycle(),
				Err(e) => {
					// Leave mode as ConfirmQuit so the user can retry or cancel.
					self.set_status(format!("Save failed: {}", e));
				}
			}
		}
	}

	pub(crate) fn cmd_cancel_quit(&mut self) {
		self.quit_cycle_idx = None;
		self.mode = Mode::Editing;
		self.clear_status();
	}

	pub(crate) fn cmd_recover_swap_accept(&mut self) {
		if let Some(swp) = self.buffer().swp_path.clone() {
			if let Some(payload) = crate::recovery::check_recovery(&swp) {
				let len = self.buffer().text.len_chars();
				self.buffer_mut().delete_range(0, len);
				self.buffer_mut().insert_str(0, &payload);
				self.buffer_mut().mark_mutated();
			}
			crate::recovery::cleanup_swap(&swp);
		}
		self.mode = Mode::Editing;
		self.clear_status();
		self.promote_info_banner();
	}

	pub(crate) fn cmd_recover_swap_decline(&mut self) {
		if let Some(swp) = self.buffer().swp_path.clone() {
			crate::recovery::cleanup_swap(&swp);
		}
		self.mode = Mode::Editing;
		self.clear_status();
		self.promote_info_banner();
	}

	pub(crate) fn cmd_copy_path_abs(&mut self) {
		match self.buffer().file_path.clone() {
			Some(p) => {
				let s = std::fs::canonicalize(&p).unwrap_or(p).display().to_string();
				if let Ok(mut cb) = arboard::Clipboard::new() {
					let _ = cb.set_text(s.clone());
					self.set_status(format!("Copied: {}", s));
				} else {
					self.set_status("Clipboard unavailable");
				}
			}
			None => self.set_status("No file path to copy"),
		}
	}

	pub(crate) fn cmd_copy_path_rel(&mut self) {
		match self.buffer().file_path.clone() {
			Some(p) => {
				let s = p.strip_prefix(&self.project_root).unwrap_or(&p).display().to_string();
				if let Ok(mut cb) = arboard::Clipboard::new() {
					let _ = cb.set_text(s.clone());
					self.set_status(format!("Copied: {}", s));
				} else {
					self.set_status("Clipboard unavailable");
				}
			}
			None => self.set_status("No file path to copy"),
		}
	}

	pub(crate) fn cmd_reveal_in_finder(&mut self) {
		let Some(p) = self.buffer().file_path.clone() else {
			self.set_status("No file to reveal"); return;
		};
		let result = if cfg!(target_os = "macos") {
			std::process::Command::new("open").args(["-R", &p.display().to_string()]).spawn()
		} else if cfg!(target_os = "linux") {
			let parent = p.parent().unwrap_or(std::path::Path::new("."));
			std::process::Command::new("xdg-open").arg(parent).spawn()
		} else if cfg!(target_os = "windows") {
			std::process::Command::new("explorer").args(["/select,", &p.display().to_string()]).spawn()
		} else {
			self.set_status("Reveal not supported on this platform"); return;
		};
		match result {
			Ok(_) => self.set_status("Revealed in file manager"),
			Err(e) => self.set_status(format!("Reveal failed: {}", e)),
		}
	}

	pub(crate) fn cmd_open_containing_folder(&mut self) {
		let Some(p) = self.buffer().file_path.clone() else {
			self.set_status("No file to open folder for"); return;
		};
		let parent = p.parent().unwrap_or(std::path::Path::new(".")).to_owned();
		let cmd = if cfg!(target_os = "macos") { "open" }
		          else if cfg!(target_os = "windows") { "explorer" }
		          else { "xdg-open" };
		match std::process::Command::new(cmd).arg(&parent).spawn() {
			Ok(_) => self.set_status(format!("Opened {}", parent.display())),
			Err(e) => self.set_status(format!("Open failed: {}", e)),
		}
	}

	pub(crate) fn cmd_clear_change_marks(&mut self) {
		self.buffer_mut().clear_change_marks();
		self.set_status("Change marks cleared");
	}

	pub(crate) fn cmd_show_buffer_info(&mut self) {
		let path = self.buffer().file_path.as_ref()
			.map(|p| p.display().to_string())
			.unwrap_or_else(|| "[Untitled]".into());
		let lines = self.buffer().text.len_lines();
		let bytes = self.buffer().text.to_string_full().len();
		let enc = self.buffer().encoding.name();
		let eol = self.config.end_of_line.as_deref().unwrap_or("auto");
		self.set_status(format!("{} · {} lines · {} bytes · {} · EOL: {}", path, lines, bytes, enc, eol));
	}
}

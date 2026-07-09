//! Command palette and multi-buffer management.
use crate::editor::mode::Mode;
use crate::editor::Editor;

impl Editor {
	pub(crate) fn cmd_palette_open(&mut self) {
		// Only open from plain Editing — don't clobber another prompt.
		if matches!(self.mode, Mode::Editing) {
			self.mode = Mode::Palette;
			self.open_palette();
		}
	}

	pub(crate) fn cmd_palette_cancel(&mut self) {
		if self.palette.close_prompt_idx.is_some() {
			self.palette.close_prompt_idx = None;
		} else {
			self.palette.close();
			self.mode = Mode::Editing;
		}
	}

	pub(crate) fn cmd_palette_insert_char(&mut self, ch: char) {
		self.ensure_project_indexer_started();
		self.palette.insert_char(ch);
	}

	pub(crate) fn cmd_palette_delete_char(&mut self) {
		self.palette.delete_char();
	}

	pub(crate) fn cmd_palette_up(&mut self) {
		self.palette.move_up();
	}

	pub(crate) fn cmd_palette_down(&mut self) {
		let rows = palette_visible_rows(self);
		self.palette.move_down(rows);
	}

	pub(crate) fn cmd_palette_page_up(&mut self) {
		let rows = palette_visible_rows(self);
		self.palette.page_up(rows);
	}

	pub(crate) fn cmd_palette_page_down(&mut self) {
		let rows = palette_visible_rows(self);
		self.palette.page_down(rows);
	}

	pub(crate) fn cmd_palette_confirm(&mut self) {
		// Clone the selected item before dispatching so we don't hold
		// an immutable borrow of `self.palette` while mutating self.
		let item = self.palette.selected_item().cloned();
		match item {
			Some(crate::palette::PaletteItem::Action { id, .. }) => {
				self.palette.close();
				self.mode = Mode::Editing;
				let cmd = crate::palette::action_to_command(id);
				self.execute(cmd);
			}
			Some(crate::palette::PaletteItem::Buffer { idx, .. }) => {
				self.palette.close();
				self.mode = Mode::Editing;
				if idx < self.buffers.len() {
					self.active_buffer = idx;
				}
			}
			Some(crate::palette::PaletteItem::File { path, .. }) => {
				self.palette.close();
				self.mode = Mode::Editing;
				match self.open_file(&path) {
					Ok(()) => {
						// open_file already records the recent entry on a
						// fresh load. Calling push_recent_file again here
						// also promotes already-open files to the top.
						self.push_recent_file(&path);
					}
					Err(e) => {
						self.set_status(format!(
							"Could not open {}: {}",
							path.display(),
							e
						));
					}
				}
			}
			None => {
				self.palette.close();
				self.mode = Mode::Editing;
			}
		}
	}

	pub(crate) fn cmd_palette_close_buffer(&mut self) {
		if let Some(idx) = self.palette.close_prompt_idx {
			// Already in sub-mode → Ctrl-D means Discard
			self.buffers[idx].dirty = false;
			let _ = self.close_buffer(idx);
			self.palette.close_prompt_idx = None;
			self.open_palette();
		} else {
			let item = self.palette.selected_item().cloned();
			match item {
				Some(crate::palette::PaletteItem::Buffer { idx, dirty, .. }) => {
					if dirty {
						self.palette.close_prompt_idx = Some(idx);
					} else {
						let _ = self.close_buffer(idx);
						self.open_palette(); // rebuild items
					}
				}
				Some(crate::palette::PaletteItem::File { path, .. }) => {
					self.recent_files.retain(|r| r.path != path);
					self.recent_files_dirty = true;
					self.open_palette();
				}
				_ => { /* Action selection: silent no-op */ }
			}
		}
	}

	pub(crate) fn cmd_palette_close_prompt_save(&mut self) {
		if let Some(idx) = self.palette.close_prompt_idx {
			self.active_buffer = idx;
			let cfg = self.config.clone();
			if let Err(e) = self.buffer_mut().save(&cfg) {
				self.set_status(format!("Save failed: {}", e));
				return;
			}
			let _ = self.close_buffer(idx);
			self.palette.close_prompt_idx = None;
			self.open_palette();
		}
	}

	pub(crate) fn cmd_palette_close_prompt_discard(&mut self) {
		// Triggered if user maps a separate key — kept for completeness.
		if let Some(idx) = self.palette.close_prompt_idx {
			self.buffers[idx].dirty = false;
			let _ = self.close_buffer(idx);
			self.palette.close_prompt_idx = None;
			self.open_palette();
		}
	}

	pub(crate) fn cmd_palette_close_prompt_cancel(&mut self) {
		self.palette.close_prompt_idx = None;
	}

	pub(crate) fn cmd_new_buffer(&mut self) {
		// If the active buffer is already an empty, clean, unpathed
		// scratch, just stay on it — no point in two empty scratches.
		let active = &self.buffers[self.active_buffer];
		if active.file_path.is_none()
			&& !active.dirty
			&& active.text.len_chars() == 0
		{
			let name = active.display_name();
			self.set_status(format!("{} is already empty", name));
			return;
		}
		// Otherwise, drop the auto-created startup scratch (if it's
		// still pristine somewhere else) so the user doesn't end up
		// with an [Untitled] they didn't ask for, then push a new one.
		self.maybe_dispose_startup_scratch();
		self.push_new_untitled();
		let name = self.buffer().display_name();
		self.set_status(format!("Created {}", name));
	}

	pub(crate) fn cmd_open_file_picker(&mut self) {
		// Same as PaletteOpen but pre-filtered? For now, just open the palette.
		self.mode = crate::editor::mode::Mode::Palette;
		self.open_palette();
	}

	pub(crate) fn cmd_reload_buffer(&mut self) {
		let path = match self.buffer().file_path.clone() {
			Some(p) => p,
			None => { self.set_status("Cannot reload [Untitled]"); return; }
		};
		if self.buffer().dirty {
			self.set_status("Buffer has unsaved changes — save or force-reload (not implemented)");
			return;
		}
		match crate::buffer::Buffer::from_file(&path) {
			Ok((mut new_buf, _et, _tw)) => {
				// Preserve crash-recovery coverage across reload (P0-1):
				// from_file leaves swp_path None.
				new_buf.swp_path = Some(crate::recovery::get_swap_path(&path));
				self.buffers[self.active_buffer] = new_buf;
				self.set_status(format!("Reloaded {}", path.display()));
			}
			Err(e) => self.set_status(format!("Reload failed: {}", e)),
		}
	}

	pub(crate) fn cmd_close_buffer(&mut self) {
		let idx = self.active_buffer;
		if self.buffer().dirty {
			self.set_status("Buffer has unsaved changes — save first or use palette Ctrl-D for prompt");
			return;
		}
		let _ = self.close_buffer(idx);
	}

	pub(crate) fn cmd_close_others(&mut self) {
		// Check dirty status BEFORE mutating the vector — the previous
		// swap_remove-then-restore approach silently reordered buffers
		// on the abort path (the keeper got push()-ed back at the end
		// rather than restored to its original index).
		let active = self.active_buffer;
		if self.buffers.iter().enumerate().any(|(i, b)| i != active && b.dirty) {
			self.set_status("Other buffers have unsaved changes; aborting");
			return;
		}
		let keeper = self.buffers.swap_remove(active);
		self.buffers = vec![keeper];
		self.active_buffer = 0;
		self.set_status("Closed other buffers");
	}

	pub(crate) fn cmd_close_all(&mut self) {
		for b in &self.buffers {
			if b.dirty {
				self.set_status("Some buffers have unsaved changes; save first or use Quit");
				return;
			}
		}
		self.buffers.clear();
		let mut scratch = crate::buffer::Buffer::new();
		scratch.untitled_seq = Some(1);
		scratch.swp_path = Some(crate::recovery::untitled_swap_path(1));
		self.buffers.push(scratch);
		self.active_buffer = 0;
	}

	pub(crate) fn cmd_save_all(&mut self) {
		let mut ok = 0; let mut fail = 0; let mut last_err = String::new();
		let cfg = self.config.clone();
		for i in 0..self.buffers.len() {
			if !self.buffers[i].dirty { continue; }
			if self.buffers[i].file_path.is_none() { continue; }
			// Save by temporarily switching active.
			let prev = self.active_buffer;
			self.active_buffer = i;
			match self.buffer_mut().save(&cfg) {
				Ok(_) => ok += 1,
				Err(e) => { fail += 1; last_err = e.to_string(); }
			}
			self.active_buffer = prev;
		}
		if fail == 0 {
			self.set_status(format!("Saved {} buffer(s)", ok));
		} else {
			self.set_status(format!("Saved {}; {} failed: {}", ok, fail, last_err));
		}
	}

	pub(crate) fn cmd_show_recent_files(&mut self) {
		self.mode = crate::editor::mode::Mode::Palette;
		self.open_palette();
		// Override the items: just buffers + recent, no actions.
		let items: Vec<crate::palette::PaletteItem> = self.palette.all_items.iter()
			.filter(|i| matches!(i, crate::palette::PaletteItem::Buffer { .. } | crate::palette::PaletteItem::File { .. }))
			.cloned().collect();
		self.palette.all_items = items;
		self.palette.refilter();
	}
}

/// Number of result rows the palette modal shows at once — divider rows
/// included. Must match `render::chrome::build_palette_window`'s `visible_rows`
/// (`min(terminal_height - 4, 20) - 6`) so the scroll math keeps the selected
/// item on screen; the previous hardcoded `14` over-reported on terminals
/// shorter than 24 rows.
fn palette_visible_rows(editor: &crate::editor::Editor) -> usize {
	let max_height = editor.terminal_height.saturating_sub(4).min(20);
	(max_height as usize).saturating_sub(6).max(1)
}


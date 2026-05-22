use super::rope::TextRope;

/// Maximum retained undo snapshots. Past this, the oldest are dropped so a long
/// session can't grow history without bound (P3-F). ropey's structural sharing
/// keeps each snapshot cheap, but the count itself was previously unbounded.
const MAX_UNDO_DEPTH: usize = 1000;

/// Undo/redo history. Stores Rope snapshots; ropey's structural sharing
/// makes each clone effectively O(1).
#[derive(Debug)]
pub struct History {
	undo_stack: Vec<TextRope>,
	redo_stack: Vec<TextRope>,
	/// Snapshot taken before the first edit of the current group; consumed
	/// by `commit` to push onto the undo stack.
	pending_snapshot: Option<TextRope>,
}

impl History {
	pub fn new() -> Self {
		Self {
			undo_stack: Vec::new(),
			redo_stack: Vec::new(),
			pending_snapshot: None,
		}
	}

	/// Begin an edit group: capture the pre-edit state if one isn't already
	/// pending, and discard any redo history (a new edit invalidates redo).
	pub fn start_group(&mut self, text: &TextRope) {
		if self.pending_snapshot.is_none() {
			self.pending_snapshot = Some(text.clone());
		}
		self.redo_stack.clear();
	}

	/// Push the pending snapshot onto the undo stack and end the current group.
	pub fn commit(&mut self) {
		if let Some(snap) = self.pending_snapshot.take() {
			self.undo_stack.push(snap);
			// Bound the stack: drop the oldest snapshot(s) past the cap (P3-F).
			while self.undo_stack.len() > MAX_UNDO_DEPTH {
				self.undo_stack.remove(0);
			}
		}
	}

	/// Pop the most recent snapshot off the undo stack, pushing `current`
	/// onto redo. Returns the snapshot to restore, or None if undo stack is empty.
	pub fn undo(&mut self, current: TextRope) -> Option<TextRope> {
		self.commit();
		if let Some(snap) = self.undo_stack.pop() {
			self.redo_stack.push(current);
			Some(snap)
		} else {
			None
		}
	}

	/// Pop the most recent snapshot off the redo stack, pushing `current`
	/// onto undo. Returns the snapshot to restore, or None if redo stack is empty.
	pub fn redo(&mut self, current: TextRope) -> Option<TextRope> {
		if let Some(snap) = self.redo_stack.pop() {
			self.undo_stack.push(current);
			Some(snap)
		} else {
			None
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn undo_stack_is_bounded() {
		// P3-F: an unbounded undo stack grows for the whole session. Cap it.
		let mut h = History::new();
		let text = TextRope::from_str("x");
		for _ in 0..(MAX_UNDO_DEPTH + 50) {
			h.start_group(&text);
			h.commit();
		}
		assert!(
			h.undo_stack.len() <= MAX_UNDO_DEPTH,
			"undo stack grew to {} (cap {})",
			h.undo_stack.len(),
			MAX_UNDO_DEPTH
		);
	}

	#[test]
	fn test_snapshot_undo_redo() {
		let initial = TextRope::from_str("hello");
		let mut history = History::new();

		// Start edit group
		history.start_group(&initial);

		// Execute edit
		let mut edited = initial.clone();
		edited.insert_str(5, " world");

		history.commit();

		let restored = history.undo(edited.clone()).unwrap();
		assert_eq!(restored.to_string_full(), "hello");

		let redone = history.redo(restored).unwrap();
		assert_eq!(redone.to_string_full(), "hello world");
	}
}

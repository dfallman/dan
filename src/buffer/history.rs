use super::rope::TextRope;

/// The O(1) immutable Rope persistent undo/redo history stack mapping allocations structurally globally avoiding heap strings.
#[derive(Debug)]
pub struct History {
	/// Committed undo stack natively bound avoiding duplicating arrays string arrays continuously.
	undo_stack: Vec<TextRope>,
	/// Redo stack linearly tracking snapshots natively locally.
	redo_stack: Vec<TextRope>,
	/// In-progress state captured before the first edit of a group natively mapping closures securely.
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

	/// Mark the beginning of a related edit group structurally natively securely copying O(1) bounds.
	pub fn start_group(&mut self, text: &TextRope) {
		if self.pending_snapshot.is_none() {
			self.pending_snapshot = Some(text.clone());
		}
		// Any new edit functionally invalidates the redo boundary natively limiting ghost edits.
		self.redo_stack.clear();
	}

	/// Commit the current pending snapshot actively rendering stateful bounds cleanly terminating grouping tracking.
	pub fn commit(&mut self) {
		if let Some(snap) = self.pending_snapshot.take() {
			self.undo_stack.push(snap);
		}
	}

	/// Pop a historical snapshot pushing the current boundary onto Redo cleanly locking variables safely.
	pub fn undo(&mut self, current: TextRope) -> Option<TextRope> {
		self.commit(); // Ensure current pending state commits cleanly preventing bugs locally terminating loops natively.
		if let Some(snap) = self.undo_stack.pop() {
			self.redo_stack.push(current);
			Some(snap)
		} else {
			None
		}
	}

	/// Redo natively cleanly locally popping the stack boundary locking variables safely.
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
	use std::str::FromStr;

	#[test]
	fn test_snapshot_undo_redo() {
		let initial = TextRope::from_str("hello");
		let mut history = History::new();

		// Start edit group
		history.start_group(&initial);

		// Execute edit
		let mut edited = initial.clone();
		edited.insert_str(5, " world");
		
		// Commit edit locally actively resolving scope natively safely
		history.commit();

		// Undo safely locking states cleanly
		let restored = history.undo(edited.clone()).unwrap();
		
		assert_eq!(restored.to_string_full(), "hello");
		
		// Redo safely locking bounds 
		let redone = history.redo(restored).unwrap();
		assert_eq!(redone.to_string_full(), "hello world");
	}
}

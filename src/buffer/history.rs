use super::rope::TextRope;

/// A single edit operation that can be undone/redone.
#[derive(Debug, Clone)]
pub enum Edit {
	/// Insert text at a char position.
	Insert { pos: usize, text: String },
	/// Delete text at a char range.
	Delete { pos: usize, text: String },
}

impl Edit {
	/// Apply this edit to a text rope (redo direction).
	pub fn apply(&self, text: &mut TextRope) {
		match self {
			Edit::Insert { pos, text: t } => {
				text.insert_str(*pos, t);
			}
			Edit::Delete { pos, text: _ } => {
				let len = self.text_len();
				text.remove(*pos..*pos + len);
			}
		}
	}

	/// Reverse this edit on a text rope (undo direction).
	pub fn reverse(&self, text: &mut TextRope) {
		match self {
			Edit::Insert { pos, text: t } => {
				text.remove(*pos..*pos + t.chars().count());
			}
			Edit::Delete { pos, text: t } => {
				text.insert_str(*pos, t);
			}
		}
	}

	fn text_len(&self) -> usize {
		match self {
			Edit::Insert { text, .. } | Edit::Delete { text, .. } => text.chars().count(),
		}
	}
}

/// A group of edits forming a single undo-able operation.
#[derive(Debug, Clone)]
pub struct EditGroup {
	pub edits: Vec<Edit>,
}

/// The undo/redo history stack.
#[derive(Debug)]
pub struct History {
	/// Committed undo stack.
	undo_stack: Vec<EditGroup>,
	/// Redo stack (cleared on new edits).
	redo_stack: Vec<EditGroup>,
	/// In-progress edits not yet committed to the undo stack.
	pending: Vec<Edit>,
}

impl History {
	pub fn new() -> Self {
		Self {
			undo_stack: Vec::new(),
			redo_stack: Vec::new(),
			pending: Vec::new(),
		}
	}

	/// Record an edit (it will be part of the current pending group).
	pub fn record(&mut self, edit: Edit) {
		self.pending.push(edit);
		// New edits invalidate the redo stack
		self.redo_stack.clear();
	}

	/// Commit the pending edits as a single undo group.
	pub fn commit(&mut self) {
		if !self.pending.is_empty() {
			let group = EditGroup {
				edits: std::mem::take(&mut self.pending),
			};
			self.undo_stack.push(group);
		}
	}

	/// Undo the last edit group, returning the group for reversal.
	pub fn undo(&mut self) -> Option<EditGroup> {
		self.commit(); // Commit any pending edits first
		if let Some(group) = self.undo_stack.pop() {
			self.redo_stack.push(group.clone());
			Some(group)
		} else {
			None
		}
	}

	/// Redo the last undone edit group.
	pub fn redo(&mut self) -> Option<EditGroup> {
		if let Some(group) = self.redo_stack.pop() {
			self.undo_stack.push(group.clone());
			Some(group)
		} else {
			None
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_undo_redo() {
		let mut text = TextRope::from_str("hello");
		let mut history = History::new();

		// Insert " world"
		let edit = Edit::Insert {
			pos: 5,
			text: " world".to_string(),
		};
		edit.apply(&mut text);
		history.record(edit);
		history.commit();

		assert_eq!(text.to_string_full(), "hello world");

		// Undo
		if let Some(group) = history.undo() {
			for edit in group.edits.iter().rev() {
				edit.reverse(&mut text);
			}
		}
		assert_eq!(text.to_string_full(), "hello");

		// Redo
		if let Some(group) = history.redo() {
			for edit in &group.edits {
				edit.apply(&mut text);
			}
		}
		assert_eq!(text.to_string_full(), "hello world");
	}
}

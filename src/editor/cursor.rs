/// A cursor position in the buffer (line, column in chars).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    /// Zero-indexed line number.
    pub line: usize,
    /// Zero-indexed column (char offset within the line).
    pub col: usize,
    /// "Desired" column — preserved across vertical movement through short lines.
    pub desired_col: usize,
}

impl Cursor {
    pub fn new(line: usize, col: usize) -> Self {
        Self {
            line,
            col,
            desired_col: col,
        }
    }

    pub fn origin() -> Self {
        Self::new(0, 0)
    }

    /// Set both actual and desired column.
    pub fn set_col(&mut self, col: usize) {
        self.col = col;
        self.desired_col = col;
    }

    /// Set column without updating desired (for vertical movement).
    pub fn set_col_clamped(&mut self, col: usize) {
        self.col = col;
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::origin()
    }
}

/// A selection range defined by an anchor and the cursor (head).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// The anchor point (where the selection started).
    pub anchor: Cursor,
    /// The head (where the cursor currently is).
    pub head: Cursor,
}

impl Selection {
    /// Create a collapsed selection (no range selected) at a cursor.
    pub fn collapsed(cursor: Cursor) -> Self {
        Self {
            anchor: cursor,
            head: cursor,
        }
    }

    /// Check if this selection is collapsed (no range).
    pub fn is_collapsed(&self) -> bool {
        self.anchor.line == self.head.line && self.anchor.col == self.head.col
    }

    /// Get the start and end of the selection in document order.
    pub fn ordered(&self) -> (Cursor, Cursor) {
        if self.anchor.line < self.head.line
            || (self.anchor.line == self.head.line && self.anchor.col <= self.head.col)
        {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }
}

/// Cursor/selection container. Wraps a single `Selection` whose anchor
/// and head diverge when the user shift-selects. Moving without shift
/// collapses the selection (anchor == head).
#[derive(Debug, Clone)]
pub struct CursorSet {
    selection: Selection,
}

impl CursorSet {
    pub fn new() -> Self {
        Self {
            selection: Selection::collapsed(Cursor::origin()),
        }
    }

    /// Get the primary selection.
    pub fn primary(&self) -> &Selection {
        &self.selection
    }

    /// Get a mutable reference to the primary selection.
    pub fn primary_mut(&mut self) -> &mut Selection {
        &mut self.selection
    }

    /// Get the primary cursor (head of primary selection).
    pub fn cursor(&self) -> Cursor {
        self.selection.head
    }

    /// Set the primary cursor position, collapsing the selection.
    pub fn set_cursor(&mut self, line: usize, col: usize) {
        let cursor = Cursor::new(line, col);
        self.selection = Selection::collapsed(cursor);
    }

    /// Returns true if the user has an active (non-collapsed) selection.
    pub fn has_selection(&self) -> bool {
        !self.selection.is_collapsed()
    }

    /// Pin the anchor at the current head position so subsequent head
    /// moves create a selection range. No-op if already selecting.
    pub fn begin_selection(&mut self) {
        if self.selection.is_collapsed() {
            // anchor stays, head will diverge on the next move
            self.selection.anchor = self.selection.head;
        }
    }

    /// Collapse the selection by snapping the anchor to the head.
    pub fn collapse_selection(&mut self) {
        self.selection.anchor = self.selection.head;
    }
}

impl Default for CursorSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_origin() {
        let c = Cursor::origin();
        assert_eq!(c.line, 0);
        assert_eq!(c.col, 0);
    }

    #[test]
    fn test_selection_collapsed() {
        let c = Cursor::new(5, 10);
        let s = Selection::collapsed(c);
        assert!(s.is_collapsed());
    }

    #[test]
    fn test_selection_ordered() {
        let anchor = Cursor::new(5, 10);
        let head = Cursor::new(3, 0);
        let s = Selection { anchor, head };
        let (start, end) = s.ordered();
        assert_eq!(start.line, 3);
        assert_eq!(end.line, 5);
    }

    #[test]
    fn test_cursor_set_default() {
        let cs = CursorSet::new();
        assert!(!cs.has_selection());
        assert_eq!(cs.cursor().line, 0);
        assert_eq!(cs.cursor().col, 0);
    }

    #[test]
    fn test_begin_and_collapse_selection() {
        let mut cs = CursorSet::new();
        cs.begin_selection();
        assert!(!cs.has_selection()); // anchor == head still
        cs.primary_mut().head.set_col(5);
        assert!(cs.has_selection());
        cs.collapse_selection();
        assert!(!cs.has_selection());
    }
}

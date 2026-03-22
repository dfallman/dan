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

/// Multi-cursor container. Always has at least one cursor.
#[derive(Debug, Clone)]
pub struct CursorSet {
    /// Primary cursor is always at index 0.
    selections: Vec<Selection>,
}

impl CursorSet {
    pub fn new() -> Self {
        Self {
            selections: vec![Selection::collapsed(Cursor::origin())],
        }
    }

    /// Get the primary selection.
    pub fn primary(&self) -> &Selection {
        &self.selections[0]
    }

    /// Get a mutable reference to the primary selection.
    pub fn primary_mut(&mut self) -> &mut Selection {
        &mut self.selections[0]
    }

    /// Get the primary cursor (head of primary selection).
    pub fn cursor(&self) -> Cursor {
        self.selections[0].head
    }

    /// Set the primary cursor position, collapsing the selection.
    pub fn set_cursor(&mut self, line: usize, col: usize) {
        let cursor = Cursor::new(line, col);
        self.selections = vec![Selection::collapsed(cursor)];
    }

    /// Get all selections.
    pub fn selections(&self) -> &[Selection] {
        &self.selections
    }

    /// Add a new cursor, creating a new selection.
    pub fn add_cursor(&mut self, line: usize, col: usize) {
        let cursor = Cursor::new(line, col);
        self.selections.push(Selection::collapsed(cursor));
    }

    /// Reduce to a single cursor (the primary).
    pub fn collapse_to_primary(&mut self) {
        self.selections.truncate(1);
        let head = self.selections[0].head;
        self.selections[0] = Selection::collapsed(head);
    }

    /// Number of active cursors/selections.
    pub fn count(&self) -> usize {
        self.selections.len()
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
        assert_eq!(cs.count(), 1);
        assert_eq!(cs.cursor().line, 0);
        assert_eq!(cs.cursor().col, 0);
    }

    #[test]
    fn test_multi_cursor() {
        let mut cs = CursorSet::new();
        cs.add_cursor(5, 10);
        assert_eq!(cs.count(), 2);
        cs.collapse_to_primary();
        assert_eq!(cs.count(), 1);
    }
}

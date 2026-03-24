pub mod history;
pub mod rope;

use std::io;
use std::path::{Path, PathBuf};

use self::history::{Edit, History};
use self::rope::TextRope;

/// A text buffer representing a file or scratch document.
pub struct Buffer {
    /// The text content.
    pub text: TextRope,
    /// Edit history for undo/redo.
    pub history: History,
    /// File path (None for scratch buffers).
    pub file_path: Option<PathBuf>,
    /// Whether the buffer has unsaved changes.
    pub dirty: bool,
}

impl Buffer {
    /// Create an empty buffer.
    pub fn new() -> Self {
        Self {
            text: TextRope::new(),
            history: History::new(),
            file_path: None,
            dirty: false,
        }
    }

    /// Create a buffer from a file.
    pub fn from_file(path: &Path) -> io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self {
            text: TextRope::from_str(&content),
            history: History::new(),
            file_path: Some(path.to_path_buf()),
            dirty: false,
        })
    }

    /// Save the buffer to its file.
    pub fn save(&mut self) -> io::Result<()> {
        if let Some(ref path) = self.file_path {
            std::fs::write(path, self.text.to_string_full())?;
            self.dirty = false;
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "No file path set for this buffer",
            ))
        }
    }

    /// Save the buffer to a new path and adopt it as the buffer's file.
    pub fn save_to(&mut self, path: &Path) -> io::Result<()> {
        std::fs::write(path, self.text.to_string_full())?;
        self.file_path = Some(path.to_path_buf());
        self.dirty = false;
        Ok(())
    }

    /// Number of lines in the buffer.
    pub fn line_count(&self) -> usize {
        self.text.len_lines()
    }

    /// Get the display name for this buffer.
    pub fn display_name(&self) -> String {
        self.file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "[scratch]".to_string())
    }

    // -- Edit operations with history tracking --

    /// Insert a character at a char position.
    pub fn insert_char(&mut self, pos: usize, ch: char) {
        let edit = Edit::Insert {
            pos,
            text: ch.to_string(),
        };
        edit.apply(&mut self.text);
        self.history.record(edit);
        self.dirty = true;
    }

    /// Insert a string at a char position.
    pub fn insert_str(&mut self, pos: usize, s: &str) {
        let edit = Edit::Insert {
            pos,
            text: s.to_string(),
        };
        edit.apply(&mut self.text);
        self.history.record(edit);
        self.dirty = true;
    }

    /// Delete a single character at a char position.
    pub fn delete_char(&mut self, pos: usize) {
        if pos < self.text.len_chars() {
            let ch = self.text.char_at(pos);
            let edit = Edit::Delete {
                pos,
                text: ch.to_string(),
            };
            edit.apply(&mut self.text);
            self.history.record(edit);
            self.dirty = true;
        }
    }

    /// Delete a range of characters.
    pub fn delete_range(&mut self, start: usize, end: usize) {
        if start < end && end <= self.text.len_chars() {
            let text = self.text.slice_to_string(start..end);
            let edit = Edit::Delete { pos: start, text };
            edit.apply(&mut self.text);
            self.history.record(edit);
            self.dirty = true;
        }
    }

    /// Commit pending edits as an undo group.
    pub fn commit_edits(&mut self) {
        self.history.commit();
    }

    /// Undo the last edit group.
    pub fn undo(&mut self) {
        if let Some(group) = self.history.undo() {
            for edit in group.edits.iter().rev() {
                edit.reverse(&mut self.text);
            }
            self.dirty = true;
        }
    }

    /// Redo the last undone edit group.
    pub fn redo(&mut self) {
        if let Some(group) = self.history.redo() {
            for edit in &group.edits {
                edit.apply(&mut self.text);
            }
            self.dirty = true;
        }
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

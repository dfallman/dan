// Syntax highlighting (tree-sitter integration).
// Placeholder — will be expanded with tree-sitter grammars.

/// Highlight span representing a styled region of text.
#[derive(Debug, Clone)]
pub struct Highlight {
    /// Start byte offset.
    pub start: usize,
    /// End byte offset.
    pub end: usize,
    /// The highlight group (e.g. "keyword", "string", "comment").
    pub group: &'static str,
}

/// Stub: Highlight a line of text. Returns empty for now.
pub fn highlight_line(_line: &str, _language: Option<&str>) -> Vec<Highlight> {
    // TODO: integrate tree-sitter for real syntax highlighting
    Vec::new()
}

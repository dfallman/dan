//! Palette items and the static action registry.

use std::path::PathBuf;
use std::time::SystemTime;
use crate::editor::commands::Command;

/// One row in the palette result list.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PaletteItem {
    Action {
        id: ActionId,
        label: String,
        hint: Option<String>,
    },
    Buffer {
        idx: usize,           // index into Editor.buffers
        dirty: bool,
        path_display: String,
        is_current: bool,
    },
    File {
        path: PathBuf,
        display: String,        // shorter form for rendering
        last_opened: Option<SystemTime>,
    },
}

#[allow(dead_code)]
impl PaletteItem {
    /// Text used for fuzzy-matching against the user's query.
    pub fn search_text(&self) -> &str {
        match self {
            PaletteItem::Action { label, .. } => label,
            PaletteItem::Buffer { path_display, .. } => path_display,
            PaletteItem::File { display, .. } => display,
        }
    }

    /// Tiebreak for equal scores. Lower = sorted first.
    pub fn kind_rank(&self) -> u8 {
        match self {
            PaletteItem::Buffer { .. } => 0,
            PaletteItem::Action { .. } => 1,
            PaletteItem::File { .. } => 2,
        }
    }
}

/// Identifies an Action item without owning the executable Command (so PaletteItem
/// stays cheap to clone/compare).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ActionId {
    Save, SaveAs, Quit, ForceQuit,
    Undo, Redo, Copy, Cut, Paste, SelectAll,
    Find, GoToLine, FormatDocument, ToggleComment,
    ToggleWrap, ToggleHelp, ToggleSyntax, ToggleWhitespace,
    DeleteLine, DuplicateLineOrSelection, MoveBufferTop, MoveBufferBottom,
    // New commands added in Task 22+; declared here to seed the registry shape.
    NewBuffer,
    OpenFile, ReloadBuffer, CloseBuffer, CloseOthers, CloseAll, SaveAll,
    CopyPathAbs, CopyPathRel, RevealInFinder, OpenContainingFolder, ShowBufferInfo,
    IndentSpaces, IndentTabs, TabWidth2, TabWidth4, TabWidth8,
    LineEndingsLF, LineEndingsCRLF,
    TrimTrailingWhitespace, ConvertTabsToSpaces, ConvertSpacesToTabs,
    SortLinesAsc, SortLinesDesc, DedupAdjacent,
    UpperCase, LowerCase, TitleCase, ReverseSelection,
    ToggleLineNumbers, ReloadConfiguration, ShowRecentFiles,
    ShowVersion, ShowKeybindings,
}

/// Maps an ActionId to the editor Command it dispatches.
/// Some ActionIds (e.g. ChangeSyntax, EncodingPick) need parametric dispatch and
/// are handled by the palette confirm logic directly rather than this map.
#[allow(dead_code)]
pub fn action_to_command(id: ActionId) -> Command {
    use ActionId::*;
    match id {
        Save => Command::Save,
        SaveAs => Command::SaveAsOpen,
        Quit => Command::Quit,
        // ActionId::ForceQuit corresponds to the Ctrl-Shift-C panic button
        // ("Force Quit" in the palette), which is an unconditional exit.
        ForceQuit => Command::ForceQuitAll,
        Undo => Command::Undo,
        Redo => Command::Redo,
        Copy => Command::Copy,
        Cut => Command::Cut,
        Paste => Command::Paste,
        SelectAll => Command::SelectAll,
        Find => Command::SearchForward,
        GoToLine => Command::GoToLineOpen,
        FormatDocument => Command::FormatDocument,
        ToggleComment => Command::ToggleComment,
        ToggleWrap => Command::ToggleWrap,
        ToggleHelp => Command::ToggleHelp,
        ToggleSyntax => Command::ToggleSyntax,
        ToggleWhitespace => Command::ToggleWhitespace,
        DeleteLine => Command::DeleteLine,
        DuplicateLineOrSelection => Command::DuplicateLineOrSelection,
        MoveBufferTop => Command::MoveBufferTop,
        MoveBufferBottom => Command::MoveBufferBottom,
        NewBuffer => Command::NewBuffer,
        OpenFile => Command::OpenFilePicker,
        ReloadBuffer => Command::ReloadBuffer,
        CloseBuffer => Command::CloseBuffer,
        CloseOthers => Command::CloseOthers,
        CloseAll => Command::CloseAll,
        SaveAll => Command::SaveAll,
        CopyPathAbs => Command::CopyPathAbs,
        CopyPathRel => Command::CopyPathRel,
        RevealInFinder => Command::RevealInFinder,
        OpenContainingFolder => Command::OpenContainingFolder,
        ShowBufferInfo => Command::ShowBufferInfo,
        // Task 24: format/encoding commands
        IndentSpaces => Command::IndentSpaces,
        IndentTabs => Command::IndentTabs,
        TabWidth2 => Command::TabWidth(2),
        TabWidth4 => Command::TabWidth(4),
        TabWidth8 => Command::TabWidth(8),
        LineEndingsLF => Command::LineEndingsLF,
        LineEndingsCRLF => Command::LineEndingsCRLF,
        TrimTrailingWhitespace => Command::TrimTrailingWhitespaceNow,
        ConvertTabsToSpaces => Command::ConvertTabsToSpaces,
        ConvertSpacesToTabs => Command::ConvertSpacesToTabs,
        // Task 25: text transform commands
        SortLinesAsc => Command::SortLinesAsc,
        SortLinesDesc => Command::SortLinesDesc,
        DedupAdjacent => Command::DedupAdjacent,
        UpperCase => Command::ConvertUpper,
        LowerCase => Command::ConvertLower,
        TitleCase => Command::ConvertTitle,
        ReverseSelection => Command::ReverseSelection,
        // Task 26: convenience + diagnostic commands
        ToggleLineNumbers => Command::ToggleLineNumbers,
        ReloadConfiguration => Command::ReloadConfiguration,
        ShowRecentFiles => Command::ShowRecentFiles,
        ShowVersion => Command::ShowVersion,
        ShowKeybindings => Command::ToggleHelp,
    }
}

/// Returns the static list of action items (label + hint) used to seed the palette.
#[allow(dead_code)]
pub fn action_registry() -> Vec<PaletteItem> {
    use ActionId::*;
    let entries: &[(ActionId, &str, Option<&str>)] = &[
        (Save, "Save", Some("⌃S")),
        (SaveAs, "Save as…", Some("⌃A")),
        (Quit, "Quit", Some("⌃Q")),
        (ForceQuit, "Force quit", None),
        (Undo, "Undo", Some("⌃Z")),
        (Redo, "Redo", Some("⌃Y")),
        (Copy, "Copy", Some("⌃C")),
        (Cut, "Cut", Some("⌃X")),
        (Paste, "Paste", Some("⌃V")),
        (SelectAll, "Select all", Some("⌃\\")),
        (Find, "Find…", Some("⌃F")),
        (GoToLine, "Go to line…", Some("⌃G")),
        (FormatDocument, "Format document", Some("⌃L")),
        (ToggleComment, "Toggle comment", Some("⌃/")),
        (ToggleWrap, "Toggle wrap", Some("⌃W")),
        (ToggleHelp, "Toggle help", Some("⌃H")),
        (ToggleSyntax, "Toggle syntax highlighting", Some("⌃T")),
        (ToggleWhitespace, "Toggle whitespace", Some("⌃R")),
        (DeleteLine, "Delete line", Some("⌃K")),
        (DuplicateLineOrSelection, "Duplicate line/selection", Some("⌃D")),
        (MoveBufferTop, "Move to top of file", Some("⌃Home")),
        (MoveBufferBottom, "Move to bottom of file", Some("⌃End")),
        // Net-new commands (no top-level keybinding):
        (NewBuffer, "New buffer", Some("⌃N")),
        (OpenFile, "Open file…", None),
        (ReloadBuffer, "Reload buffer from disk", None),
        (CloseBuffer, "Close buffer", None),
        (CloseOthers, "Close other buffers", None),
        (CloseAll, "Close all buffers", None),
        (SaveAll, "Save all", None),
        (CopyPathAbs, "Copy file path (absolute)", None),
        (CopyPathRel, "Copy file path (relative)", None),
        (RevealInFinder, "Reveal in Finder", None),
        (OpenContainingFolder, "Open containing folder", None),
        (ShowBufferInfo, "Show buffer info", None),
        (IndentSpaces, "Indent: spaces", None),
        (IndentTabs, "Indent: tabs", None),
        (TabWidth2, "Tab width: 2", None),
        (TabWidth4, "Tab width: 4", None),
        (TabWidth8, "Tab width: 8", None),
        (LineEndingsLF, "Line endings: LF (Unix)", None),
        (LineEndingsCRLF, "Line endings: CRLF (Windows)", None),
        (TrimTrailingWhitespace, "Trim trailing whitespace", None),
        (ConvertTabsToSpaces, "Convert indentation: tabs → spaces", None),
        (ConvertSpacesToTabs, "Convert indentation: spaces → tabs", None),
        (SortLinesAsc, "Sort lines (ascending)", None),
        (SortLinesDesc, "Sort lines (descending)", None),
        (DedupAdjacent, "Deduplicate adjacent lines", None),
        (UpperCase, "Convert: UPPERCASE", None),
        (LowerCase, "Convert: lowercase", None),
        (TitleCase, "Convert: Title Case", None),
        (ReverseSelection, "Reverse selection", None),
        (ToggleLineNumbers, "Toggle line numbers", None),
        (ReloadConfiguration, "Reload configuration", None),
        (ShowRecentFiles, "Show recent files", None),
        (ShowVersion, "Show version", None),
        (ShowKeybindings, "Show keybindings", None),
    ];
    entries.iter().map(|&(id, label, hint)| PaletteItem::Action {
        id,
        label: label.to_string(),
        hint: hint.map(String::from),
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_non_empty() {
        let r = action_registry();
        assert!(r.len() >= 30, "expected ≥30 actions, got {}", r.len());
    }

    #[test]
    fn every_registry_entry_has_known_command() {
        for item in action_registry() {
            if let PaletteItem::Action { id, .. } = item {
                let _ = action_to_command(id); // doesn't panic
            }
        }
    }

    #[test]
    fn kind_rank_orders_buffer_action_file() {
        let buf = PaletteItem::Buffer { idx: 0, dirty: false, path_display: "x".into(), is_current: false };
        let act = PaletteItem::Action { id: ActionId::Save, label: "Save".into(), hint: None };
        let f = PaletteItem::File { path: "x".into(), display: "x".into(), last_opened: None };
        assert!(buf.kind_rank() < act.kind_rank());
        assert!(act.kind_rank() < f.kind_rank());
    }

    #[test]
    fn search_text_returns_action_label() {
        let a = PaletteItem::Action { id: ActionId::Save, label: "Save File Now".into(), hint: None };
        assert_eq!(a.search_text(), "Save File Now");
    }
}

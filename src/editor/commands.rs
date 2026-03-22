/// All editor commands — pico-style, no modal keybindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    // -- Motion --
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveLineStart,
    MoveLineEnd,
    MoveWordForward,
    MoveWordBackward,
    MoveBufferTop,
    MoveBufferBottom,
    PageUp,
    PageDown,

    // -- Selection (shift+arrows) --
    SelectLeft,
    SelectRight,
    SelectUp,
    SelectDown,
    SelectLineStart,
    SelectLineEnd,
    SelectAll,

    // -- Editing --
    InsertChar(char),
    InsertString(String),
    InsertNewline,
    InsertTab,
    DeleteBackward,
    DeleteForward,
    DeleteLine,

    // -- Undo / Redo --
    Undo,
    Redo,

    // -- Clipboard (GUI-style) --
    Copy,
    Cut,
    Paste,

    // -- Search --
    SearchForward,
    SearchNext,
    SearchPrev,

    // -- File --
    Save,
    Quit,
    ForceQuit,

    // -- Misc --
    Noop,
}

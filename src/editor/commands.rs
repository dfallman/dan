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
	SwapLineUp,
	SwapLineDown,
	MoveBufferTop,
	MoveBufferBottom,
	PageUp,
	PageDown,
	ScrollViewportUp,
	ScrollViewportDown,
	MoveFastUp,
	MoveFastDown,

	// -- Selection (shift+arrows) --
	SelectLeft,
	SelectRight,
	SelectUp,
	SelectDown,
	SelectWordForward,
	SelectWordBackward,
	SelectLineStart,
	SelectLineEnd,
	SelectAll,

	// -- Mouse --
	MouseDown { col: u16, row: u16, extend: bool },
	MouseDrag { col: u16, row: u16 },
	MouseUp { col: u16, row: u16 },

	// -- Editing --
	InsertChar(char),
	InsertString(String),
	InsertNewline,
	InsertTab,
	Dedent,
	DeleteBackward,
	DeleteForward,
	DeleteLine,
	DuplicateLineOrSelection,
	FormatDocument,

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
	SearchInsertChar(char),
	SearchDeleteChar,
	SearchConfirm,
	SearchCancel,
	SearchConvertToReplace,

	// -- Global Replace --
	ReplaceInsertChar(char),
	ReplaceDeleteChar,
	ReplaceWithConfirm,
	ReplaceActionYes,
	ReplaceActionNo,
	ReplaceActionAll,
	ReplaceCancel,

	// -- Go-to-line --
	GoToLineOpen,
	GoToLineInsertChar(char),
	GoToLineDeleteChar,
	GoToLineConfirm,
	GoToLineCancel,

	// -- Save As --
	SaveAsOpen,
	SaveAsInsertChar(char),
	SaveAsDeleteChar,
	PromptCursorLeft,
	PromptCursorRight,
	SaveAsConfirm,
	SaveAsCancel,

	// -- Overwrite confirmation --
	ConfirmOverwrite,
	CancelOverwrite,

	// -- File --
	Save,
	Quit,
	/// Discard the active buffer's dirty state and advance to the next dirty
	/// buffer in the quit cycle. Used by Ctrl-F in ConfirmQuit mode.
	ForceQuit,
	/// Unconditional immediate exit — the panic-button shortcut (Ctrl-Shift-C)
	/// and the recover-swap "give up" keys (Esc, Ctrl-Q).
	ForceQuitAll,
	SaveAndQuit,
	CancelQuit,
	OpenFilePicker,
	NewBuffer,
	ReloadBuffer,
	CloseBuffer,
	CloseOthers,
	CloseAll,
	SaveAll,

	RecoverSwapAccept,
	RecoverSwapDecline,

	// -- Palette --
	PaletteOpen,
	PaletteInsertChar(char),
	PaletteDeleteChar,
	PaletteUp,
	PaletteDown,
	PalettePageUp,
	PalettePageDown,
	PaletteConfirm,
	PaletteCancel,
	PaletteCloseBuffer,
	PaletteClosePromptSave,
	#[allow(dead_code)]
	PaletteClosePromptDiscard,
	#[allow(dead_code)]
	PaletteClosePromptCancel,

	// -- Path / Metadata --
	CopyPathAbs,
	CopyPathRel,
	RevealInFinder,
	OpenContainingFolder,
	ShowBufferInfo,

	// -- Format / Encoding --
	IndentSpaces,
	IndentTabs,
	TabWidth(usize),
	LineEndingsLF,
	LineEndingsCRLF,
	TrimTrailingWhitespaceNow,
	ConvertTabsToSpaces,
	ConvertSpacesToTabs,

	// -- Text Transforms --
	SortLinesAsc,
	SortLinesDesc,
	DedupAdjacent,
	ConvertUpper,
	ConvertLower,
	ConvertTitle,
	ReverseSelection,

	// -- Misc --
	ToggleWrap,
	ToggleWhitespace,
	ToggleHelp,
	ToggleSyntax,
	ToggleComment,
	ToggleLineNumbers,
	ReloadConfiguration,
	ShowRecentFiles,
	ShowVersion,
	Noop,
}

/// Categorises edits for undo grouping: consecutive same-kind edits collapse
/// into one undo step, kind-changes commit a snapshot.
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum EditAction {
    Insert,
    Whitespace,
    Delete,
    Other,
}

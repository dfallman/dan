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
	ReplaceSearchConfirm,
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
	ForceQuit,
	SaveAndQuit,
	CancelQuit,

	RecoverSwapAccept,
	RecoverSwapDecline,

	// -- Misc --
	ToggleWrap,
	ToggleHelp,
	ToggleSyntax,
	ToggleComment,
	Noop,
}

---
name: dan-editor
description: >
  Comprehensive development guide for the "dan" terminal text editor — a modeless,
  nano-style Rust TUI editor using crossterm + ropey. Covers architecture,
  module conventions, command pattern, rendering pipeline, Unicode handling,
  and contribution workflow. Use when writing, reviewing, or extending any
  code in the dan editor project. Runs natively on macOS (aarch64-apple-darwin).
---

# Dan Editor — Development Skill

## Project Identity

**Dan** is a modeless terminal text editor written in Rust. It targets the
nano/Notepad mental model: type to insert, arrow keys to move, `Ctrl+S` to
save. No vim modes, no emacs chords. GUI-style keybindings throughout.

**Binary:** `dan`  
**Entry point:** `src/main.rs`  
**Build:** `cargo build 2>&1`  
**Tests:** `cargo test 2>&1` (Always use exactly this with stderr redirection; never use `cargo check` or `cargo test` unredirected)  
**Version:** stored in `VERSION` file; git hash embedded by `build.rs`

---

## Architecture Overview

```
src/
├── main.rs              Event loop, CLI arg parsing, terminal setup/teardown
├── utils.rs             Unicode char-width helper (char_width)
├── buffer/
│   ├── mod.rs           Buffer: TextRope + file path + dirty flag + edit ops
│   ├── rope.rs          TextRope: thin wrapper around ropey::Rope
│   └── history.rs       Undo/redo stack with edit grouping (EditGroup)
├── config/
│   └── mod.rs           TOML config loading from platform config_dir()
├── editor/
│   ├── mod.rs           Editor struct: state, command dispatch, visual-wrap nav
│   ├── commands.rs      Command enum (≈70 variants for all user actions)
│   ├── cursor.rs        CursorSet: multi-cursor positions + selection anchors
│   └── mode.rs          Mode enum: Editing, Selecting, Searching
├── input/
│   └── mod.rs           map_event(): crossterm Event → Command mapping
├── render/
│   └── mod.rs           Full terminal rendering: gutter, text, status, cursor
└── syntax/
    └── mod.rs           Syntax highlighting stub (tree-sitter placeholder)
```

### Data Flow

```
crossterm::Event
  → input::map_event()  →  Command enum
  → editor.execute(cmd) →  mutates Editor/Buffer/CursorSet state
  → render::render()    →  draws to BufWriter<Stdout>
```

The event loop in `main.rs` is **batched**: after processing the first event,
it drains all buffered events via `event::poll(Duration::ZERO)` before
rendering. This collapses fast typing / unbracketed paste into a single draw.

---

## Module-by-Module Reference

### `main.rs` — Entry Point

- Parses CLI args (`-v`/`--version`, or a file path).
- Creates `Editor`, opens file or sets empty buffer with target path.
- Enters raw mode + alternate screen via crossterm.
- Enables bracketed paste (`EnableBracketedPaste`).
- Calls `run_loop()`: render → read event → drain burst → repeat.
- On exit: disables bracketed paste, leaves alternate screen, restores terminal.
- Uses a **64 KB `BufWriter`** to minimize syscalls over SSH.

**Constants:**
- `VERSION: &str` — `include_str!("../VERSION")`
- `GIT_HASH: &str` — `env!("GIT_HASH")` set by `build.rs`

### `editor/commands.rs` — Command Enum

All user actions are a `Command` variant. Key categories:
- **Navigation:** `MoveLeft`, `MoveRight`, `MoveUp`, `MoveDown`, `MoveWordLeft`,
  `MoveWordRight`, `MoveHome`, `MoveEnd`, `MoveFileStart`, `MoveFileEnd`,
  `PageUp`, `PageDown`
- **Selection:** `SelectLeft`, `SelectRight`, `SelectUp`, `SelectDown`,
  `SelectWordLeft`, `SelectWordRight`, `SelectHome`, `SelectEnd`,
  `SelectAll`
- **Editing:** `InsertChar(char)`, `InsertNewline`, `InsertTab`,
  `Dedent`, `DeleteBackward`, `DeleteForward`, `DeleteLine`,
  `DuplicateLine`, `SwapLineUp`, `SwapLineDown`, `Paste(String)`
- **Clipboard:** `Copy`, `Cut`
- **Undo/Redo:** `Undo`, `Redo`
- **Search:** `SearchStart`, `SearchNext`, `SearchAccept`, `SearchCancel`,
  `SearchInput(char)`, `SearchBackspace`
- **File:** `Save`, `Quit`, `ForceQuit`
- **UI:** `ToggleHelp`
- **Noop:** `Noop` (unhandled keys)

### `editor/mode.rs` — Modes

Three modes (not vim-style; they're UI states):
- **`Editing`** — default; typing inserts text.
- **`Selecting`** — entered via shift+arrow; cursor movement extends selection.
- **`Searching`** — entered via `Ctrl+F`; keys go to search prompt.

Mode transitions:
- `Editing → Selecting` on any `Select*` command.
- `Selecting → Editing` on any non-select movement, `InsertChar`, `Noop`, etc.
- `Editing → Searching` on `SearchStart`.
- `Searching → Editing` on `SearchAccept` or `SearchCancel`.

### `editor/cursor.rs` — CursorSet

**CursorSet** manages multiple cursors (currently single-cursor in practice):
- `cursors: Vec<CursorPosition>` — each has `(line, col)`.
- `primary: usize` — index of the primary cursor.
- `anchor: Option<(usize, usize)>` — selection anchor `(line, col)`.
- `selection_text: Option<String>` — cached selected text.
- `preferred_col: Option<usize>` — "sticky" column for up/down movement.

**Key methods:**
- `primary()` / `primary_mut()` — access the primary cursor.
- `set_primary(line, col)` — move primary, clear preferred_col.
- `start_selection(line, col)` — set anchor.
- `clear_selection()` — remove anchor and cached text.
- `selection_range()` — returns `((start_line, start_col), (end_line, end_col))`.
- `cache_selection_text(&Rope)` — extracts text between anchor and cursor.

Selections are **anchor-based**: the anchor stays fixed while the cursor moves.
Direction is determined by comparing anchor vs cursor position.

### `editor/mod.rs` — Editor State & Logic (~1125 lines)

Central struct fields:
- `buffer: Buffer` — the text buffer.
- `cursor: CursorSet` — cursor/selection state.
- `mode: Mode` — current editing mode.
- `scroll_row: usize` — first visible **visual row** (wrap-aware).
- `width / height: u16` — terminal dimensions.
- `config: Config` — loaded from TOML.
- `clipboard: Option<String>` — internal clipboard.
- `should_quit: bool` — exit flag.
- `status_message: String` — shown in status bar.
- `show_help: bool` — help overlay toggle.
- `search_query: String` — current search term.
- `search_matches: Vec<(usize, usize)>` — `(line, col)` of each match.
- `search_match_index: usize` — current match index.

**Command dispatch:** `execute(cmd)` matches on the `Command` enum and calls
the appropriate method. The match is exhaustive.

**Visual wrap model:**
The editor uses a `visual_row` concept for soft-wrapped lines. A buffer line
may span multiple visual rows. Key helper functions:

- `visual_row_of(line)` — sum of wrapped-row counts for all lines before `line`.
- `visible_line_count()` — height available for text (total height − 2 for
  status bar and help bar).
- `ensure_cursor_visible()` — adjusts `scroll_row` so the cursor's visual
  row is within `[scroll_off, height - scroll_off]`.
- `wrapped_line_count(line)` — how many visual rows a buffer line occupies
  given the current viewport width and gutter width.

**Navigation helpers:**
- `move_left/right/up/down()` — character + visual-row movement.
- `move_word_left/right()` — uses `unicode_segmentation::UnicodeSegmentation`
  for UAX #29 word boundaries.
- `move_home/end()` — start/end of buffer line.
- `page_up/down()` — scrolls by `visible_line_count()`.

**Editing operations:**
- All edits go through `buffer.insert_char()`, `buffer.delete_char_before()`, etc.
- Undo grouping: `buffer.history.start_group(kind)` /
  `buffer.history.end_group()`. Groups are started per-command, e.g. typing
  characters uses `EditKind::Insert`, backspace uses `EditKind::Delete`.
- After edits, `buffer.dirty = true`.

**Search:**
- `search_start()` → enters `Searching` mode, clears query.
- `search_input(ch)` / `search_backspace()` → updates query, calls
  `update_search_matches()` which does a linear scan.
- `search_next()` → cycles to next match, moves cursor.
- `search_accept()` → returns to `Editing` mode.
- `search_cancel()` → returns to `Editing`, restores original position.

### `buffer/mod.rs` — Buffer

Wraps `TextRope` + `History`:
- `text: TextRope` — the rope.
- `history: History` — undo/redo.
- `file_path: Option<PathBuf>` — file on disk.
- `dirty: bool` — unsaved changes flag.
- `line_count()` — delegates to rope.
- `line_len(idx)` — char count of a line (via rope).
- `insert_char(line, col, ch)` / `insert_str(line, col, s)`.
- `delete_char_at(line, col)` / `delete_char_before(line, col)`.
- `delete_range((l1,c1), (l2,c2))`.
- `merge_line_with_previous(line)` — joins lines at a newline boundary.
- `save()` — writes rope content to `file_path` via `std::fs::write`.

**All mutation methods** record `Edit` entries (Insert/Delete with position,
content, old text) in the history for undo support.

### `buffer/rope.rs` — TextRope

Thin wrapper around `ropey::Rope`:
- `new()` / `from_str()` — constructors.
- `len_chars()` / `len_lines()` — counts.
- `line(idx)` — returns `RopeSlice`.
- `line_len_chars(idx)` — char count for a line.
- `char_at(line, col)` — single char lookup.
- `insert_char(line, col, ch)` / `insert_str(line, col, s)`.
- `remove_range(start_char..end_char)` — char-index range removal.
- `line_to_char(line)` / `char_to_line(char_idx)` — conversions.
- `to_string()` — collect full content.
- `slice(range)` — returns a `RopeSlice`.

### `buffer/history.rs` — Undo/Redo

- **`Edit`** enum: `Insert { line, col, content }` or
  `Delete { line, col, content, deleted }`.
- **`EditKind`**: `Insert`, `Delete`, `Newline`, `Other` — used for grouping.
- **`EditGroup`**: a vec of edits + the kind. Consecutive same-kind edits
  merge into one group.
- **`History`**: `undo: Vec<EditGroup>`, `redo: Vec<EditGroup>`,
  `current_group: Option<EditGroup>`.
- `start_group(kind)` — begins a new group (flushes any open one first).
- `end_group()` — pushes the current group to the undo stack.
- `record(edit)` — appends to current group.
- `undo(text) → Option<Vec<Edit>>` — pops from undo, pushes to redo,
  returns edits to replay inverted.
- `redo(text) → Option<Vec<Edit>>` — symmetric.
- `flush()` — closes any open group.

### `input/mod.rs` — Key Mapping

`map_event(event, mode) → Command`:
- Mode-aware: in `Searching` mode, character keys become `SearchInput(ch)`,
  Backspace becomes `SearchBackspace`, etc.
- Modifier-aware: Ctrl, Shift, Alt combinations. Uses crossterm's
  `KeyModifiers` bitflags.
- Ctrl+C = `ForceQuit`, Ctrl+Shift+C = `Copy` (distinguished by shift flag).
- Handles `Event::Paste(s)` → `Command::Paste(s)`.
- Unrecognized keys → `Command::Noop`.

### `render/mod.rs` — Rendering (~830 lines)

Single `render(editor, writer)` function orchestrates all drawing:
1. Hides cursor.
2. Draws text area (line-by-line, wrap-aware).
3. Draws status bar.
4. Draws help bar (bottom row).
5. Positions cursor at the correct visual (col, row) accounting for wrapping,
   gutter width, and Unicode char widths.
6. Shows cursor.
7. Flushes writer.

**Key rendering details:**
- **Gutter:** line numbers right-aligned, width = `log10(total_lines) + 1`.
  Gutter is highlighted blue for the cursor line.
- **Soft wrapping:** lines that exceed viewport width wrap to the next
  visual row. The renderer tracks a `visual_row` counter and skips rows
  before `scroll_row`.
- **Selection highlighting:** selected text gets a dark blue background.
  Selection range is computed from `cursor.selection_range()`.
- **Search match highlighting:** all matches get a yellow/dark background;
  the current match gets a yellow background with dark text.
- **CJK/wide chars:** each char's display width comes from
  `utils::char_width()` (delegates to `unicode_width::UnicodeWidthChar`).
  When a wide char would split across the wrap boundary, the renderer
  inserts padding space at line end.
- **Tab rendering:** tabs are expanded to spaces based on `config.tab_width`
  and current column position (tab stops).
- **Status bar:** left = mode + filename + modified flag + line/col;
  right = file info.
- **Help bar:** pico/nano-style bottom bar showing key shortcuts.

### `config/mod.rs` — Configuration

`Config` struct with serde `Deserialize`:
- `tab_width: usize` (default 4)
- `expand_tab: bool` (default false)
- `line_numbers: bool` (default true)
- `scroll_off: usize` (default 5)
- `theme: String` (default "default")

Loaded from `dirs::config_dir() / "dan" / "config.toml"`.
Falls back to defaults if file missing or parse error.

### `syntax/mod.rs` — Syntax Highlighting (Stub)

`SyntaxHighlighter` struct with a `highlight_line()` method that currently
returns an empty vec. Placeholder for tree-sitter integration.

### `utils.rs` — Utilities

`char_width(ch: char) -> usize`:
- 0 for control chars (< 0x20, except tab which is handled elsewhere).
- Delegates to `UnicodeWidthChar::width()` for everything else.
- Falls back to 1 for `None` (unusual chars).

---

## Dependencies

| Crate                  | Version | Purpose                                   |
|------------------------|---------|-------------------------------------------|
| `crossterm`            | 0.29    | Terminal I/O, raw mode, events, colors    |
| `ropey`                | 1.6     | Rope data structure for text buffer       |
| `serde`                | 1.0     | Config deserialization (with `derive`)    |
| `toml`                 | 1.0     | TOML config file parsing                  |
| `dirs`                 | 6.0     | Platform-specific config directory paths  |
| `unicode-width`        | 0.2     | Character display width (CJK, emoji)     |
| `unicode-segmentation` | 1.12    | UAX #29 word boundary detection           |

No C dependencies. No runtime deps. No async runtime.

---

## Coding Conventions

### Rust Style
- **Edition 2021**, no nightly features.
- **Indentation: TABS (hard tabs), NOT spaces.** Enforced by `rustfmt.toml`
  (`hard_tabs = true`) and `.editorconfig` (`indent_style = tab`).
  All `.rs` files use tabs. Cargo.toml uses 2 spaces (TOML convention).
- All public items should have `///` doc comments.
- Use `snake_case` for functions/variables, `PascalCase` for types/enums.
- Keep modules focused: one concept per file.
- No `unwrap()` in production paths — use `?` or explicit error handling.
- `#[cfg(test)] mod tests` at the bottom of files.

### Architecture Rules
1. **Commands are the API surface:** Never handle raw key events in
   `editor/mod.rs`. All behavior goes through the `Command` enum.
2. **Edits go through Buffer:** Never mutate the rope directly from
   Editor. Use `Buffer`'s methods so edits are recorded in history.
3. **Rendering is read-only:** `render::render()` takes `&Editor` (immutable).
   It must never mutate editor state. The only exception is the writer flush.
4. **Unicode widths everywhere:** When calculating column positions or
   display offsets, always use `utils::char_width()` or the rope's
   char-iteration methods. Never assume 1 char = 1 column.
5. **Visual rows ≠ buffer lines:** All scrolling and cursor positioning
   must account for soft wrapping. Use `visual_row_of()` and
   `wrapped_line_count()`.

### Test Conventions
- Tests use `#[cfg(test)]` modules within each source file.
- Rope tests: create a `TextRope::from_str()`, perform operations, assert
  on `to_string()` or `line_len_chars()`.
- Cursor tests: create a `CursorSet`, manipulate, assert positions.
- Tests should be fast (no I/O, no terminal).

### Commit Conventions
- One logical change per commit.
- `cargo test` must pass.
- `cargo clippy` must be clean.

---

## Common Tasks

### Adding a New Command

1. Add a variant to `Command` enum in `editor/commands.rs`.
2. Add the key mapping in `input/mod.rs` (`map_event` function).
3. Add the match arm in `Editor::execute()` in `editor/mod.rs`.
4. Implement the handler method on `Editor` (if non-trivial).
5. Add tests.

### Adding a Config Option

1. Add the field (with serde default) to `Config` in `config/mod.rs`.
2. Use it in the appropriate module via `editor.config.your_option`.
3. Document it in `config.toml` and `README.md`.

### Extending the Renderer

1. All rendering happens in `render/mod.rs`.
2. Follow the existing pattern: compute visual rows, skip rows before
   `scroll_row`, draw chars one at a time with width tracking.
3. For new highlighting: add a new color state and check it during
   char-by-char rendering in the `render_visible_lines()` helper.

### Implementing Syntax Highlighting

1. The stub is in `syntax/mod.rs` — `SyntaxHighlighter`.
2. Add `tree-sitter` and language grammars to `Cargo.toml`.
3. `highlight_line()` should return a `Vec<(Range<usize>, Style)>`.
4. The renderer should use these spans during char-by-char drawing.

---

## Build & Run

**Platform:** macOS 15.x (aarch64-apple-darwin)  
**Toolchain:** stable Rust via rustup (`~/.cargo/bin/cargo`)  
**Project path:** `/Users/dfallman/dev/dan`

```bash
# All commands run natively — no container, no orb prefix.
# Cwd: /Users/dfallman/dev/dan

# Build
cargo build --release

# Run
cargo run -- myfile.txt

# Test
cargo test

# Lint
cargo clippy

# Version
cargo run -- --version
```

> [!IMPORTANT]
> Previously this project ran inside an OrbStack Linux VM.
> As of 2026-03-24 it runs **natively on macOS**.
> Run `cargo`, `rustc`, `clippy` directly — no `orb run` wrapper needed.

The `build.rs` script embeds the short git hash as `GIT_HASH` env var.
It re-runs when `.git/HEAD`, `.git/refs/`, or `VERSION` changes.

---

## Tricky Areas / Gotchas

1. **Visual row math:** `scroll_row` is in visual-row space, not buffer-line
   space. Functions like `visual_row_of()` sum `wrapped_line_count()` for each
   preceding line. This is O(n) — could be cached if performance is an issue
   with very large files.

2. **Wide char wrapping:** When a CJK character (width 2) would land on the
   last column of a visual row, the renderer inserts a padding space and wraps
   the character to the next row. The cursor positioning logic must match.

3. **Tab stops:** Tabs expand to `tab_width - (col % tab_width)` spaces.
   This means tab display width depends on column position. Both the editor
   (for cursor movement) and the renderer must use the same formula.

4. **Selection with wrapping:** Selection range is stored as buffer
   `(line, col)` positions, but rendering must highlight the correct visual
   rows. The renderer converts buffer positions to visual positions during
   drawing.

5. **Undo grouping:** Groups are keyed by `EditKind`. Starting a new group
   of a different kind flushes the current group. Callers must call
   `end_group()` when an editing command completes — forgetting this
   causes edits to silently merge.

6. **Bracketed paste:** The terminal must support bracketed paste for
   multi-line paste to arrive as `Event::Paste(String)`. Without it,
   each character arrives as a separate `Event::Key`, and the burst-drain
   loop in `run_loop` handles it (but it's slower).

7. **`preferred_col`:** When moving up/down, the cursor should "remember"
   the column it started at (e.g., moving down from col 40 past a short line
   should return to col 40 on the next long line). This is stored in
   `CursorSet.preferred_col` and cleared on horizontal movement.

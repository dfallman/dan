# dan

**Dan** is a friendly, familiar, and fast terminal text editor written in Rust.

It works the way you already know — type to insert text, use arrow keys to move, `Ctrl+S` to save, `Ctrl+Z` to undo. No modes to learn, no cheat sheet needed. If you can use Notepad or nano, you can use Dan.

Under the hood it uses a rope data structure for efficient editing of large files, full Unicode support (CJK, emoji, combining marks), and a batched rendering pipeline that stays smooth even over SSH.

---

## Features

- **No modes** — start typing immediately, just like nano or a GUI editor
- **GUI-style keybindings** — `Ctrl+C`/`Ctrl+V` clipboard, `Ctrl+Z`/`Ctrl+Y` undo/redo, `Ctrl+F` search
- **CJK & Unicode** — correct display widths for Chinese/Japanese/Korean characters, fullwidth forms, combining marks, and emoji
- **Rope-backed buffer** — O(log n) insert/delete, handles large files without lag
- **Undo/redo** with edit grouping — related keystrokes are bundled into a single undo step
- **Selections** — `Shift+Arrow` to select text, `Ctrl+Shift+Arrow` to select by word, `Ctrl+A` to select all
- **Clipboard** — `Ctrl+X` cut, `Ctrl+Shift+C` copy, `Ctrl+V` paste
- **Bracketed paste** — multi-line paste from your system clipboard arrives as a single event, no garbled text
- **Incremental search** — `Ctrl+F` to search, `Ctrl+G` to jump to next match
- **Line operations** — `Alt+Up/Down` to move lines, `Ctrl+K` to delete a line, `Ctrl+D` to duplicate
- **Tab / Dedent** — `Tab` inserts a tab (or spaces), `Shift+Tab` dedents the current line
- **TOML configuration** — customise tab width, expand tabs, line numbers, scroll padding, and theme
- **Status bar** — shows file name, cursor position, mode, and contextual messages
- **Git hash in version** — `dan --version` prints the version and commit hash
- **Cross-platform** — runs on Linux, macOS, and Windows (any terminal that supports ANSI)

---

## Installation

### From source (recommended)

You need [Rust 1.70+](https://rustup.rs/) installed.

```bash
git clone https://github.com/yourusername/dan.git
cd dan
cargo build --release
```

The binary will be at `target/release/dan`. Copy it somewhere on your `$PATH`:

```bash
cp target/release/dan ~/.local/bin/
```

### Quick run (without installing)

```bash
cargo run -- myfile.txt
```

---

## Usage

```
dan [OPTIONS] [FILE]
```

| Argument     | Description                          |
|--------------|--------------------------------------|
| `FILE`       | Open a file (created on first save if it doesn't exist) |
| `-v`, `--version` | Print version and git commit hash, then exit |

### Examples

```bash
# Open an existing file
dan src/main.rs

# Start with an empty scratch buffer
dan

# Create a new file (saved on Ctrl+S)
dan notes.md

# Check version
dan --version
# dan 0.1.2 (a3b8c1d)
```

---

## Keybindings

Dan uses familiar GUI-style shortcuts. No modes — every key works the same way at all times.

### Navigation

| Key                  | Action                              |
|----------------------|-------------------------------------|
| `←` `→` `↑` `↓`    | Move cursor                         |
| `Home`               | Move to start of line               |
| `End`                | Move to end of line                 |
| `Ctrl+Home`          | Move to start of file               |
| `Ctrl+End`           | Move to end of file                 |
| `Ctrl+←`             | Move to previous word               |
| `Ctrl+→`             | Move to next word                   |
| `Alt+←`              | Move to previous word (alternate)   |
| `Alt+→`              | Move to next word (alternate)       |
| `Page Up`            | Scroll up one page                  |
| `Page Down`          | Scroll down one page                |

### Selection

| Key                    | Action                            |
|------------------------|-----------------------------------|
| `Shift+←` `→` `↑` `↓` | Extend selection by character/line |
| `Shift+Home`           | Select to start of line           |
| `Shift+End`            | Select to end of line             |
| `Ctrl+Shift+←`         | Select to previous word           |
| `Ctrl+Shift+→`         | Select to next word               |
| `Alt+Shift+←` `→`      | Select by word (alternate)       |
| `Alt+Shift+↑` `↓`      | Extend selection by line          |
| `Ctrl+A`               | Select all                        |

### Editing

| Key                  | Action                              |
|----------------------|-------------------------------------|
| Any character        | Insert text at cursor               |
| `Enter`              | Insert newline                      |
| `Tab`                | Insert tab (or spaces if configured)|
| `Shift+Tab`          | Dedent current line                 |
| `Backspace`          | Delete character before cursor      |
| `Delete`             | Delete character after cursor       |
| `Ctrl+K`             | Delete entire line                  |
| `Ctrl+D`             | Duplicate line (or selection)       |
| `Alt+↑`              | Swap current line up                |
| `Alt+↓`              | Swap current line down              |

### Clipboard & Undo

| Key                  | Action                              |
|----------------------|-------------------------------------|
| `Ctrl+Shift+C`       | Copy selection                      |
| `Ctrl+X`             | Cut selection                       |
| `Ctrl+V`             | Paste                               |
| `Ctrl+Z`             | Undo                                |
| `Ctrl+Y`             | Redo                                |

### Search

| Key                  | Action                              |
|----------------------|-------------------------------------|
| `Ctrl+F`             | Open search prompt                  |
| `Ctrl+G`             | Jump to next search match           |

### File & Misc

| Key                  | Action                              |
|----------------------|-------------------------------------|
| `Ctrl+S`             | Save file                           |
| `Ctrl+Q`             | Quit (prompts if unsaved changes)   |
| `Ctrl+C`             | Force quit (discards changes)       |
| `Ctrl+H`             | Toggle help overlay                 |

---

## Configuration

Dan reads its configuration from `~/.config/dan/config.toml`. If the file doesn't exist, sensible defaults are used.

To get started, copy the example config:

```bash
mkdir -p ~/.config/dan
cp config.toml ~/.config/dan/config.toml
```

### Options

```toml
# Tab width in spaces (default: 4)
tab_width = 4

# Expand tabs to spaces on insert (default: false)
# true  = pressing Tab inserts spaces
# false = pressing Tab inserts a literal tab character
expand_tab = false

# Show line numbers in the gutter (default: true)
line_numbers = true

# Scroll padding — lines to keep visible above/below cursor (default: 5)
scroll_off = 5

# Color theme (default: "default")
theme = "default"
```

### Config locations by OS

| OS      | Path                                  |
|---------|---------------------------------------|
| Linux   | `~/.config/dan/config.toml`           |
| macOS   | `~/Library/Application Support/dan/config.toml` |
| Windows | `%APPDATA%\dan\config.toml`           |

The path is resolved using the [`dirs`](https://crates.io/crates/dirs) crate's `config_dir()`.

---

## Architecture

Dan is structured as a set of loosely coupled modules:

```
src/
├── main.rs          Entry point, CLI args, event loop
├── utils.rs         Unicode character width utilities
├── buffer/
│   ├── mod.rs       Buffer (text + file path + dirty flag + edit operations)
│   ├── rope.rs      TextRope — wrapper around `ropey` for O(log n) editing
│   └── history.rs   Undo/redo with edit grouping
├── config/
│   └── mod.rs       TOML config loading from ~/.config/dan/
├── editor/
│   ├── mod.rs       Core editor state and command execution
│   ├── commands.rs  Command enum (all possible actions)
│   ├── cursor.rs    Cursor positions, multi-cursor support, selections
│   └── mode.rs      Editor modes (Editing / Selecting)
├── input/
│   └── mod.rs       Keybinding map (crossterm events → Commands)
├── render/
│   └── mod.rs       Terminal rendering (gutter, text, status bar, cursor)
└── syntax/
    └── mod.rs       Syntax highlighting stub (placeholder for future work)
```

### Design decisions

- **Rope data structure** — The text buffer uses [ropey](https://crates.io/crates/ropey), a B-tree–based rope that gives O(log n) insert and delete at any position. This means editing a 100MB log file is just as responsive as editing a 10-line script.

- **Unicode-correct rendering** —  Character display widths are computed using the [unicode-width](https://crates.io/crates/unicode-width) crate, which correctly handles CJK ideographs (2 columns), combining marks (0 columns), fullwidth forms, and control characters. Word movement uses [unicode-segmentation](https://crates.io/crates/unicode-segmentation) for proper UAX #29 word boundaries.

- **Command pattern** — All user actions are represented as a `Command` enum. The input layer maps key events to commands, and the editor executes them. This decouples keybindings from behavior — rebinding keys means changing the mapping, not the editor logic.

- **Batched rendering** — The event loop drains all pending key events before rendering. This collapses rapid bursts (fast typing, unbracketed paste) into a single screen update, preventing flicker and wasted draws.

- **64KB buffered writer** — Terminal output goes through a `BufWriter` with a 64KB buffer, minimizing syscalls and keeping rendering fast even over high-latency SSH connections.

---

## Development

### Building

```bash
# Debug build (fast compilation, slow binary)
cargo build

# Release build (slow compilation, fast binary)
cargo build --release
```

### Running tests

```bash
cargo test
```

There are currently 25 tests covering the rope, cursor/selection, mode, and Unicode width utilities.

### Versioning

The version is stored in the `VERSION` file at the project root. The build script (`build.rs`) also embeds the short git commit hash, so `dan --version` shows e.g. `dan 0.1.2 (a3b8c1d)`.

To bump the version, edit the `VERSION` file.

### Project structure

| File / Dir    | Purpose                                           |
|---------------|----------------------------------------------------|
| `Cargo.toml`  | Rust package manifest and dependencies             |
| `build.rs`    | Build script — embeds git hash into the binary     |
| `VERSION`     | Single-source-of-truth version string              |
| `config.toml` | Example configuration file                         |
| `src/`        | All Rust source code                               |

---

## Dependencies

Dan uses a deliberately small set of well-maintained Rust crates:

| Crate                 | Purpose                                     |
|-----------------------|---------------------------------------------|
| `crossterm`           | Cross-platform terminal I/O and key events  |
| `ropey`               | Rope data structure for the text buffer     |
| `serde` + `toml`      | Configuration file parsing                  |
| `dirs`                | Platform-specific config directory paths    |
| `unicode-width`       | Correct display width for CJK, emoji, etc.  |
| `unicode-segmentation`| UAX #29 word boundary detection             |

No C dependencies. No build toolchain beyond `cargo`. No runtime dependencies.

---

## License

MIT

---

## Contributing

Contributions are welcome. If you're not sure whether a change fits, open an issue first to discuss.

When submitting a PR:
1. Make sure `cargo test` passes
2. Make sure `cargo clippy` has no warnings
3. Keep commits focused — one logical change per commit

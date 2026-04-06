# Dan

Dan is a modeless, terminal-native text editor engineered in Rust for users who prefer GUI-style workflows (like `Ctrl+S`, `Ctrl+C/V`). It provides a consistent, low-latency editing experience with no strange modes to learn, no archaic shortcuts, and no massive dotfiles required to get started.

Whether you are working locally or maintaining source code over an unstable SSH connection, Dan is explicitly designed to remain highly responsive.

## Performance & Reliability

- **Smart Differential Rendering**: Dan computes delta changes and only pushes updated bytes to the terminal emulator. This strict differential approach minimizes I/O overhead, keeping navigation smooth even when editing large files over constrained SSH connections.
- **Rope Data Structure**: Standard Strings degrade when handling massive files. Dan uses a Rope data structure to maintain a constant memory footprint and consistent line-modification speeds, regardless of file scale.
- **Fault-Tolerant Autosave**: Changes are actively flushed to a background `.swp` file every 5 seconds. In the event of a terminal crash or sudden power failure, your unsaved progress is reliably preserved for easy recovery upon reopening.

## The Modern Workflow

- **Non-Blocking Formatting (`Ctrl+L`)**: Dan securely pipes your buffer through external, industry-standard code formatters (like Prettier, Ruff, or Rustfmt) asynchronously. Execution happens in the background, ensuring you can continue editing without stutter.
- **Syntax-Aware Commenting (`Ctrl+/`)**: Dan natively parses the syntax metadata of your active file to seamlessly toggle language-specific block or line comments.
- **Automatic Pair Insertion**: Writing structural code is accelerated by automatic bracket and quote closures, including the ability to wrap existing selections intelligently.
- **Unicode First**: Dan reliably processes complex UTF-8 and CJK encodings, correctly rendering double-width characters and emojis without mangling the terminal's visual alignment grid.

## Keybindings

Dan utilizes familiar native keystrokes, categorized for optimal efficiency.

### General
| **Key**               | **Action**                                               |
| --------------------- | -------------------------------------------------------- |
| `Ctrl` + `S`          | **Save**: Write changes to disk.                         |
| `Ctrl` + `Q`          | **Quit**: Safe exit (prompts to save).                   |
| `Ctrl` + `Shift` + `Q`| **Force Quit**: Exit immediately and flush recovery file.|
| `Ctrl` + `H`          | **Show Help**: Open the built-in reference bar.          |

### Editing
| **Key**               | **Action**                                                       |
| --------------------- | ---------------------------------------------------------------- |
| `Ctrl` + `C` / `X` / `V`| **Clipboard**: Standard Copy, Cut, and Paste.                  |
| `Ctrl` + `Z` / `Y`      | **Undo / Redo**: Infinite, persistent undo history.            |
| `Shift` + `Arrows`      | **Select**: Highlight text blocks precisely.                   |
| `Ctrl` + `W`          | **Word Wrap**: Toggle between soft-wrapping and horizontal scroll. |
| `Alt` + `↑` / `↓`     | **Move Line**: Slide the current line or active block up and down. |

### Navigation & Search
| **Key**               | **Action**                                               |
| --------------------- | -------------------------------------------------------- |
| `Ctrl` + `↑` / `↓`    | **Scroll**: Move the viewport without moving the cursor. |
| `Ctrl` + `Shift` + `↑` / `↓`| **Quick Scroll**: Accelerated viewport navigation. |
| `Ctrl` + `F`          | **Find**: Inline search within the current buffer.       |
| `Ctrl` + `R`          | **Replace**: Interactive search and replace.             |
| `Ctrl` + `G`          | **Go-To Line**: Fast jump to an exact line marker.       |
| `Ctrl` + `L`          | **Format**: Run the external code formatter pipeline.    |

## Configuration

Dan utilizes a deterministic **Layered Configuration** system that prioritizes contextual settings:

1. **Internal Defaults**: Hardcoded, reliable baseline parameters.
2. **Global Config** (`~/.config/dan/config.toml`): User-level overrides.
3. **Local Project Style** (`.editorconfig`): Native directory-level configuration ensuring consistent repository styling.

### Example `~/.config/dan/config.toml`

```toml
# Display Settings
wrap_lines = true       # Toggle word wrapping
tab_width = 4           # Indentation depth
expand_tab = false      # Expand tabs to raw spaces
line_numbers = true     # Render gutter numbers
highlight_active = true # Highlight the active line background

# Editor Features
auto_close = true       # Auto-pair (), [], {}
scroll_off = 5          # Number of lines to pad above/below the cursor

# Theme Configuration
# If "default", Dan queries the terminal and auto-assigns an optimal theme.
theme = "default"       
```

### Themes and Syntax Highlighting

Dan ships with a highly optimized embedded syntax parser and supports 20+ industry-standard themes out of the box (including Dracula, Nord, Solarized, and Sublime Snazzy). If you leave the theme set to `"default"`, Dan communicates with your terminal emulator to automatically detect your environment and adapt to your precise light or dark mode setup.

*Note: For the formatting pipeline (`Ctrl+L`), you should install your desired execution binaries like [rustfmt](https://github.com/rust-lang/rustfmt), [ruff](https://docs.astral.sh/ruff/), or [prettier](https://prettier.io/) globally on your machine.*

## Installation

Dan is built with Rust. To compile it from source, ensure you have the official toolchain installed via [rustup.rs](https://rustup.rs/) (Minimum Supported Rust Version: **1.94**).

### macOS & Linux

```bash
git clone https://github.com/dfallman/dan.git
cd dan
cargo build --release

# Move to your local path
cp target/release/dan /usr/local/bin/

# ...alternatively, install via Cargo
cargo install --path .
```

### Windows (PowerShell)

```powershell
git clone https://github.com/dfallman/dan.git
cd dan
cargo build --release

# Move to your Cargo bin for easy access
Copy-Item target\release\dan.exe ~/.cargo/bin/
```

## Planned Improvements

While Dan prioritized speed, stability, and security for its foundational architecture, we are actively tracking the following capability expansions:

- **Mouse Support**: Click-to-position, scroll wheel rendering, and click-drag selections.
- **Regex Search**: Enable regular expression evaluation in the find/replace pipeline.
- **Extended Markdown Support**: Enhanced semantic rendering for bold, italic, and tabular structures.
- **Buffer Switching**: Multi-buffer plumbing, though tools like Zellij and tmux handle multiplexing well today.
- **Vertical Split**: Dual-pane rendering for viewing two files or regions simultaneously.
- **Suspend & Resume**: POSIX SIGTSTP handling for cleanly toggling between the editor sequence and the shell.
- **Configurable Keybindings**: Decoupling the input matrix for custom shortcut mappings.
- **Multi-Cursor**: Synchronous multi-line text mutation.

---
**License**: MIT

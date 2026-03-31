# dan

**Dan** is a friendly, lightning-fast, and modern terminal text editor. Written natively in **Rust**, it is designed to be completely modeless and zero-latency. It ships with sensible defaults that just work.

The goal of Dan is simple: to provide a no-fuss editing experience that works exactly like the modern GUI editors you already know, but optimized for the terminal. Whether you are working locally or over a fluctuating SSH connection, Dan stays responsive without dropping a single frame.

No strange modes to learn, no archaic shortcuts, and no massive configuration files. 

---

## Key features

### High-performance architecture

- **Smart rendering engine**: Dan uses a differential rendering system. By computing exactly what has changed on your screen, it only sends the necessary updates to your terminal. This makes scrolling 100MB files over SSH feel as smooth as local editing.

- **Fluid 60FPS experience**: With built-in event debouncing, Dan collapses rapid keystrokes and large pastes into an efficient render loop. This preserves **battery life** and ensures a stutter-free experience.

- **Rock-solid stability**: Using a "Rope" data structure, Dan handles massive files with a constant memory footprint. It also features a **5-second background autosave**—if your terminal crashes or you lose power, your work is safely tucked away in a `.swp` file for easy recovery.

### Tools for developers, useful for everyone

- **Effortless formatting (`Ctrl+L`)**: Clean up your code instantly. Dan pipes your text through industry-standard tools like **Prettier**, **Ruff**, or **Rustfmt** in the background. It’s non-blocking, so you can keep typing while it works.

- **Smart selection & commenting**: Use `Ctrl+/` to toggle language-specific comments across multi-line selections. Dan naturally understands the syntax logic of your file.

- **Automatic pair insertion**: Save keystrokes with auto-closing brackets and quotes. If you highlight a block of code and type a bracket, Dan will wrap the selection for you.
### International & Adaptive

- **Full Unicode & CJK support**: Dan handles Chinese, Japanese, and Korean characters perfectly (or, at least, that's the plan), maintaining correct visual alignment even with double-width characters and emojis.

- **Automatic encoding detection**: Opening an old file? Dan intelligently "sniffs" legacy formats (like Shift-JIS or Windows-1252) and converts them to clean UTF-8 for editing.

---

## Keyboard shortcuts

Dan uses familiar shortcuts so you don't need a cheat sheet.

### Navigation

| **Key**              | **Action**                                               |
| -------------------- | -------------------------------------------------------- |
| `Ctrl` + `H`         | **Show Help**: Open the built-in reference.              |
| `Ctrl` + `S`         | **Save**: Write changes to disk.                         |
| `Ctrl` + `Q`         | **Quit**: Safe exit (prompts to save).                   |
| `Ctrl` + `Shift` + `Q` | **Force Quit**: Exit immediately and save recovery file. |
| `Ctrl` + `↑` / `↓`     | **Scroll**: Move the view without moving the cursor.     |
| `Ctrl` + `Shift` + `↑` / `↓`     | **Quick scroll**: Scroll faster up and down.     |

### Editing

| **Key**              | **Action**                                                       |
| -------------------- | ---------------------------------------------------------------- |
| `Ctrl` + `C` / `X` / `V` | **Clipboard**: Standard Copy, Cut, and Paste.                    |
| `Ctrl` + `Z` / `Y`     | **Undo / Redo**: Infinite, persistent history.                   |
| `Shift` + `Arrows`   | **Select**: Highlight text naturally.                            |
| `Ctrl` + `W`        | **Word Wrap**: Toggle between wrapping and horizontal scrolling. |
| `Alt` + `↑` / `↓`      | **Move Line**: Slide the current line or selection up/down.      |

### Search & Productivity

| **Key**      | **Action**                                      |
| ------------ | ----------------------------------------------- |
| `Ctrl` + `F` | **Find**: Search within the current file.       |
| `Ctrl` + `R` | **Replace**: Interactive search and replace.    |
| `Ctrl` + `G` | **Go-To Line**: Jump to a specific line number. |
| `Ctrl` + `L` | **Format**: Run your configured code formatter. |

---

## Installation

You will need [Rust 1.70+](https://rustup.rs/) to compile Dan from source.

### macOS & Linux

Bash

```
git clone https://github.com/dfallman/dan.git
cd dan
cargo build --release

# Move to your local path
cp target/release/dan ~/.local/bin/

```

### Windows (PowerShell)

PowerShell

```
git clone https://github.com/dfallman/dan.git
cd dan
cargo build --release

# Move to your Cargo bin for easy access
Copy-Item target\release\dan.exe ~/.cargo/bin/

```

---

## ⚙️ Configuration

Dan follows a "Layered Configuration" model. It looks for settings in this order:

1. **Internal Defaults** (The baseline).
2. **Global Config** (`~/.config/dan/config.toml`).
3. **Local Project Style** (`.editorconfig`).

### Unified Configuration Architecture (V2)
Dan is built around a singular "ConfigBuilder" schema mapping perfectly across the entire codebase. When a file is opened, Dan explicitly sniffs its format structure, parses your `.editorconfig` matrices (handling spacing limits, whitespace trimming, line endings natively) and merges them flawlessly into your global layout limits!

*Note: For strict security and reliability, Dan explicitly ignores generic `./config.toml` files found in terminal directories natively. This guarantees another tool's TOML format never accidentally overrides your editor's runtime structure.*

### Global Settings (`config.toml`)

You can customize your experience by editing the TOML file:

```
wrap_lines = true       # Wrap long lines or scroll?
tab_width = 4           # Spaces per tab
expand_tab = false      # Expand tabs to spaces
line_numbers = true     # Show gutter numbers
highlight_active = true # Highlight the current line
scroll_off = 5          # Lines to keep visible above/below cursor
fast_scroll_steps = 10  # Number of lines to scroll when using fast scroll
auto_close = true       # Auto-pair (), [], {}, etc.
show_help = true        # Always show help in the toolbar
show_encoding = true    # Show detected character encoding in the toolbar
show_lang = true        # Show detected programming language in the toolbar
theme = "default"       # Color scheme

```

### Formatter Setup

To get the most out of liting using **Ctrl+L**, ensure your system has the following tools installed:

- For **Rust**: [rustfmt](https://github.com/rust-lang/rustfmt)
- For **Python**: [ruff](https://docs.astral.sh/ruff/) (`pip install ruff`)
- For **Web/JS/JSON**: [prettier](https://prettier.io/) (`npm i -g prettier`)

---

**License**: MIT

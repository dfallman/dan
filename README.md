# dan

**Dan** is a friendly, lightning-fast, and modern terminal text editor. Written natively in **Rust**, it is designed to be completely modeless and zero-latency. Dan ships with sensible defaults.

No strange modes to learn, no archaic shortcuts, and no massive configuration files. 

The goal of Dan is simple: to provide a no-fuss editing experience that works exactly like the modern GUI editors you already know, but optimized for the terminal. Whether you are working locally or over a fluctuating SSH connection, Dan stays responsive without dropping a single frame.


# Features

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


# Keyboard shortcuts

Dan uses familiar shortcuts so you don't need a cheat sheet.

## Navigation

| **Key**              | **Action**                                               |
| -------------------- | -------------------------------------------------------- |
| `Ctrl` + `H`         | **Show Help**: Open the built-in reference.              |
| `Ctrl` + `S`         | **Save**: Write changes to disk.                         |
| `Ctrl` + `Q`         | **Quit**: Safe exit (prompts to save).                   |
| `Ctrl` + `Shift` + `Q` | **Force Quit**: Exit immediately and save recovery file. |
| `Ctrl` + `↑` / `↓`     | **Scroll**: Move the view without moving the cursor.     |
| `Ctrl` + `Shift` + `↑` / `↓`     | **Quick scroll**: Scroll faster up and down.     |

## Editing

| **Key**              | **Action**                                                       |
| -------------------- | ---------------------------------------------------------------- |
| `Ctrl` + `C` / `X` / `V` | **Clipboard**: Standard Copy, Cut, and Paste.                    |
| `Ctrl` + `Z` / `Y`     | **Undo / Redo**: Infinite, persistent history.                   |
| `Shift` + `Arrows`   | **Select**: Highlight text naturally.                            |
| `Ctrl` + `W`        | **Word Wrap**: Toggle between wrapping and horizontal scrolling. |
| `Alt` + `↑` / `↓`      | **Move Line**: Slide the current line or selection up/down.      |

## Search & Productivity

| **Key**      | **Action**                                      |
| ------------ | ----------------------------------------------- |
| `Ctrl` + `F` | **Find**: Search within the current file.       |
| `Ctrl` + `R` | **Replace**: Interactive search and replace.    |
| `Ctrl` + `G` | **Go-To Line**: Jump to a specific line number. |
| `Ctrl` + `L` | **Format**: Run your configured code formatter. |


# Installation

You will need the latest [Rust](https://rustup.rs/) to compile Dan from source. (1.94 is recommended). On most system, installing using the package manager will not give you the latest version. Use [https://rustup.rs/](https://rustup.rs/) instead.

## macOS & Linux

Bash

```
git clone https://github.com/dfallman/dan.git
cd dan
cargo build --release

# Move to your local path
cp target/release/dan /usr/local/bin/

# ...or (macOS specifically)
cp target/release/dan ~/.local/bin/
```

## Windows (PowerShell)

PowerShell

```
git clone https://github.com/dfallman/dan.git
cd dan
cargo build --release

# Move to your Cargo bin for easy access
Copy-Item target\release\dan.exe ~/.cargo/bin/
```

# Configuration

Dan follows a "Layered Configuration" model. It looks for settings in this order:

1. **Internal Defaults** (sensible defaults).
2. **Global Config** (`~/.config/dan/config.toml`).
3. **Local Project Style** (`.editorconfig`).

## Unified Configuration Architecture (V2)
Dan is built around a singular "ConfigBuilder" schema mapping perfectly across the entire codebase. When a file is opened, Dan explicitly sniffs its format structure, parses your `.editorconfig` matrices (handling spacing limits, whitespace trimming, line endings natively) and merges them flawlessly into your global layout limits!

## Global Settings (`config.toml`)

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
theme = "default"       # Set color theme for syntax highlighting

# Default theme: "default"
# On startup, Dan sends a specific ANSI escape sequence query to your terminal emulator 
# to understand if the terminal is dark or light and pick a default theme.
# - for dark terminals, we use theme: "OneHalfDark"
# - for light terminals, we use theme: "OneHalfLight"
#
# Other available themes (Note: if you set one of these, Dan will use it regardless of terminal color):
# "1337" A retro, high-contrast dark theme inspired by old-school hacker culture.
# "Coldark-Cold" A clean, blue-tinted light theme.
# "Coldark-Dark" A deep, cool-blue dark theme.
# "DarkNeon" A vibrant dark theme bursting with bright neon accents.
# "Dracula" A popular, high-contrast dark theme with distinct purple and pink accents.
# "GitHub" A light theme accurately mimicking the classic GitHub code view.
# "Monokai Extended" An updated version of the classic, vivid Monokai theme.
# "Monokai Extended Bright" A brighter, higher-contrast variant of the Monokai palette.
# "Monokai Extended Light" A light-background adaptation of the Monokai colors.
# "Monokai Extended Origin" The authentic, unaltered original Monokai color palette.
# "Nord" A clean, arctic-inspired dark theme with frosty blue tones.
# "OneHalfDark" A clean, modern dark theme based heavily on the Atom "One" series (default for dark terminals).
# "OneHalfLight" A clean, modern light theme based on the Atom "One" series (default for light terminals).
# "Solarized (dark)" A very popular, scientifically formulated low-contrast dark theme.
# "Solarized (light)" The light-background version of the Solarized palette.
# "Sublime Snazzy" A vibrant dark theme with bright, elegant (snazzy) colors.
# "TwoDark" A dark theme inspired by Atom's One Dark but tuned for slightly better contrast.
# "Visual Studio Dark+" Accurately emulates the prominent default dark theme of VS Code.
# "ansi" A minimal, dynamic theme that falls back to your terminal's built-in 16 ANSI colors.
# "base16" A balanced, standard boilerplate dark theme from the base16 project.
# "base16-256" A variant of base16 specifically optimized for limited 256-color palette terminals.
# "gruvbox-dark" A retro "groove" color scheme with earthy, warm dark tones.
# "gruvbox-light" A retro "groove" color scheme with earthy, warm light tones.
# "zenburn" A low-contrast "alien" dark color scheme designed to be extremely easy on the eyes.
```

## Formatter Setup

To get the most out of linting (quicky formatting you file) using `Ctrl+L`, ensure your system has the following tools installed:

- For **Rust**: [rustfmt](https://github.com/rust-lang/rustfmt) (`rustup component add rustfmt`)
- For **Python**: [ruff](https://docs.astral.sh/ruff/) (`pip install ruff`)
- For **Web/JS/JSON**: [prettier](https://prettier.io/) (`npm i -g prettier`)

# Roadmap

Dan is a work in progress. The focus has been on speed, stability, efficiency and security over features and tweakability. While we don't see Dan ever becoming the world's most feature-rich text editor (which isn't our mission) we have the following ideas on our roadmap going forward:

- **Mouse support** — click-to-position, scroll wheel, click-drag select
- **Regex search** — toggle regex mode in search bar
- **Extended markdown support** — render some markdown script better, such as bold, italic, perhaps table
- **Buffer switching** — Dan has multi-buffer plumbing but currently only supports a single buffer at a time. While useful, tools like tmux, Zellij, and others make this less cruical and adds complexity, not least to the interface
- **Vertical split** — view two files or two parts of the same file
- **Suspend & resume** — SIGTSTP as a simple and effective way to switch between the editor and the terminal.
- **Configurable keybindings** — allow users to select their preferred keybindings
- **Multi-cursor** — allow multiple cursors to edit many lines/occurances simultaneously 
- **Built-in terminal** or shell pipe
- **Plugin interface** — in a v1, 'run external command, pipe selection'


---

**License**: MIT

# dan

A fast, friendly, lightweight yet highly capable terminal text editor written in Rust. Dan is modeless, requires no configuration to get started, and stays fast even over slow or unstable SSH connections and while working with very large files.

<p align="center">
  <img width="800" alt="screenshot of dan" src="https://github.com/user-attachments/assets/dba6a295-93fa-4da5-a109-b389e84e0bbb" />
</p>

## Quick install

Install or update [Rust](https://rustup.rs/):
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Clone, build, and install Dan:
```
git clone https://github.com/dfallman/dan.git
cd dan
cargo install --path .
```

For more installation options, see [Installation](#installation).

## Features

Dan uses familiar shortcuts out of the box — `Ctrl-C`/`V` to copy/paste, `Ctrl-S` to save, `Ctrl-Z`/`Y` to undo/redo, `Ctrl-Q` to quit. Press `Ctrl-H` to toggle the built-in help bar at any time.

- **Command palette** (`Ctrl-P`): A searchable overlay covering every editor action, all open buffers, and project files. Switch buffers, open files, or trigger any command without leaving the keyboard.

<p align="center">
  <img width="800" alt="Command palette" src="https://github.com/user-attachments/assets/4c3ca372-42cd-46f5-934a-26bcae4babcd" />
  <br>
  <em>Dan's command palette</em>
</p>

- **Multiple buffers**: Dan supports multiple open buffers simultaneously, managed through the command palette.

- **Rendering**: Differential rendering — only changed cells are written to the terminal. A syntax snapshot cache (taken every 200 lines) means scrolling deep into long files stays smooth, including over slow SSH links.

- **Text buffer**: Rope-backed, so inserts and deletes are O(log N) and memory use scales with edits rather than file size. Dan handles very large files without loading them into a flat string.

- **Syntax highlighting**: Powered by [syntect](https://github.com/trishume/syntect/), with broad language support. Dan queries your terminal's background color at startup and picks a sensible default theme. Override the theme via config, or toggle syntax highlighting on/off with `Ctrl-T`.

- **Auto-formatter** (`Ctrl-L`): Pipes the buffer through an external formatter (Prettier, Rustfmt, or Ruff) in a background thread. The result is applied only if the buffer hasn't been edited during formatting. See [Formatter](#formatter).

- **Search & replace**: `Ctrl-F` to search; press `Ctrl-R` while searching to promote to find-and-replace.

- **Atomic saves**: Saves go through a sibling temp file → fsync → rename. A crash or full disk mid-write leaves the file in its prior state rather than truncated. Original permissions and symlink targets are preserved.

- **Crash recovery**: Every 5 seconds, Dan writes the buffer to a hidden `.swp` file using the same safe write pattern. If the terminal crashes or SSH drops, reopening the file prompts for recovery.

- **Unicode & CJK support**: Correct visual alignment for double-width characters and emojis.

- **Native OS clipboard support**: Cross-platform via [arboard](https://docs.rs/arboard/latest/arboard/), with an in-memory fallback when no display server is available (e.g., headless SSH).

- **Auto-pairs**: Closing brackets and quotes auto-insert on typing; typing a bracket around an active selection wraps it.

- **File encoding detection**: Detects legacy encodings (Shift-JIS, Windows-1252, etc.) on open, works in UTF-8, and round-trips back to the original encoding on save.

- **Content safety**: Terminal escape sequences embedded in file content are sanitized at render time, so opening a hostile file can't write over the editor chrome or exfiltrate clipboard state.

- **Layered config**: Internal defaults → `~/.config/dan/config.toml` → `.editorconfig` in the project tree. See [Configuration](#configuration).

## Keyboard shortcuts

### Basic operation

| Key | Action |
|-----|--------|
| `↑` `↓` `←` `→` | Move cursor |
| `Ctrl` + `S` | Save |
| `Ctrl` + `A` | Save As |
| `Ctrl` + `Q` | Quit (prompts if there are unsaved changes) |
| `Ctrl` + `H` | Toggle help bar |
| `Ctrl` + `P` | Command palette (actions, buffers, project files) |

### Text editing

| Key | Action |
|-----|--------|
| `Ctrl` + `C` / `X` / `V` | Copy / Cut / Paste |
| `Ctrl` + `Z` / `Y` | Undo / Redo |
| `Ctrl` + `D` | Duplicate line or selection |
| `Ctrl` + `K` | Delete line or selection |
| `Ctrl` + `E` (or `Ctrl` + `/`) | Toggle comment (syntax-aware) |
| `Ctrl` + `T` | Toggle syntax highlighting |
| `Ctrl` + `W` | Toggle word wrap |
| `Ctrl` + `R` | Toggle whitespace markers |
| `Ctrl` + `L` | Format document |
| `Alt` + `↑` / `↓` | Move line up / down |
| `Tab` / `Shift` + `Tab` | Indent / Dedent |

### Selection

| Key | Action |
|-----|--------|
| `Ctrl` + `\` | Select all |
| `Shift` + `Arrows` | Extend selection |
| `Ctrl`/`Alt` + `Shift` + `←` / `→` | Extend selection by word |

### Navigation

| Key | Action |
|-----|--------|
| `Ctrl` + `↑` / `↓` | Scroll without moving cursor |
| `Ctrl` + `Shift` + `↑` / `↓` | Fast scroll |
| `Ctrl` / `Alt` + `←` / `→` | Jump by word |
| `Ctrl` + `Home` / `End` | Jump to start / end of file |
| `Ctrl` + `G` | Go to line |

### Search & replace

| Key | Action |
|-----|--------|
| `Ctrl` + `F` | Search |
| `Ctrl` + `R` *(while searching)* | Promote to find-and-replace |

**Note for macOS users**: Terminal emulators use escape sequences dating back to the late 70s and some at the time highly influential video display terminals such as VT100. Long story short, this means some "modern" key combinations available in GUI editors can't be distinguished in a terminal. Most notably, Dan (and other terminal apps) uses `Ctrl` where a Mac user might expect `⌘`. Many terminal emulators (including [iTerm2](https://iterm2.com/)) let you remap `⌘` to `Ctrl` if you prefer, although it can create side-issues. Additionally, the built-in Terminal.app is not recommended: a third-party emulator such as [iTerm2](https://iterm2.com/), [Kitty](https://sw.kovidgoyal.net/kitty/), [Ghostty](https://ghostty.dev/), or [WezTerm](https://wez.dev/) will give better results.

# Installation

Dan requires Rust v1.94 or later. We recommend installing via [rustup](https://rustup.rs/) rather than your system package manager, which often provides an older version:

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

To install on Windows, [follow these instructions](https://rustup.rs/#).

### macOS & Linux

```
git clone https://github.com/dfallman/dan.git
cd dan
cargo build --release
cp target/release/dan /usr/local/bin/
# or
cargo install --path .
```

### Windows

> **Note**: If you're running Dan inside WSL, follow the Linux instructions above instead.

```
git clone https://github.com/dfallman/dan.git
cd dan
cargo build --release
Copy-Item target\release\dan.exe ~/.cargo/bin/
```

# Configuration

Dan works without any configuration file. To customize it, create `~/.config/dan/config.toml` (on Windows: `C:\Users\<username>\AppData\Roaming\dan\config.toml`) and add the options you want to change. Full defaults are shown below for reference.

```
dan ~/.config/dan/config.toml
```

<p align="center">
  <img width="800" alt="Settings" src="https://github.com/user-attachments/assets/6c6515dc-ba43-4944-ab96-ee634bf26f0a" />
</p>

```toml
# Display
wrap_lines = true           # Wrap long lines (default: true)
tab_width = 4               # Visual tab width (default: 4)
expand_tab = false          # Insert spaces instead of tabs (default: false)
line_numbers = true         # Show line numbers (default: true)
highlight_active = true     # Highlight the current line (default: true)
scroll_off = 5              # Lines to keep visible above/below cursor (default: 5)
fast_scroll_steps = 10      # Lines jumped per fast-scroll keypress (default: 10)
show_full_path = false      # Show full file path in toolbar (default: false)

# Editing
auto_indent = true          # Match indentation of the previous line (default: true)
auto_close = true           # Auto-insert closing brackets and quotes (default: true)
syntax_highlight = true     # Enable syntax highlighting (default: true)

# Interface
show_help = true            # Show shortcut bar at the bottom (default: true)
show_encoding = true        # Show file encoding in status bar (default: true)
show_lang = true            # Show detected language in status bar (default: true)

# Theme
theme = "default"           # Syntax highlight theme; "default" auto-detects terminal background
comments_are_italics = true # Render comments in italics (default: true)
```

### Project-aware settings

Dan automatically picks up `.editorconfig` files in the project tree. Tab width, line endings, and trailing-whitespace rules defined there take precedence over your global config, so Dan adapts to each project's style without manual adjustment.

<img width="2080" height="1610" alt="CleanShot 2026-04-16 at 15 19 24@2x" src="https://github.com/user-attachments/assets/81620335-6df2-494c-ad9c-dca863df526e" />

## Themes

When `theme = "default"`, Dan queries your terminal's background color at startup and picks `OneHalfDark` for dark terminals or `OneHalfLight` for light terminals. Toggle syntax highlighting on/off at any time with `Ctrl-T`.

To use a specific theme, set it in your config:

```toml
theme = "DarkNeon"
```

> **Note**: macOS's built-in Terminal.app does not render ANSI colors correctly. A third-party terminal emulator is recommended for best results.

**Available themes:**

| Theme | Style |
|-------|-------|
| `OneHalfDark` | Clean modern dark (default for dark terminals) |
| `OneHalfLight` | Clean modern light (default for light terminals) |
| `Dracula` | High-contrast dark, purple/pink accents |
| `Nord` | Arctic-inspired dark |
| `Monokai Extended` | Classic Monokai, updated |
| `Monokai Extended Bright` | Higher-contrast Monokai variant |
| `Monokai Extended Light` | Light-background Monokai |
| `Monokai Extended Origin` | Original unaltered Monokai |
| `Visual Studio Dark+` | VS Code default dark |
| `GitHub` | Light, mimics GitHub's code view |
| `Solarized (dark)` / `Solarized (light)` | Classic low-contrast Solarized |
| `gruvbox-dark` / `gruvbox-light` | Warm, earthy retro tones |
| `Coldark-Cold` | Blue-tinted light |
| `Coldark-Dark` | Cool-blue dark |
| `DarkNeon` | Vibrant dark with neon accents |
| `Sublime Snazzy` | Bright, elegant dark |
| `TwoDark` | Atom One Dark with slightly better contrast |
| `1337` | High-contrast dark |
| `zenburn` | Low-contrast, easy on the eyes |
| `base16` / `base16-256` | Standard base16 (256-color variant available) |
| `ansi` | Uses your terminal's 16 built-in ANSI colors |

## Formatter

`Ctrl-L` pipes the current buffer to an external formatter in a background thread. The formatted result is applied only if the buffer hasn't changed during formatting — keystrokes made while a slow format runs are not discarded. Dan detects the right formatter based on file type:

- **Rust**: [rustfmt](https://github.com/rust-lang/rustfmt) — `rustup component add rustfmt`
- **Python**: [ruff](https://docs.astral.sh/ruff/) — `pip install ruff`
- **JS / TS / JSON / CSS / HTML**: [prettier](https://prettier.io/) — `npm i -g prettier`

Formatter output and errors are shown in the status bar.

## Note on AI use

I've been writing code for over 30 years. Lately, LLM agent-enhanced coding practices have rekindled my sense of awe at what's possible. This project has been built using a range of tools, including Anthropic's Claude Code (with Opus 4.7).

Unlike some who dismiss anything touched by a coding agent as "slop," I don't see it that way. To me, these tools are a way to move much faster, explore many more ideas, and test those ideas and implementations more rigorously than I ever could on my own.

---

**License**: GNU General Public License v3.0 (GPLv3)

# dan

A fast, modeless, lightweight terminal text editor written in Rust. Designed for zero-configuration deployment, dan maintains ultra-low input latency even over high-jitter SSH connections and delivers high-performance operations on massive files thanks to its Rope buffer architecture.

<p align="center">
  <img width="800" alt="screenshot of dan" src="https://github.com/user-attachments/assets/d84984c2-ed8e-4df5-8161-77a235c65a6e" />
</p>

### Key performance metrics:
- **Memory footprint**: Typically consumes < 20MB RSS.
- **File handling capacity**: Fluid, non-blocking navigation and manipulation of 100MB+ log files.
- **Bandwidth optimization**: Implements aggressive rendering optimizations to minimize transmitted escape sequences.

### Architectural comparison

| Feature | Dan | Vim | Nano | Micro |
| --- | --- | --- | --- | --- |
| Modeless | ✅ | ❌ | ✅ | ✅ |
| Rust-based | ✅ | ❌ | ❌ | ✅ |
| Atomic Saves | ✅ (fsync/rename) | ⚠️ (Configurable) | ❌ | ❌ |
| Buffer Architecture | Rope $O(\log N)$ | Gap buffer/Piece table | Flat string | Gap buffer |
| Rendering | Differential | Full/partial redraw | Full redraw | Full redraw |
| Crash Recovery | ✅ Auto-swap | ✅ Swap files | ❌ | ❌ |
| Command Palette | ✅ | ❌ (Cmd line) | ❌ | ❌ |
| Out-of-box Config | Zero-config | High learning curve | Minimal | Minimal |


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

- **Rope-Backed Text Buffer**: Utilizes a rope structure ensuring $O(\\log N)$ time complexity for insertions and deletions. Memory usage scales with edit volume rather than raw file size, permitting fluid, non-blocking navigation and manipulation of 100MB+ log files.
- **Optimized Terminal I/O & Differential Rendering**: Implements differential rendering to minimize bandwidth by emitting ANSI escape sequences strictly for modified cells. To sustain $O(1)$ scroll performance in massive files, `dan` maintains a syntax snapshot cache every 200 lines, eliminating the need to re-lex the entire visible range during rapid vertical movement.
- **POSIX-Compliant Atomic Writes (Crash-Safe I/O)**: File writes are executed via a temporary sibling file, followed by an `fsync` and atomic `rename`. A system crash or disk-full condition mid-save leaves the original file intact, preserving original file permissions and symlink targets.
- **Crash Recovery**: Periodically checkpoints the active buffer to a hidden `.swp` file every 5 seconds using safe write patterns. Unplanned terminal disconnects or crashed sessions trigger automatic recovery prompts on the next open.
- **Interactive Command Palette (`Ctrl-P`)**: A fuzzy-search overlay covering all editor actions, active buffers, and project workspace files to keep operations entirely on the home row.
- **Multiple Buffers**: Concurrent support for multiple active buffers, indexed and managed via the command palette.
- **Context-Aware Syntax Highlighting**: Powered by `syntect` with broad language grammar support. Queries the terminal background at startup to apply adaptive themes (OneHalfDark/OneHalfLight) automatically, with immediate toggling via `Ctrl-T`.
- **Background Auto-Formatter (`Ctrl-L`)**: Pipes buffer contents to external formatters (Prettier, Rustfmt, Ruff) on a background thread. Formatted output is applied transactionally only if the buffer was not modified during execution.
- **Fuzzy Search & Destructive Replace**: Instant buffer-wide searching with `Ctrl-F`, easily promoted to standard find-and-replace using `Ctrl-R`.
- **Unicode & CJK Support**: Correct visual alignment, cell measurements, and cursor positioning for double-width characters and complex emoji.
- **Native Clipboard Integration**: Cross-platform clipboard access using `arboard`, falling back gracefully to an internal in-memory buffer on headless SSH sessions without display servers.
- **Auto-Pairs & Wrap-on-Type**: Automated closure insertion for brackets and quotes, with contextual wrap behavior when keys are typed over an active selection.
- **Robust Encoding Detection**: Scans and parses legacy encodings (Shift-JIS, Windows-1252, etc.) utilizing Byte Order Mark (BOM) sniffing, normalizes to UTF-8 internally, and transparently round-trips to the native encoding on save.
- **Active Content Sanitization**: Sanitizes raw terminal escape sequences at render time. Malicious or hostile files containing raw ANSI codes cannot alter terminal chrome or exfiltrate local clipboard states.
- **Hierarchical Configuration**: Evaluates settings through a layered model: core defaults → `~/.config/dan/config.toml` → local workspace `.editorconfig` rules.


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
<img width="2180" height="1806" alt="CleanShot 2026-06-04 at 15 12 03@2x" src="https://github.com/user-attachments/assets/3bf61843-f315-4cce-8bdc-c4bea84352c2" />
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

I've been writing code for over 30 years. Lately, LLM agent-enhanced coding practices have rekindled my sense of awe at what's possible. This project has been built using a range of tools. By leveraging advanced LLMs for boilerplate generation, rapid prototyping, and automated unit testing, development efforts were focused on high-level architectural decisions, robust edge-case verification, and low-level performance optimizations.

---

**License**: GNU General Public License v3.0 (GPLv3)

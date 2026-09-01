# dan

A fast, modeless terminal text editor written in Rust. It's a lighter,
quicker alternative to nano, pico, vim, joe, and micro.

Dan needs no configuration, just build it and start editing using familiar keyboard shortcuts.
It keeps input latency low even over high-jitter SSH links, and its
rope-based buffer keeps editing responsive on files far past the point
where most editors stall. Try it with 100 MB+ logs, it opens and scrolls without hesitation.

<p align="center">
  <img width="800" alt="dan terminal editor" src="https://github.com/user-attachments/assets/ccebe66e-b927-418b-9cf1-4140771d3826" />
</p>

### Standout features:
- **Modeless**: no insert/normal split, no modal muscle memory to learn
- **Zero-config**: sensible and (lightly) opinionated defaults out of the box, with ample configuration options if you want to
- **Low latency**: designed to work equally well on remote sessions as in local terminals
- **Large files**: uses a rope buffer, so file size doesn't dictate speed
- **Multiple buffers**: supports multiple buffers (files), fast buffer switching 
- **Multi-platform**: Linux, macOS, BSD, Windows

### Key performance metrics:
- **Memory footprint**: Typically consumes < 20MB RSS
- **File handling capacity**: Fluid, non-blocking navigation and manipulation of 100MB+ log files
- **Bandwidth optimization**: Implements aggressive rendering optimizations to minimize transmitted escape sequences

### Architectural comparison

| Feature | Dan | Vim | Nano | Micro |
| --- | --- | --- | --- | --- |
| Modeless | ✅ | ❌ | ✅ | ✅ |
| Rust-based | ✅ | ❌ | ❌ | ✅ |
| Atomic saves | ✅ (fsync/rename) | ⚠️ (Configurable) | ❌ | ❌ |
| Buffer architecture | Rope $O(\log N)$ | Gap buffer/Piece table | Flat string | Gap buffer |
| Rendering | Differential | Full/partial redraw | Full redraw | Full redraw |
| Crash recovery | ✅ Auto-swap | ✅ Swap files | ❌ | ❌ |
| Command palette | ✅ | ❌ (Cmd line) | ❌ | ❌ |
| Out-of-box config | Zero-config | High learning curve | Minimal | Minimal |


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

Dan uses familiar shortcuts out of the box — `Ctrl-C`/`V` to copy/paste, `Ctrl-S` to save, `Ctrl-Z`/`Y` to undo/redo, `Ctrl-Q` to quit. Press `Ctrl-H` to toggle the built-in help bar at any time. Mouse is enabled by default: click to place the cursor, drag to select, and use the scroll wheel to move the viewport (set `mouse = false` to disable).

- **Rope-backed text buffer**: Utilizes a rope structure ensuring $O(\\log N)$ time complexity for insertions and deletions. Memory usage scales with edit volume rather than raw file size, permitting fluid, non-blocking navigation and manipulation of 100MB+ log files.
- **Optimized terminal I/O & differential rendering**: Implements differential rendering to minimize bandwidth by emitting ANSI escape sequences strictly for modified cells. To sustain $O(1)$ scroll performance in massive files, `dan` maintains a syntax snapshot cache every 200 lines, eliminating the need to re-lex the entire visible range during rapid vertical movement.
- **POSIX-compliant atomic writes (crash-safe I/O)**: File writes are executed via a temporary sibling file, followed by an `fsync` and atomic `rename`. A system crash or disk-full condition mid-save leaves the original file intact, preserving original file permissions and symlink targets.
- **Crash recovery**: Periodically checkpoints the active buffer to a hidden `.swp` file every 5 seconds using safe write patterns. Unplanned terminal disconnects or crashed sessions trigger automatic recovery prompts on the next open.
- **Interactive command palette (`Ctrl-P`)**: A fuzzy-search overlay covering all editor actions, active buffers, and project workspace files to keep operations entirely on the home row.
- **Multiple buffers**: Concurrent support for multiple active buffers. `Ctrl-N` opens a new buffer; switching, closing, and saving buffers is handled through the command palette. Quitting with unsaved changes steps through each dirty buffer in turn.
- **Context-aware syntax highlighting**: Powered by `syntect` with broad language grammar support. Auto-picks OneHalfDark/OneHalfLight from `COLORFGBG` or an OSC colour query when `theme = "default"`, with immediate toggling via `Ctrl-T`.
- **Background auto-formatter (`Ctrl-L`)**: Pipes buffer contents to external formatters (Prettier, Rustfmt, Ruff) on a background thread. Formatted output is applied transactionally only if the buffer was not modified during execution.
- **Fuzzy search & destructive replace**: Instant buffer-wide searching with `Ctrl-F`, easily promoted to find-and-replace with `Ctrl-R`. Wrap the query in `/pattern/` for regex (case-sensitive; use `(?i)` for insensitive). Regex replace supports `$0`, `$1`, `$name`, and `$$`.
- **Unicode & CJK support**: Correct visual alignment, cell measurements, and cursor positioning for double-width characters and complex emoji.
- **Soft-wrap navigation**: With wrap on, Up/Down/Page/Home/End and scrolling move by *visual* rows (sticky goal column, word-boundary wraps). `Ctrl+Alt+Home/End` jump the logical line; optional `breakindent` indents continuation rows.
- **Native clipboard integration**: Cross-platform clipboard access using `arboard`, falling back gracefully to an internal in-memory buffer on headless SSH sessions without display servers.
- **Auto-pairs & wrap-on-type**: Automated closure insertion for brackets and quotes, with contextual wrap behavior when keys are typed over an active selection.
- **Robust encoding detection**: Scans and parses legacy encodings (Shift-JIS, Windows-1252, etc.) utilizing Byte Order Mark (BOM) sniffing, normalizes to UTF-8 internally, and transparently round-trips to the native encoding on save.
- **Active content sanitization**: Sanitizes raw terminal escape sequences at render time. Malicious or hostile files containing raw ANSI codes cannot alter terminal chrome or exfiltrate local clipboard states.
- **Hierarchical configuration**: Evaluates settings through a layered model: core defaults → `~/.config/dan/config.toml` → local workspace `.editorconfig` rules.


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
| `Ctrl` + `N` | New buffer |

### Command palette (`Ctrl-P`)

The palette is a fuzzy-search overlay: start typing to filter across editor actions, open buffers, and project files, then `Enter` to run or switch. Every keyboard shortcut is also available here, plus a number of actions that have no dedicated key:

- **Buffers & files**: Open file, reload buffer from disk, close buffer / close others / close all, save all, show recent files. `Ctrl-D` on a highlighted buffer closes it directly (with a save prompt if it has unsaved changes).
- **Path utilities**: Copy the file's absolute or relative path, reveal in Finder / open containing folder, show buffer info.
- **Per-buffer format settings**: Switch indentation between spaces and tabs, set tab width (2/4/8), switch line endings between LF and CRLF, trim trailing whitespace, convert existing indentation tabs ↔ spaces.
- **Text transforms**: Sort lines ascending/descending, deduplicate adjacent lines, convert to UPPERCASE / lowercase / Title Case, reverse the selection.
- **Misc**: Toggle line numbers, reload configuration, show version, show keybindings.

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
| `Home` / `End` | Start / end of current visual row (soft-wrap aware) |
| `Ctrl` + `Alt` + `Home` / `End` | Start / end of logical line |
| `Ctrl` + `↑` / `↓` | Scroll without moving cursor |
| `Ctrl` + `Shift` + `↑` / `↓` | Fast scroll |
| `Ctrl` / `Alt` + `←` / `→` | Jump by word |
| `Ctrl` + `Home` / `End` | Jump to start / end of file |
| `Ctrl` + `G` | Go to line |

### Search & replace

| Key | Action |
|-----|--------|
| `Ctrl` + `F` (or `F7`) | Open search |
| `Ctrl` + `G` | Next match *(while searching)* |
| `Ctrl` + `T` | Previous match *(while searching)* |
| `Enter` | Select the current match and leave search |
| `Esc` | Cancel search and restore the cursor |
| `Ctrl` + `R` *(while searching)* | Promote to find-and-replace |
| `Ctrl` + `Y` / `N` / `A` *(step replace)* | Replace this match / skip / replace all remaining |

Search is incremental: matches update as you type. The prompt shows `N/M matches` when there are hits. Without surrounding slashes, search is **literal** and **case-insensitive**.

#### Regex search (`/pattern/`)

Wrap the query in forward slashes to switch from literal search to a regular expression:

```
/pattern/
```

Dan uses the Rust [`regex`](https://docs.rs/regex/) crate (finite automata; no lookaround or backreferences). There is no separate “regex mode” key — the slashes are the switch.

**When a query counts as regex**

| Query | Mode | Notes |
|-------|------|-------|
| `foo` | Literal | Case-insensitive substring |
| `/foo/` | Regex | Pattern is `foo` |
| `/\w+_id/` | Regex | Word characters before `_id` |
| `/(?i)todo/` | Regex | Case-insensitive via inline flag |
| `/foo` | Literal | Missing closing `/` |
| `foo/` | Literal | Missing opening `/` |
| `//` | Literal | Empty interior — not treated as regex |

Rules: the query must start with `/`, end with `/`, and have a non-empty interior. Alternation and other Rust regex syntax work inside the slashes (e.g. `/error|warn/`). Trailing flags like `/pattern/i` are **not** supported; put flags inside the pattern instead (see below).

**Case sensitivity**

| Mode | Default | Override |
|------|---------|----------|
| Literal | Case-insensitive | — |
| Regex | Case-sensitive | `(?i)` for insensitive, `(?-i)` to force sensitive again |

Other useful inline flags (Rust `regex` syntax):

| Flag | Effect |
|------|--------|
| `(?i)` | Case-insensitive |
| `(?m)` | `^` / `$` match line boundaries |
| `(?s)` | `.` matches newlines |

Example: `/(?im)^\s*todo:/` finds `todo:` at the start of a line, ignoring case.

**Invalid patterns**

While you type, incomplete or illegal patterns (e.g. `/foo(/`) clear all highlights and show `invalid regex` in the search bar. As soon as the pattern compiles again, matches return. Promote-to-replace (`Ctrl+R`) only works when there is at least one match, so an invalid pattern cannot enter replace.

**Searching for literal `/…/` text**

There is no special escape for “literal slash-wrapped text.” To find the characters `/foo/`, use a regex and escape the slashes, for example:

```
/\/foo\//
```

**Regex replace (capture groups)**

With a regex search that has matches, press `Ctrl+R`, type a replacement, then `Enter` to step through matches (`^Y` yes, `^N` skip, `^A` all remaining).

In regex sessions the replacement string supports Rust-style expansions:

| Token | Meaning |
|-------|---------|
| `$0` | Entire match |
| `$1`, `$2`, … | Numbered capture groups |
| `$name` or `${name}` | Named group (`(?P<name>…)` or `(?<name>…)`) |
| `$$` | A literal `$` |

Examples:

| Search | Replace with | On text `foo_bar` |
|--------|--------------|-------------------|
| `/(foo)_(bar)/` | `$2-$1` | `bar-foo` |
| `/(?P<w>\w+)/` | `[$w]` | `[foo_bar]` (one match) |
| `/a(\d)/` | `X$1` | `a1` → `X1` |

Literal (non-`/…/`) search never expands `$` — a replacement of `$1` inserts the characters `$1`.

Missing groups expand to an empty string (same as the `regex` crate). Each match is expanded independently; replace-all applies from the current match onward.

**Limits (v1)**

- No trailing `/flags` after the closing slash — use `(?i)`, `(?m)`, `(?s)` inside the pattern.
- No lookaround or backreferences (`fancy-regex` features are not enabled).
- Regex search materializes the buffer once per keystroke; huge files may feel heavier than literal search.
- Zero-width matches are skipped so next/replace cannot loop forever.

**Note for macOS users**: Terminal emulators use escape sequences dating back to the late 70s and some at the time highly influential video display terminals such as VT100. Long story short, this means some "modern" key combinations available in GUI editors can't be distinguished in a terminal. Most notably, Dan (and other terminal apps) uses `Ctrl` where a Mac user might expect `⌘`. Many terminal emulators (including [iTerm2](https://iterm2.com/)) let you remap `⌘` to `Ctrl` if you prefer, although it can create side-issues. Additionally, the built-in Terminal.app is not recommended: a third-party emulator such as [iTerm2](https://iterm2.com/), [Kitty](https://sw.kovidgoyal.net/kitty/), [Ghostty](https://ghostty.dev/), or [WezTerm](https://wez.dev/) will give better results.

# Installation

Dan requires Rust 1.88 or later. We recommend installing via [rustup](https://rustup.rs/) rather than your system package manager, which often provides an older version:

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

```toml
# Display
wrap_lines = true           # Wrap long lines (default: true)
breakindent = false         # Indent soft-wrap continuations to match leading indent
tab_width = 4               # Visual tab width (default: 4)
expand_tab = false          # Insert spaces instead of tabs (default: false)
line_numbers = true         # Show line numbers (default: true)
highlight_active = true     # Highlight the current line (default: true)
scroll_off = 5              # Lines to keep visible above/below cursor (default: 5)
fast_scroll_steps = 10      # Lines jumped per fast-scroll keypress (default: 10)
show_full_path = false      # Show full file path in toolbar (default: false)
show_whitespace = false     # Show visible markers for spaces/tabs/EOL (default: false; toggle with Ctrl-R)
cursor_style = "block"      # "block" | "line" | "underscore" (default: "block")
cursor_blink = false        # Blink the terminal cursor (default: false)
# cursor_color = "#FF8800"  # Optional; omit to leave the terminal cursor color alone

# Editing
auto_indent = true          # Match indentation of the previous line (default: true)
auto_close = true           # Auto-insert closing brackets and quotes (default: true)
syntax_highlight = true     # Enable syntax highlighting (default: true)

# Interface
show_help = true            # Show shortcut bar at the bottom (default: true)
show_encoding = true        # Show file encoding in status bar (default: true)
show_lang = true            # Show detected language in status bar (default: true)
mouse = true                # Click, drag-select, wheel scroll (default: true)

# Theme
theme = "default"           # "default" = COLORFGBG then OSC auto-detect; or a syntect theme name
comments_are_italics = true # Render comments in italics (default: true)
```

### Project-aware settings

Dan automatically picks up `.editorconfig` files in the project tree. Tab width, line endings, and trailing-whitespace rules defined there take precedence over your global config, so Dan adapts to each project's style without manual adjustment.

## Cursor

The terminal cursor (document, prompts, and command palette) is configured with three keys:

| Key | Values | Default |
|-----|--------|---------|
| `cursor_style` | `"block"`, `"line"`, `"underscore"` | `"block"` |
| `cursor_blink` | `true` / `false` | `false` |
| `cursor_color` | `#RGB` or `#RRGGBB` (optional) | unset |

When `cursor_color` is omitted, Dan leaves your terminal's cursor color alone. When set, Dan applies it via OSC 12 at startup and restores the previous color on exit. Most modern emulators honor this; some (including older Terminal.app builds) may ignore it.

Example — blinking orange bar:

```toml
cursor_style = "line"
cursor_blink = true
cursor_color = "#FF8800"
```

Save the file with normal Unix newlines (`\n`). Unusual line endings can make the whole config fail to parse; Dan then falls back to defaults and prints a warning.

## Themes

When `theme = "default"`, Dan picks `OneHalfDark` or `OneHalfLight` from your
terminal background:

1. **`COLORFGBG`** environment variable (no terminal I/O), if set and valid
2. Otherwise an **OSC 10/11** colour query (via `terminal-colorsaurus`)
3. Otherwise **dark** (`OneHalfDark`)

If you set an explicit theme name (e.g. `theme = "DarkNeon"`), Dan skips the
OSC query. Chrome colours still follow `COLORFGBG` when present, else dark.

Toggle syntax highlighting on/off at any time with `Ctrl-T`.

To force a light or dark syntax theme without auto-detect:

```toml
theme = "OneHalfLight"
# or
theme = "OneHalfDark"
```

To use a different specific theme:

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

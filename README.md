# dan
**A fast, modern, and intuitive terminal text editor: light, quick, and intuitive, yet very capable** 

Dan is a modeless terminal editor that, despite being [highly configurable](#configuration), needs no dotfile to get started. Just [install Dan](#installation) and start editing: `dan ~/my-file.txt`. Dan uses [familiar keyboard shortcuts](#keyboard-shortcuts): `Ctrl-S` to save, `Ctrl-Q` to quit, `Ctrl-F` to search. Hit `Ctrl-H` for an inline help bar and `Ctrl-P` for a command palette from which [every feature](#features) in Dan is readily available. 

<p align="center">
    <img width="800" alt="Dan" src="https://github.com/user-attachments/assets/a872bf6d-98a2-46c6-b18d-837a5d355c8d" />
</p>

Dan ships with sensible defaults intended to suit most users and use-cases out of the box. It keeps input latency low over SSH links, employs advanced render optimizations, and its modern rope-based buffer 
keeps editing responsive on files far past the point where most editors stall. Try it with 100 MB+ logs, it opens and scrolls without hesitation. Dan is written entirely in Rust for safety, reliability, and performance, and runs on Linux, macOS, BSD, and Windows.

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
| Edits made on disk while open | ✅ Live 3-way merge | ⚠️ Reload/warn | ⚠️ Warns on save | ⚠️ Reload prompt |
| Out-of-box config | Zero-config | High learning curve | Minimal | Minimal |


## Quick install

On macOS or Linux, install with [Homebrew](https://brew.sh/):
```
brew install dfallman/tap/dan
```

Or grab a prebuilt binary for macOS, Linux, or Windows from the
[latest release](https://github.com/dfallman/dan/releases/latest).
See [Installation](#installation) for where to put it on each platform.

...or, build from source (it's easy!) with [Rust](https://rustup.rs/):
```
git clone https://github.com/dfallman/dan.git
cd dan
cargo install --path .
```

## Features

<table>
<tr>
<td width="50%" valign="top">

**Familiar from the first keystroke**<br>
Modeless editing with the shortcuts you already know: `Ctrl-S`, `Ctrl-Z`, and `Ctrl-C`/`V`. The mouse works too, and `Ctrl-H` toggles a help bar.

</td>
<td width="50%" valign="top">

**One palette for everything**<br>
`Ctrl-P` fuzzy-searches every action, open buffer, and project file. Open as many buffers as you like with `Ctrl-N`. [More →](#command-palette-ctrl-p)

</td>
</tr>
<tr>
<td valign="top">

**Works alongside coding agents**<br>
When an agent, a formatter, or `git checkout` changes an open file, Dan merges it with your unsaved edits and highlights what changed. [More →](#files-changed-on-disk)

</td>
<td valign="top">

**Hard to lose work**<br>
Saves are atomic, and a swap file every 5 seconds means a crash or dropped SSH session gets offered back to you on the next open.

</td>
</tr>
<tr>
<td valign="top">

**Fast on huge files**<br>
100 MB+ logs open and scroll without stalling, in under 20 MB of memory. Minimal redraws keep it snappy over slow SSH links.

</td>
<td valign="top">

**Find and replace**<br>
Incremental search as you type, `/regex/` when you need it, and replacements with capture groups. [More →](#search--replace)

</td>
</tr>
<tr>
<td valign="top">

**Any text, any terminal**<br>
Syntax highlighting that follows your terminal's light or dark theme, Unicode and CJK, legacy encodings, and soft-wrap that moves by visual row.

</td>
<td valign="top">

**Fits your project**<br>
Zero config to start, `config.toml` when you want it, and `.editorconfig` respected. `Ctrl-L` formats with rustfmt, ruff, or prettier.

</td>
</tr>
</table>

<details>
<summary><b>Under the hood</b></summary>

- **Rope buffer**: $O(\log N)$ inserts and deletes; memory scales with edit volume, not file size.
- **Differential rendering**: only changed cells are redrawn, and syntax state is cached every 200 lines so fast scrolling never re-lexes the whole view.
- **Atomic writes**: temp sibling file, then `fsync`, then `rename`. A crash or full disk mid-save leaves the original intact, with permissions and symlink targets preserved.
- **Three-way line merge** for on-disk changes: you're asked only when both sides touched the same lines, each update is one undo step, and saves re-check the disk first.
- **Escape-sequence sanitization**: files containing raw ANSI codes can't alter the terminal or reach your clipboard.
- **Encoding detection**: BOM sniffing for Shift-JIS, Windows-1252, and others; UTF-8 internally, written back in the original encoding.
- **Clipboard**: native via `arboard`, falling back to an internal buffer on headless SSH sessions.
- **Background formatter**: output is applied only if the buffer didn't change while it ran.
- **Layered config**: built-in defaults, then `~/.config/dan/config.toml`, then `.editorconfig`.

</details>


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
The command palette is a fuzzy-search overlay that has every feature of Dan, every open file buffer, and all openable files in your current directory all there, ready at hand: start typing to filter across editor actions, open buffers, and file operations, then `Enter` to run or switch. 

<p align="center">
    <img width="600" alt="Dan command palette" src="https://github.com/user-attachments/assets/2f7d6d6a-56f9-4e41-9759-b7b8ad3fef41" />
</p>

The mouse works too: click a result to run it, scroll the wheel to move through the list, and click outside the palette to dismiss it. Every keyboard shortcut is also available here, plus a number of actions that have no dedicated key:

- **Buffers & files**: Open file, reload buffer from disk, clear change marks, close buffer / close others / close all, save all, show recent files. `Ctrl-D` on a highlighted buffer closes it directly (with a save prompt if it has unsaved changes).
- **Path utilities**: Copy the file's absolute or relative path, reveal in Finder / open containing folder, show buffer info.
- **Per-buffer format settings**: Switch indentation between spaces and tabs, set tab width (2/4/8), switch line endings between LF and CRLF, trim trailing whitespace, convert existing indentation tabs ↔ spaces.
- **Text transforms**: Sort lines ascending/descending, deduplicate adjacent lines, convert to UPPERCASE / lowercase / Title Case, reverse the selection.
- **Misc**: Toggle line numbers, reload configuration, show version, show keybindings.

**Note for macOS users**: Terminal emulators use escape sequences dating back to the late 70s and some at the time highly influential video display terminals such as VT100. Long story short, this means some "modern" key combinations available in GUI editors can't be distinguished in a terminal. Most notably, Dan (and other terminal apps) uses `Ctrl` where a Mac user might expect `⌘`. Many terminal emulators (including [iTerm2](https://iterm2.com/)) let you remap `⌘` to `Ctrl` if you prefer, although it can create side-issues. Additionally, the built-in Terminal.app is not recommended: a third-party emulator such as [iTerm2](https://iterm2.com/), [Kitty](https://sw.kovidgoyal.net/kitty/), [Ghostty](https://ghostty.dev/), or [WezTerm](https://wez.dev/) will give better results.

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

**A few known limitations**
- No trailing `/flags` after the closing slash — use `(?i)`, `(?m)`, `(?s)` inside the pattern.
- No lookaround or backreferences (`fancy-regex` features are not enabled).
- Regex search materializes the buffer once per keystroke; huge files may feel heavier than literal search.
- Zero-width matches are skipped so next/replace cannot loop forever.


# Installation

## Option 1: Homebrew (macOS & Linux)

```
brew install dfallman/tap/dan
```

Homebrew puts `dan` on your `PATH` and handles macOS quarantine for you; update later with `brew upgrade dan`. The [tap](https://github.com/dfallman/homebrew-tap) is refreshed automatically on every release.

## Option 2: Download a prebuilt binary

Every release ships ready-to-run binaries on the
[releases page](https://github.com/dfallman/dan/releases/latest). Pick the archive for your platform:

| Platform | Archive |
|---|---|
| macOS (Apple Silicon) | `dan-<version>-aarch64-apple-darwin.tar.gz` |
| macOS (Intel) | `dan-<version>-x86_64-apple-darwin.tar.gz` |
| Linux (x86_64) | `dan-<version>-x86_64-unknown-linux-gnu.tar.gz` |
| Linux (arm64) | `dan-<version>-aarch64-unknown-linux-gnu.tar.gz` |
| Windows (x86_64) | `dan-<version>-x86_64-pc-windows-msvc.zip` |

Each archive contains the `dan` binary plus the README and license.

### macOS

Extract and move the binary somewhere on your `PATH` (`/usr/local/bin` is in the default `PATH`):

```
tar xzf dan-*-apple-darwin.tar.gz
sudo mv dan-*-apple-darwin/dan /usr/local/bin/
```

If macOS refuses to run it (with a message such as "...cannot be opened because the developer cannot be verified", this can happen for browser downloads as the binaries are not notarized), you can clear the quarantine flag with:

```
xattr -d com.apple.quarantine /usr/local/bin/dan
```

If you'd rather not use `sudo`, put it in `~/bin` or `~/.local/bin` instead and add that directory to `PATH` in your shell profile: `export PATH="$HOME/.local/bin:$PATH"`.

**Note**: on macOS, it's easier to install using Homebrew: `brew install dfallman/tap/dan` or building Dan from source, see below.

### Linux

```
tar xzf dan-*-linux-gnu.tar.gz
install -Dm755 dan-*-linux-gnu/dan ~/.local/bin/dan
```

`~/.local/bin` is on the default `PATH` of most modern distributions; if `dan` isn't found afterwards, add `export PATH="$HOME/.local/bin:$PATH"` to your shell profile, or use `sudo install -m755 .../dan /usr/local/bin/dan` for a system-wide install.

### Windows

> **Note**: If you're running Dan inside WSL, follow the Linux instructions above instead.

Unzip the archive and put `dan.exe` in a folder of your choice, e.g. `%LOCALAPPDATA%\Programs\dan`. Then add that folder to your `PATH` so you can run `dan` from any terminal:

```powershell
Expand-Archive dan-*-windows-msvc.zip
New-Item -ItemType Directory -Force "$env:LOCALAPPDATA\Programs\dan"
Move-Item dan-*-windows-msvc\dan-*\dan.exe "$env:LOCALAPPDATA\Programs\dan\"
# add to PATH for the current user (takes effect in new terminals)
[Environment]::SetEnvironmentVariable("Path", $env:LOCALAPPDATA + "\Programs\dan;" + [Environment]::GetEnvironmentVariable("Path", "User"), "User")
```

SmartScreen may warn the first time you run a downloaded, unsigned executable — choose "More info" → "Run anyway".

## Option 3: Build from source

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
scrollbar = "scrolling"     # "none" | "always" | "scrolling" = show while scrolling or hovering the right column (default: "scrolling")
watch_files = true          # Update open files changed on disk by other programs (default: true)
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

## Scrollbar

A vertical scrollbar can be drawn in the rightmost column of the text area:

| `scrollbar` | Behaviour |
|-------------|-----------|
| `"none"` | No scrollbar |
| `"always"` | Always visible |
| `"scrolling"` | Appears while the viewport moves or the mouse pointer rests on the scrollbar column, and fades out about two seconds after that stops (default) |

The thumb is drawn as `█` on a `│` track. With the mouse enabled, click the track to jump there or drag the thumb to scroll; in `"scrolling"` mode, hovering over the rightmost column reveals the bar so you can grab it. Enabling the scrollbar reserves one column, so soft-wrapped text is one character narrower; in `"scrolling"` mode the column stays reserved while the bar is hidden so text never reflows when it appears.

## Files changed on disk

When another program (a coding agent, a formatter, `git checkout`) changes a
file you have open, Dan updates the buffer within about half a second:

- **No unsaved edits:** the new version is loaded in place; your cursor and
  scroll position stay put.
- **Unsaved edits elsewhere in the file:** both are kept.
- **Unsaved edits in the same lines:** Dan asks — `^K` keep mine, `^T` take
  theirs, `Esc` decide later (keeps yours and marks the lines red).

Changed line numbers are highlighted yellow (conflicts you deferred are red)
until you save, or until you run **Clear change marks** from the command
palette. The marks follow your edits, and each update is one undo step, so
`^Z` backs it out.

A few more details:

- **Every open buffer is watched**, not just the visible one. A conflict in a
  background buffer waits and is shown when you switch to it.
- **Saving checks the disk first.** If the file changed since Dan last looked,
  the change is merged in and you save again, so a save never overwrites an
  edit you haven't seen. The same goes for *Save all* and saving on quit.
- **Deleted or moved files** are reported once, and the buffer is kept and
  marked as unsaved; saving recreates the file.
- **Detection is cross-platform polling** of the file's modification time
  and size (plus its inode on Unix), so it works the same on macOS, Linux,
  Windows, WSL, and network drives, including tools that save through a
  temporary file and rename.

Set `watch_files = false` to turn this off.

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

## How it's made
Dan is written in Rust, with help from tools like Anthropic's Claude Code. I've been writing code for over 30 years, and working with coding agents has rekindled my sense of awe at what code can do. They let me move faster, try more ideas, and test them more thoroughly than I would on my own.

---

**License**: GNU General Public License v3.0 (GPLv3)

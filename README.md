# dan

**Dan** is a friendly, lightning-fast, modeless, and zero-latency terminal text editor written natively in Rust.

Creating Dan, the goal was to forge a truly fast, no-fuss text editor for the terminal environment that works exactly the way you expect a modern GUI editor to work—yet packed inside an architecture capable of running over fluctuating SSH connections without dropping a single frame.

No strange modes to learn, no archaic keyboard shortcuts, and no unnecessarily long configuration dot files. Out of the box, Dan ships with an intelligent Differential Rendering Matrix, pure O(1) immutable tree histories, continuous background I/O fault-tolerance, and full Unicode/CJK character support. It comes with a sensible set of defaults that should work for most people.

---

## 🚀 Core Features

### Performance & Architecture
- **Zero-Copy Rendering Matrix** — Dan operates entirely on an isolated `ScreenBuffer` double-buffering framework. Keystrokes are computed mathematically on a structural grid, and the `.diff()` engine ensures that **only modified cells** are broadcasted via ANSI escape codes. Scrolling a 100MB file over SSH consumes virtually 0 bytes of network payload compared to typical layout refresh blasting.

- **Micro-Timeout Event Debouncing** — Natively draining the `crossterm` execution queue inside a 5ms window ensures that rapid 30Hz keystroke barrages or unbracketed macro pastes are smoothly collapsed into a single, highly efficient 60FPS render loop tick preserving battery life organically.

- **O(1) Immutable Tree History** — The Undo/Redo tracking framework operates transparently using `ropey::Rope` snapshots, mapping shared structural pointer leaves. Dan utilizes zero `String` memory relocations or duplications during massive formatting cascades, maintaining an absolutely perfect constant memory footprint indefinitely.

- **Crash Recovery & Autosave System** — Dan seamlessly captures heavily modified files automatically via a silent 5-second asynchronous heartbeat. The structural `TextRope` payload is shifted directly into a background thread safely writing over `.swp` caches, guaranteeing the main UI execution loop **never freezes** regardless of disk I/O bottlenecks. 

### Built for Developers
- **Smart Line Commenting (`Ctrl+E`)** — Automatically detects the active syntax logic natively and toggles language-specific line comments (`//`, `#`, `--`, `<!--`) across multi-line selections instantly preserving internal indentation structures.

- **Asynchronous Code Clean Up (`Ctrl+L`)** — Press `Ctrl+L` to pipe code seamlessly through native system formatters (`rustfmt`, `prettier`, and `ruff`) using entirely non-blocking background threads. The results are mapped to a hyper-fast $O(N)$ text matrix diffing structure that automatically prevents active cursors from scrambling dynamically!

- **Auto-Closing Pairs** — Automatically injects paired brackets `({['"` seamlessly while typing. Execute wrapping loops natively by highlighting code boundaries and tapping a target wrapper!

- **Language Aware Syntax Highlighting** — Powered by `syntect` tracking runtime configurations tracking file extensions natively. Toggle dynamically at runtime safely using `Ctrl+T`.

### Advanced Editing
- **No Modes** — Start typing immediately, exactly like your GUI editor.

- **Rope-Backed Buffers** — B-Tree based geometry ensures $O(log n)$ insert/delete latency bounds mapping structural scaling cleanly.

- **Interactive Global Replace (`Ctrl+R`)** — Deploy sequential batch substitution strings structurally stepping interactively (`y`, `n`, `a`, `q`) within atomic Undo/Redo boundaries natively!

- **Multi-Line Indent/Dedent** — Highlight any structure and strike `Tab` or `Shift+Tab` to seamlessly shift spatial logic blocks cleanly!

### Internationalization & Environment
- **Dynamic Encoding Detection** — Automatically sniffs and cleanly buffers legacy byte formats (Shift-JIS, Windows-1252) into perfect UTF-8 formats using intelligent `chardetng` heuristic limits natively dynamically.

- **CJK & Unicode Bounds** — Perfect runtime tracking mapped to Japanese/Chinese/Korean character matrices cleanly maintaining dual-column and multi-offset visual-row geometry constraints locally natively!

- **Semantic Version Hooks** — Hardcoded to run `commit-msg` git hooks universally tracking and resolving `MINOR_VERSION_UPGRADE` bindings seamlessly updating `Cargo.toml` arrays locally securely automatically!

---

## 📥 Installation

You need [Rust 1.70+](https://rustup.rs/) installed to compile Dan.

### macOS & Linux
```bash
git clone https://github.com/dfallman/dan.git
cd dan
cargo build --release

# Move the binary into your local user path
cp target/release/dan ~/.local/bin/

# Alternatively, configure system-wide access:
sudo cp target/release/dan /usr/local/bin/
```

### Windows (PowerShell)
```powershell
git clone https://github.com/dfallman/dan.git
cd dan
cargo build --release

# Move the binary into a predefined directory mapped inside your Environment Variables (PATH).
# For standard standard local execution, moving it dynamically into Cargo's bin works flawlessly:
Copy-Item target\release\dan.exe ~/.cargo/bin/
```

### Quick Execution (All Platforms)
If you just want to launch Dan to test without structurally installing it into your `$PATH`:
```bash
cargo run --release -- myfile.rs
```

---

## ⌨️ Keybindings

Dan utilizes familiar GUI-style shortcuts strictly minimizing terminal mode confusion. Dan has been knowingly designed to be easy to use and learn. We've tried to rely on commonly used keyboard shortcuts whenever possible, while taking the limitations of the terminal environment into account. Hence, you can use 

On macOS, we recommend running Dan inside [iTerm2](https://iterm2.com/) or [Kitty](https://sw.kovidgoyal.net/kitty/) for the best experience, as they support the necessary escape codes for proper rendering. For some Mac users, especially if you're new to terminal apps or don't use them that often, using `Ctrl-S` instead of `Cmd-S` may take some time getting used to. It's worthwhile to consider remapping your left `Cmd` key (in iTerm2 and other terminal apps) to `Ctrl` so that you can use your standard shortcuts for things like saving, copying, pasting, etc.

On Windows, we recommend running Dan inside [Windows Terminal](https://www.microsoft.com/en-us/p/windows-terminal/9n0dx20hk701?activetab=pivot:overviewtab) for the best experience, as it supports the necessary escape codes for proper rendering. You can of course also use [Alacritty](https://alacritty.org/) or [WezTerm](https://wezterm.org/), or run Dan in WSL.


## Default Keyboard Shortcuts

**Dan** uses familiar, GUI-style keybindings so you can start editing immediately without memorizing a complex manual.

### Application & Navigation
| Key | Action |
| :--- | :--- |
| **Ctrl + H** | **Show Help**: Open the built-in command and shortcut reference. |
| **Ctrl + S** | **Save**: Write the current buffer to disk. |
| **Ctrl + Q** | **Quit**: Exit the editor (prompts to save if there are pending changes). |
| **Ctrl + Shift + Q** | **Force Quit**: Exit immediately and save a recovery snapshot. |
| **Ctrl + ↑ / ↓** | **Scroll**: Move the viewport up or down without moving the cursor. |
| **Ctrl + Shift + ↑ / ↓** | **Quick Scroll**: Fast viewport navigation. |

### Editing & Clipboard
| Key | Action |
| :--- | :--- |
| **Ctrl + C / X / V** | **Copy / Cut / Paste**: Standard system clipboard integration. |
| **Ctrl + Z / Y** | **Undo / Redo**: Step backward or forward through your edit history. |
| **Shift + Arrows** | **Select**: Highlight text for copying, cutting, or formatting. |
| **Ctrl + W** | **Word Wrap**: Toggle between soft-wrapped text and horizontal scrolling. |
| **Ctrl + K / D** | **Delete / Duplicate**: Quickly manage the current line. |
| **Alt + ↑ / ↓** | **Move Line**: Shift the current line or selection up or down. |

### Search & Productivity
| Key | Action |
| :--- | :--- |
| **Ctrl + F** | **Find**: Search for text within the current buffer. |
| **Ctrl + R** | **Replace**: Open the interactive search-and-replace module. |
| **Ctrl + G** | **Go-To Line**: Jump directly to a specific line number. |

### Development Tools
| Key | Action |
| :--- | :--- |
| **Ctrl + T** | **Syntax Highlighting**: Toggle language-specific colors on or off. |
| **Ctrl + L** | **Format/Lint**: Clean up the file using your configured external formatter. |
| **Ctrl + / (or Ctrl + E)**| **Comment**: Toggle comments for the current line or selection. |
| **Tab / Shift + Tab** | **Indent**: Increase or decrease indentation for the selection. |

---

## 🔧 Formatter configuration

Dan automatically matches your project’s style by reading `.editorconfig` files or falling back to your global settings. To enable professional code formatting with Ctrl+L, ensure your preferred language tools are installed:

- **Rust (`.rs`):** [`rustfmt`](https://github.com/rust-lang/rustfmt)
- **Python (`.py`):** [`ruff`](https://docs.astral.sh/ruff/) (`pip install ruff`)
- **Web (`.ts`, `.json`, `.css`):** [`prettier`](https://prettier.io/) (`npm i -g prettier`)

Dan is designed to fail safely. If a background subsystem or formatting target (such as the above) is unavailable, the editor intercepts the error to prevent a system panic, notifying you via a clean UI alert while maintaining the integrity of your session.

---

## ⚙️ Layered Configuration

Dan utilizes a comprehensive structured override environment to resolve visual settings organically:

1. **Hardcoded Internal Defaults** 
2. **Global Native TOML File** (`~/.config/dan/config.toml`)
3. **Local Project Schema** (`.editorconfig`)

Dan will automatically override internal configurations (like space vs tabs, indent arrays, line trailing overloads, etc.) by parsing your local `.editorconfig` files natively, bridging generic terminal parameters into strict local IDE matrices accurately.

### Global Config Parameters (`config.toml`)
```toml
# Wrap long lines (true) or scroll horizontally (false).
# Toggle at runtime with Ctrl+W.
wrap_lines = true

# Tab width in spaces (default: 4)
tab_width = 4

# Expand tabs to spaces on insert (default: false)
# true  = pressing Tab inserts spaces
# false = pressing Tab inserts a literal tab character
expand_tab = false

# Show line numbers in the gutter (default: true)
line_numbers = true

# Highlight the active (cursor) line (default: true)
highlight_active = true

# Scroll padding — lines to keep visible above/below cursor (default: 5)
scroll_off = 5

# Color theme (default: "default")
theme = "default"

# Fast scroll lines when jumping via Ctrl+Shift+Up/Down (default: 10)
fast_scroll_steps = 10

# Auto-close matched pairing structures (default: true)
auto_close = true

# Always show label "^H Help" in the toolbar
show_help = true

# Show active document character encoding in the toolbar (e.g. "utf-8")
show_encoding = true

# Show detected programming language in the toolbar (e.g. "Rust")
show_lang = true
```

---
**License**: MIT 

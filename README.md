# dan (Enterprise-Grade Terminal Editor)

**Dan** is a friendly, lightning-fast, modeless, and zero-latency terminal text editor written natively in Rust.

Creating Dan, the goal was to forge an ultra-fast, no-fuss text editor for the terminal environment that works exactly the way you expect a modern GUI editor to work—but packed inside an architecture mathematically capable of running over fluctuating 3G SSH connections without ever dropping a single frame.

No strange modes to learn, no archaic keyboard shortcuts, and no insanely long configuration schemas. Dan structurally ships with an intelligent Differential Rendering Matrix, pure O(1) immutable tree histories, continuous background I/O fault-tolerance, and full Unicode/CJK character support natively.

---

## 🚀 Core Features

### Enterprise Performance & Architecture
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

### From Source (Recommended)
You need [Rust 1.70+](https://rustup.rs/) installed to compile the zero-copy architectures.

```bash
git clone https://github.com/dfallman/dan.git
cd dan
cargo build --release
```

The compiled native payload will be located at `target/release/dan`. Softlink or move it sequentially inside your `$PATH`:
```bash
cp target/release/dan ~/.local/bin/
```

### Quick Execution
```bash
cargo run --release -- myfile.rs
```

---

## ⌨️ Keybindings

Dan utilizes familiar GUI-style shortcuts strictly minimizing terminal mode confusion.

### Navigation & Operations
| Key                    | Action                                          |
|------------------------|-------------------------------------------------|
| `Ctrl+S`               | Save File natively over Disk Bounds             |
| `Ctrl+Q`               | Safe Quit (Triggers Warning Frame on dirty hit) |
| `Ctrl+Shift+C`         | Force Quit (Bypasses Dirty Flags natively)      |
| `Ctrl+C` `Ctrl+X` `V`  | System Clipboard Interfacing                    |
| `Ctrl+Z`               | O(1) Snapshot History Undo                      |
| `Ctrl+Y`               | O(1) Snapshot History Redo                      |
| `Ctrl+H`               | Layout Overlay Toggle                           |
| `Ctrl+W`               | Toggle Soft Word-Wrapping Constraints           |

### Structure & Layouts
| Key                    | Action                                          |
|------------------------|-------------------------------------------------|
| `Ctrl+F`               | Launch Incremental Search Subsystem             |
| `Enter` / `Shift+Enter`| Navigate Forward / Backward across Regex Limits |
| `Ctrl+R`               | Global Search & Replace Interactive Module      |
| `Ctrl+G`               | Go-To Line Bounds                               |
| `Ctrl+K`               | Vaporize Line Iterators natively                |
| `Ctrl+D`               | Duplicate Current Entity Blocks                 |
| `Alt+↑` / `Alt+↓`      | Structurally Shift Lines natively Up / Down     |
| `Ctrl+↑` / `Ctrl+↓`    | Visual Viewport Slide Frame Lock Tracking       |

### Automation & Tooling
| Key                    | Action                                          |
|------------------------|-------------------------------------------------|
| `Ctrl+L`               | Native Async Backend Code Format (`prettier`)   |
| `Ctrl+E` / `Ctrl+/`    | Semantic Context-Aware Commenting Target        |
| `Tab` / `Shift+Tab`    | Multi-Line Text Frame Re-Alignments             |
| `Ctrl+T`               | Force Syntax Color Toggles                      |

---

## 🔧 Formatter Configuration Chains

Dan physically integrates external subsystems by analyzing `.editorconfig` matrices or defaulting to background shell executions safely. To access `Ctrl+L` format tracking, install the proper bindings:

- **Rust (`.rs`):** [`rustfmt`](https://github.com/rust-lang/rustfmt)
- **Python (`.py`):** [`ruff`](https://docs.astral.sh/ruff/) (`pip install ruff`)
- **Web (`.ts`, `.json`, `.css`):** [`prettier`](https://prettier.io/) (`npm i -g prettier`)

If your system lacks these targets natively, Dan handles the fallback securely and warns cleanly on the UI without causing panic matrices natively.

---

## ⚙️ Layered Configuration

Dan utilizes a comprehensive structured override environment to resolve visual settings organically:

1. **Hardcoded Internal Defaults** 
2. **Global Native TOML File** (`~/.config/dan/config.toml`)
3. **Local Project Schema** (`.editorconfig`)

Dan will automatically override internal configurations (like Space VS Tabs, Indent Arrays, Line Trailing Overloads etc) by parsing your local `.editorconfig` files natively bridging generic terminal parameters into strict local IDE matrices accurately!

### Global Config Parameters (`config.toml`)
```toml
# Soft Wrap Long Bounds
wrap_lines = true

# Fallback Geometry Space Limits
tab_width = 4
expand_tab = false

# Layout Toggles
line_numbers = true
highlight_active = true
scroll_off = 5

# Bracketed Auto Bindings
auto_close = true

# UI Component Visibility
show_help = true
show_encoding = true
show_lang = true
fast_scroll_steps = 10
```

---

## 🛡️ Audited & Benchmarked
Dan's entire architecture has been rigorously audited mathematically, natively purging `unwrap()` limits, dynamically bounding geometry sequences truncating variable bounds, and explicitly mitigating shell injection pipelines globally preventing execution parameters natively gracefully!

*(V2 Architecture completely isolates the application from blocking `.to_string()` standard library delays maintaining strict CPU and Memory boundaries seamlessly globally.)*

---
**License**: MIT 

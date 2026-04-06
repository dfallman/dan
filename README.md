# Dan

Dan is an incredibly fast, highly reliable, and radically efficient terminal text editor. Built in Rust from the ground up to prioritize zero-latency text manipulation and deterministic stability, Dan throws away archaic modal paradigms in favor of raw performance and intuitive, standard keystrokes.

It is explicitly engineered to handle multi-gigabyte payloads, execute instant background formatting, and flawlessly sustain 60FPS fluid navigation over heavily constrained and fluctuating SSH connections—all while preventing data loss through robust background persistence.

## Core Architecture & Reliability

- **Differential Screen Rendering**: The rendering pipeline utilizes an aggressive delta-computation system. Dan only flushes the exact structural text differences directly to standard output frame-by-frame, drastically minimizing I/O and entirely eliminating scroll tearing over network connections.
- **Rope Data Structure Backing**: The text buffer is internally powered by a Rope graph designed to withstand punishing mutation intervals. This guarantees O(log N) insertion and deletion complexities, maintaining constant memory footprints and instantaneous mutations regardless of file scale.
- **Fault-Tolerant Native .swp Recovery**: All unsaved changes are asynchronously serialized to a `.swp` recovery file every 5 seconds. If your SSH connection drops, your terminal crashes, or you experience a power cycle, Dan will immediately prompt to recover your atomic state identically upon reopening.

## Feature Overview

- **Layered Deterministic Configuration**: Reads from Internal Defaults $\rightarrow$ `~/.config/dan/config.toml` $\rightarrow$ local `.editorconfig` matrices dynamically. It fully respects project-level stylistic constraints (tab widths, CRLF vs LF line endings, trailing whitespace trims) instantly.
- **Asynchronous Auto-Formatter (`Ctrl+L`)**: Never block the main thread while formatting. Dan securely pipes the active buffer to external industry-standard binaries (like Prettier, Rustfmt, or Ruff) in a background thread, effortlessly hot-swapping the buffer when execution confirms success without interrupting your cursor sequence.
- **True Syntax and Encoding Awareness**: Beyond just stripping simple file extensions, Dan intelligently parses complex hidden targets (like `Cargo.lock`, `Makefile`, and `.bashrc`). It reliably digests raw Unicode, Shift-JIS, and legacy formats locally, converting correctly to pristine UTF-8.
- **Automatic Environment Introspection**: Dan fires native OSC 11 ANSI sequences sequentially on boot to ascertain your shell’s true background luminance dynamically—auto-selecting optimal high-contrast (`OneHalfDark`/`OneHalfLight`) rendering without manual intervention, while still bundling 20+ specialized syntax themes.
- **Automatic Pair Insertion**: Writing structural code is accelerated by automatic bracket and quote closures (`()`, `[]`, `{}`), including the ability to wrap existing active select regions simultaneously.

## Comprehensive Keybindings

Dan relies on explicit, intuitive chord patterns—eliminating the need to memorize arbitrary single-character modality switches. 

### File Actions & General
| **Key**                         | **Action**                                               |
| ------------------------------- | -------------------------------------------------------- |
| `Ctrl` + `S`                    | **Save**: Atomic write to disk.                          |
| `Ctrl` + `A`                    | **Save As**: Interactive prompt to write to a new path.  |
| `Ctrl` + `Q`                    | **Quit**: Safe exit (halts if there are unsaved changes).|
| `Ctrl` + `Shift` + `C`          | **Force Quit**: Exit instantly and flush recovery file.  |
| `Ctrl` + `H`                    | **Help Bar**: Toggle the built-in reference footer.      |

### Standard Text Editing
| **Key**                         | **Action**                                                       |
| ------------------------------- | ---------------------------------------------------------------- |
| `Ctrl` + `C` / `X` / `V`        | **Clipboard**: Access standard copy, cut, and paste pipelines.   |
| `Ctrl` + `Z` / `Y`              | **Undo / Redo**: Navigate the persistent infinite mutation tree. |
| `Ctrl` + `D`                    | **Duplicate**: Clone the current line or selection context.      |
| `Ctrl` + `K`                    | **Delete Line**: Erase the active line or selection block.       |
| `Ctrl` + `E` (or `Ctrl` + `/`)  | **Toggle Comment**: Invert comment state using syntax-aware characters. |
| `Ctrl` + `W`                    | **Word Wrap**: Hot-toggle between soft-wrapping and horizontal scroll.|
| `Ctrl` + `L`                    | **Lint/Format**: Execute standard background code formatters.    |
| `Alt` + `↑` / `↓`               | **Swap Line**: Swap current contiguous block upward or downward. |
| `Tab` / `Shift` + `Tab`         | **Indent / Dedent**: Shift selection boundaries via configured tabs. |

### Advanced Selection Context
| **Key**                         | **Action**                                                       |
| ------------------------------- | ---------------------------------------------------------------- |
| `Ctrl` + `@`                    | **Select All**: Immediately highlight the entire buffer.         |
| `Shift` + `Arrows`              | **Standard Select**: Highlight raw continuous characters.        |
| `Shift` + `Home` / `End`        | **Line Span Select**: Highlight until horizontal boundary limits.|
| `Ctrl` + `Shift` + `←` / `→`    | **Word Block Select**: Highlight contiguous syntactic chunks.    |
| `Alt` + `Shift` + `←` / `→`     | **Alternate Word Select**: Highlight contiguous syntactic chunks.|

### High-Speed Navigation
| **Key**                         | **Action**                                               |
| ------------------------------- | -------------------------------------------------------- |
| `Ctrl` + `↑` / `↓`              | **In-place Scroll**: Shift viewport natively without moving cursor. |
| `Ctrl` + `Shift` + `↑` / `↓`    | **Fast Scroll**: Granularly accelerate viewport Y-axis shifts.      |
| `Ctrl` + `←` / `→`              | **Word Jump**: Leap over tokens and symbols.                        |
| `Alt` + `←` / `→`               | **Alternate Word Jump**: Leap over tokens and symbols.              |
| `Ctrl` + `Home` / `End`         | **Buffer Ends**: Seek instantly to start/EOF of the file array.     |
| `Ctrl` + `G`                    | **Go-To Line**: Prompt and jump precisely to numeric markers.       |

### Search & Interactive Replace
| **Key**                         | **Action**                                               |
| ------------------------------- | -------------------------------------------------------- |
| `Ctrl` + `F` (or `F7`)          | **Search Mode**: Initiate dynamic string pattern query.  |
| `Enter` (or `Ctrl` + `G`)       | **Search Focus Next**: Leap forward to next target hit.  |
| `Shift` + `Enter`               | **Search Focus Previous**: Reverse leap to prior target. |
| `Ctrl` + `R`                    | **Replace Context**: Enter the sequential replacer pipeline. |

*(Note: The Replace Context walks through Target Query $\rightarrow$ Replacement String $\rightarrow$ Action Stepper [Yes/No/All/Quit]).*

## Advanced Configuration

Edit user-level targets freely at `~/.config/dan/config.toml`. 

```toml
# Display Optimization
wrap_lines = true       # Enforce word boundary wrapping algorithms
tab_width = 4           # Space calculation for single level depth
expand_tab = false      # Mutate raw tabs to whitespace equivalent
line_numbers = true     # Render active line indexing matrix
highlight_active = true # Isolate editing line luminance
scroll_off = 5          # Number of structural lines enforcing padding
fast_scroll_steps = 10  # Delta coefficient for fast vertical scanning

# Active Editing
auto_indent = true      # Clone prior whitespace arrays linearly
auto_close = true       # Inject closure algorithms naturally
syntax_highlight = true # Toggle native highlighting

# Terminal Interface
show_help = true        # Persist dynamic shortcut reference bar
show_encoding = true    # Track active file interpretation payload
show_lang = true        # Track current syntax execution pipeline

# Aesthetic Logic
theme = "default"       # Use OS heuristics natively if "default"
```

## Compilation & Installation

Dan guarantees high mechanical execution speeds through Rust. A minimum supported Rust Version of **`1.94`** is explicitly required for accurate resolution. We recommend acquiring the latest toolchains directly via [rustup.rs](https://rustup.rs/).

### Linux & macOS Formats

```bash
git clone https://github.com/dfallman/dan.git && cd dan
cargo install --path .
```
*(Optionally explicitly copy `/target/release/dan` to `/usr/local/bin` after building)*

### Windows Pipeline (PowerShell)

```powershell
git clone https://github.com/dfallman/dan.git; cd dan
cargo build --release
Copy-Item target\release\dan.exe ~/.cargo/bin/
```

## Tracked Architecture Roadmap

Dan focuses specifically on eliminating friction with maximum determinism, explicitly ruling out heavy bloated framework-level functionalities. However, ongoing critical engine upgrades include:

- **Configurable Binding Topography**: Decoupling the input interpretation matrix to allow unrestricted structural reassignment.
- **Vertical Code Splitting**: Isolating read/write pipelines across dual-pane configurations natively.
- **Multi-Cursor Manipulation**: Orchestrating sequence edits across parallel vertical line contexts identically.
- **Regex Query Resolution**: Pushing standard regular expressions natively through the find/replace pipeline.
- **Mouse Orchestration Matrix**: Supporting scroll-wheel and click-and-drag terminal coordinate interactions safely.
- **Extended Markdown Lexing**: Expanding abstract tree evaluation visually formatting bold and italic syntactic constructs.
- **POSIX SIGTSTP Injection**: Hooking native interrupt routines to cleanly suspend/resume directly to the parent shell.

---
**License**: MIT

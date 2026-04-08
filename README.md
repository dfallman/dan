# dan

**Dan** is a fast, friendly, and zero fuss terminal text editor. Written natively in [Rust](https://rust-lang.org/), it is designed to be modeless, resource-efficient, and with the lowest possible latency, even over fluctuating SSH connections. It's architectured to handle very large files and complex editing tasks, while remaining easy to use and configure to your liking. Dan works in most terminal emulator environments, whether you're using macOS, Linux, or Windows.

Dan has no strange modes to learn, no archaic shortcuts, and no massive dot files. Dan ships with sensible defaults, so most users can start using Dan without any configuration.

<img width="3584" height="2036" alt="CleanShot 2026-04-06 at 15 01 18@2x" src="https://github.com/user-attachments/assets/bb923556-50f5-439d-b745-6ca87344b607" />

# Quick install
Install/update to latest version of [Rust](https://rustup.rs/)
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
Clone, build, and install Dan
```
git clone https://github.com/dfallman/dan.git
cd dan
cargo build --release
cargo install --path .
```
For more installation options, see Install below.


# Features

### At a glance

- **Zero configuration**: Dan works out of the box with sensible defaults. No need to configure anything. However, Dan is highly configurable if you want it to be, see below.

- **Sensible defaults**: Dan uses familiar shortcuts and keybindings, so you don't need a cheat sheet. Use `Ctrl-C` and `Ctrl-V` for copy and paste, `Ctrl-S` to save, `Ctrl-Z` and `Ctrl-Y` for undo and redo, `Ctrl-F` to search, and `Ctrl-Q` to quit. If you're lost, just press `Ctrl-H` for help and it's all there.

- **Smart rendering engine**: Dan uses a differential rendering system. By computing exactly what has changed on your screen, it only sends the necessary updates to your terminal. This makes scrolling 100MB files over SSH feel as smooth as local editing.

- **Reliability and performance**: Using a so-called Rope data structure, Dan can handles _massive_ files with a constant memory footprint. It's built entirely in Rust, so it's fast, memory efficient, and reliable. We've built Dan to favor speed and reliability over bells and whistles. Did we say it's _fast_?

- **Full Unicode & CJK support**: Dan handles Chinese, Japanese, and Korean characters perfectly (or, at least, that's the plan), maintaining correct visual alignment even with double-width characters and emojis.

- **OS clipboard integration**: Dan integrates with the OS clipboard, so you can copy and paste between Dan and other applications. This works on Linux, macOS, and Windows.

- **Auto-save and recovery**: Dan features a 5-second background autosave to a temporary buffer. This means that if your terminal crashes or you lose power, your work is safely tucked away in a `.swp` file for easy recovery. Just fire up Dan again to recover your work.


### Tools for developers, useful for everyone

- **Automatic encoding detection**: Opening an old file? Dan intelligently sniffs legacy formats (like Shift-JIS or Windows-1252) and converts them to clean UTF-8 for editing.

- **Syntax highlighting**: Dan features accurate and fast syntax highlighting for a wide range of languages. It auto-detects your terminal's color scheme and picks a sensible default, but also supports a range of custom color themes.

- **Quick formatting**: Clean up your code instantly with `Ctrl-L`. Dan pipes your text through industry-standard tools like **Prettier**, **Ruff**, or **Rustfmt** in the background. It's non-blocking, so you can keep typing while it's working.

- **Smart comment toggling**: Use `Ctrl+E` to toggle language-specific comments across multi-line selections. Dan understands the syntax logic of your file and will comment out a single line or your selected section.

- **Automatic pair insertion**: Save keystrokes with auto-closing brackets. If you highlight a block of code and type a bracket, Dan will wrap the selection for you.


### Under the hood

- **Differential screen rendering**: The rendering pipeline utilizes an aggressive delta-computation system. Dan only flushes the exact structural text differences directly to standard output frame-by-frame, drastically minimizing I/O and entirely eliminating scroll tearing over network connections.

- **Rope data structure architecture**: The text buffer is internally powered by a Rope graph designed to withstand punishing mutation intervals. This guarantees O(log N) insertion and deletion complexities, maintaining constant memory footprints and instantaneous mutations regardless of file scale.

- **Fault-tolerant recovery**: All unsaved changes are asynchronously serialized to a `.swp` recovery file every 5 seconds. If your SSH connection drops, your terminal crashes, or you experience a power cycle, Dan will immediately prompt to recover your atomic state identically upon reopening.

- **Layered deterministic configuration**: Reads from Internal Defaults $\rightarrow$ `~/.config/dan/config.toml` $\rightarrow$ local `.editorconfig` matrices dynamically. It fully respects project-level stylistic constraints (tab widths, CRLF vs LF line endings, trailing whitespace trims) instantly.

- **Asynchronous auto-formatter**: Never block the main thread while formatting. Dan securely pipes the active buffer to external industry-standard binaries (like Prettier, Rustfmt, or Ruff) in a background thread, effortlessly hot-swapping the buffer when execution confirms success without interrupting your cursor sequence.

- **True syntax and encoding awareness**: Beyond just stripping simple file extensions, Dan intelligently parses complex hidden targets (like `Cargo.lock`, `Makefile`, and `.bashrc`). It reliably digests raw Unicode, Shift-JIS, and legacy formats locally, converting correctly to pristine UTF-8.

- **Automatic environment introspection**: Dan fires native OSC 11 ANSI sequences sequentially on boot to ascertain your shell's true background luminance dynamically—auto-selecting optimal high-contrast (`OneHalfDark`/`OneHalfLight`) rendering without manual intervention, while still bundling 20+ specialized syntax themes.

- **Automatic pair insertion**: Writing structural code is accelerated by automatic bracket and quote closures (`()`, `[]`, `{}`), including the ability to wrap existing active select regions simultaneously

- **OS clipboard integration**: Dan leverages the [arboard crate](https://docs.rs/arboard/latest/arboard/) for cross-platform OS clipboard integration, gracefully falling back to an in-memory clipboard if system access is unavailable. This hybrid architecture ensures that copy-paste operations remain functional within the editor even in restricted environments or over remote connections where a display server is missing.


# Keyboard shortcuts (keybindings)

Dan tries to use familiar and intuitive shortcuts, so you shouldn't need to print off a cheat sheet. If you're ever in doubt, hit `Ctrl-H` to toggle the help bar.

**Note**: terminal emulators (i.e. your terminal app such as [iTerm2](https://iterm2.com/), [Kitty](https://sw.kovidgoyal.net/kitty/), [Alacritty](https://alacrittywm.org/), [Ghostty](https://ghostty.dev/), [WezTerm](https://wez.dev/), and others) struggle with certain keybindings because they rely on an archaic system of so-called 'escape sequences,' where many modern key combinations (such as `Ctrl` + `Shift` + `↓`) either send the exact same code as the base key (without the `Shift`) or simply aren't defined in the standard protocols used by legacy shells. 

What this means in practice is that terminal editors can't typically use exactly the same keyboard shortcuts as a GUI editor. macOS users in particular might find the use of `Ctrl` instead of `Cmd` (`⌘`) unusual and difficult to get used to. Others, coming from linux and Windows environments, will feel right at home. 

For macOS users who simply can't get used to using `Ctrl` over `⌘`, note that many terminal emulators have ways of remapping keys. For example, in iTerm2, you can remap `Ctrl` to the `⌘` in `Preferences` > `Keys` > `Key Bindings`. This means that you can then, as an example, use the left `⌘` key as `Ctrl` in your terminal (and Dan), while the right `⌘` is still available to control iTerm2 and the OS while iTerm2 is active. Finally, for macOS users, we strongly recommend using an alternative terminal emulator such as [iTerm2](https://iterm2.com/) (see list above) over the built-in Terminal app.

### Basic operation
| **Key**                         | **Action**                                               |
| ------------------------------- | -------------------------------------------------------- |
| `↑` `↓` `←` `→`                 | **Move cursor**: Move the cursor around in your file.   |
| `Ctrl` + `S`                    | **Save**: Write buffer to disk (i.e. save).                          |
| `Ctrl` + `A`                    | **Save As**: Write buffer to a new path (file).          |
| `Ctrl` + `Q`                    | **Quit**: Safe exit (prompts if there are unsaved changes).|
| `Ctrl` + `H`                    | **Help**: Toggle the built-in reference cheat sheet.      |


### Text editing
| **Key**                         | **Action**                                                       |
| ------------------------------- | ---------------------------------------------------------------- |
| `Ctrl` + `C` / `X` / `V`        | **Clipboard**: Copy, cut, and paste to clipboard.   |
| `Ctrl` + `Z` / `Y`              | **Undo / Redo**: Access Dan's persistent infinite mutation tree. |
| `Ctrl` + `D`                    | **Duplicate**: Clone the current line or selection.      |
| `Ctrl` + `K`                    | **Delete line**: Erase the active line or selection block.       |
| `Ctrl` + `E` (or `Ctrl` + `/`)  | **Toggle comment**: Activate/remove comment (syntax-aware). |
| `Ctrl` + `T`                    | **Toggle syntax highlight**: Turn on and off syntax highlighting. |
| `Ctrl` + `W`                    | **Word wrap**: Hot-toggle between soft-wrapping and horizontal scroll.|
| `Ctrl` + `L`                    | **Lint/Format**: Auto-format the document using an external linter.    |
| `Alt` + `↑` / `↓`               | **Swap line**: Swap current line/block upward or downward. |
| `Tab` / `Shift` + `Tab`         | **Indent / Dedent**: Indent or dedent current line/selection. |

### Advanced selection
| **Key**                         | **Action**                                                       |
| ------------------------------- | ---------------------------------------------------------------- |
| `Ctrl` + `\`                    | **Select all**: Select the entire buffer.         |
| `Shift` + `Arrows`              | **Standard select**: Select continuous characters/lines.        |
| `Ctrl`/`Alt` + `Shift` + `←` / `→`    | **Word block select**: Fast select by words/chunks.    |

### High-speed navigation
| **Key**                         | **Action**                                               |
| ------------------------------- | -------------------------------------------------------- |
| `Ctrl` + `↑` / `↓`              | **In-place scroll**: Scroll the viewport without moving the cursor. |
| `Ctrl` + `Shift` + `↑` / `↓`    | **Fast scroll**: Scroll multiple lines per keypress.      |
| `Ctrl` or `Alt` + `←` / `→`     | **Word jump**: Go to next word, leap over tokens and symbols.                        |
| `Ctrl` + `Home` / `End`         | **Buffer ends**: Jump instantly to start/EOF of the file.     |
| `Ctrl` + `G`                    | **Go-to line**: Jump to specific line number.       |

### Search & replace
| **Key**                         | **Action**                                               |
| ------------------------------- | -------------------------------------------------------- |
| `Ctrl` + `F`                    | **Search**: Search for phrase.  |
| `Ctrl` + `R`                    | **Replace**: Search and replace phrase.  |


# Installation
You need the latest [Rust](https://rustup.rs/) to compile Dan from source (currently, v1.94 is recommended). Note that on most systems, installing rustc using your package manager (such as apt and brew) will _not_ give you the latest version, and older versions might fail to compile Dan. Therefore, we highly recommend you use the official rust installation manager available at [https://rustup.rs/](https://rustup.rs/) instead.

For all unix based systems, you can use:

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

To install on Windows, [follow these instructions](https://rustup.rs/#).

Once you have Rust installed, let's go ahead and build Dan:

### Installing Dan on macOS & Linux

```
git clone https://github.com/dfallman/dan.git
cd dan
cargo build --release

# Move to your local path
cp target/release/dan /usr/local/bin/

# ...alternatively
cargo install --path .
```

### Installing Dan on Windows
For Windows PowerShell users, see instructions below. **Note**: if you're running Dan inside of a WSL environment, you should follow the instructions for Linux instead (see above).

```
git clone https://github.com/dfallman/dan.git
cd dan
cargo build --release

# Move to your Cargo bin
Copy-Item target\release\dan.exe ~/.cargo/bin/
```


# Configuration
Out of the box, Dan follows a layered configuration model, but is project-aware. This means it looks for settings in this order:

1. **Internal Defaults**: Dan's sensible defaults
2. **Global Config**: Your global settings stored in `~/.config/dan/config.toml`.
3. **Local Project Style**: Project-specific settings stored in `.editorconfig`.

### Project-aware styling
...however, Dan understands that different projects have different rules. Whenever you open a file, Dan automatically scans for local `.editorconfig` guidelines. It also automatically parses your project's specific styling rules—like tab sizes, space preferences, trailing whitespace rules, and line endings—and automatically prioritizes them over your global defaults. The idea is that you should never have to manually adjust your editor just to contribute to a new project. Also, you'll not drive your collaborators nuts by consistently using your preferred indentation or line endings (although, as we all know, Tab and 4 spaces are the only acceptable indentation options. That's why they're the defaults).


## Global config

Your global config file should be saved in `~/.config/dan/config.toml`

If you're new to unix-based systems, `~/` denotes your home folder. In macOS, this folder is `/Users/<username>` and in Linux it's (usually) at `/home/<username>`.

**Note**: this file is _not_ created by default as Dan ships with sensible (and somewhat opinionated) defaults and doesn't _require_ any configuration to be used. If you do wish to tweak Dan however, simply create the config file: 
```dan ~/.config/dan/config.toml```

Then, copy and paste the below defaults into it using `Ctrl-V`. Change what you want to have changed and hit save: `Ctrl-S` then `Ctrl-Q` to quit. Next time you restart Dan your settings should be active.

**Note**: on Windows systems (outside of WSL), the global config file can also be saved to `C:\Users\<username>\AppData\Roaming\dan\config.toml` if you prefer not to use the `.config` directory.

```toml
# Display
wrap_lines = true       # Enforce word boundary wrapping algorithms
tab_width = 4           # Space calculation for single level depth
expand_tab = false      # Mutate raw tabs to whitespace equivalent
line_numbers = true     # Render active line indexing matrix
highlight_active = true # Isolate editing line luminance
scroll_off = 5          # Number of structural lines enforcing padding
fast_scroll_steps = 10  # Delta coefficient for fast vertical scanning

# Editing
auto_indent = true      # Clone prior whitespace arrays linearly
auto_close = true       # Inject closure algorithms naturally
syntax_highlight = true # Toggle native highlighting

# Interface
show_help = true        # Persist dynamic shortcut reference bar
show_encoding = true    # Show active file encoding
show_lang = true        # Show current syntax highlighting language

# Theme for syntax highlighting
theme = "default"       # Default color theme for light/dark
```

# Themes
Dan uses [syntect](https://github.com/trishume/syntect/) for reliable and accurate syntax highlighting for a large number of file types. If you don't set a theme (i.e. you use "default"), Dan will query your terminal emulator on startup to attempt to understand if the terminal is dark or light and pick a default theme. For dark terminals, the default is "OneHalfDark" and for light terminals it's "OneHalfLight". Note that these themes change the color of the syntax highlighting, they don't impact the color of Dan's interface elements. 

You can customize your theme by editing the configuration file. Note that if you change the theme (i.e. you're not using "default"), Dan will use this theme regardless of your terminal's  scheme. If you're using a theme designed for dark terminals on a light terminal, it might be difficult to read. Turn syntax highlighting on and off using `Ctrl-T`.

To set one of the themes listed below, change the `theme` value in the configuration file to the name of the theme you want to use, such as:

```
# Theme for syntax highlighting
theme = "DarkNeon"
```

### Available color themes:
- `1337`: A retro, high-contrast dark theme inspired by old-school hacker culture.
- `Coldark-Cold`: A clean, blue-tinted light theme.
- `Coldark-Dark`: A deep, cool-blue dark theme.
- `DarkNeon`: A vibrant dark theme bursting with bright neon accents.
- `Dracula`: A popular, high-contrast dark theme with distinct purple and pink accents.
- `GitHub`: A light theme accurately mimicking the classic GitHub code view.
- `Monokai Extended`: An updated version of the classic, vivid Monokai theme.
- `Monokai Extended Bright`: A brighter, higher-contrast variant of the Monokai palette.
- `Monokai Extended Light`: A light-background adaptation of the Monokai colors.
- `Monokai Extended Origin`: The authentic, unaltered original Monokai color palette.
- `Nord`: A clean, arctic-inspired dark theme with frosty blue tones.
- `OneHalfDark`: A clean, modern dark theme based heavily on the Atom "One" series (default for dark terminals).
- `OneHalfLight`: A clean, modern light theme based on the Atom "One" series (default for light terminals).
- `Solarized (dark)`: A very popular, scientifically formulated low-contrast dark theme.
- `Solarized (light)`: The light-background version of the Solarized palette.
- `Sublime Snazzy`: A vibrant dark theme with bright, elegant (snazzy) colors.
- `TwoDark`: A dark theme inspired by Atom's One Dark but tuned for slightly better contrast.
- `Visual Studio Dark+`: Accurately emulates the prominent default dark theme of VS Code.
- `ansi`: A minimal, dynamic theme that falls back to your terminal's built-in 16 ANSI colors.
- `base16`: A balanced, standard boilerplate dark theme from the base16 project.
- `base16-256`: A variant of base16 specifically optimized for limited 256-color palette terminals.
- `gruvbox-dark`: A retro "groove" color scheme with earthy, warm dark tones.
- `gruvbox-light`: A retro "groove" color scheme with earthy, warm light tones.
- `zenburn`: A low-contrast "alien" dark color scheme designed to be extremely easy on the eyes.


# Formatter
Dan comes with a straightforward and simple yet powerful approach to auto-formatting (or linting) your document. Rather than implementing its own formatting algorithms, Dan relies on existing external tools to format your document (that do this for a living). Dan securely pipes the active buffer to the external tools (at the moment, we support **Prettier**, **Rustfmt**, and **Ruff**) in a background thread, and then hot-swaps the buffer when execution confirms success without interrupting your cursor sequence. Hence, the formatting happens asynchronous in the background, so we'll never block the main thread while formatting (which, unless you're linting a very large file, is rarely a problem anyway as it's usually fast).

Engage the linting/formatter at any time using `Ctrl-L`. 

For this to work, ensure that your system has the following tools installed (one or all of them, depending on what languages you're working with). Dan picks the right one for the right language:
- For **Rust**: [rustfmt](https://github.com/rust-lang/rustfmt) (`rustup component add rustfmt`)
- For **Python**: [ruff](https://docs.astral.sh/ruff/) (`pip install ruff`)
- For **Web/Javascript/Typescript/JSON**: [prettier](https://prettier.io/) (`npm i -g prettier`)

Dan shows the output from the linter in the interface so you can see if something didn't go as planned.


# Roadmap
Dan remains a work in progress, prioritizing speed, stability, and security over feature density or complex customization. While the goal is a streamlined editing experience rather than the most feature-rich environment, the following roadmap outlines our planned trajectory; we welcome your feedback and suggestions on these upcoming priorities.

- **Mouse support**: click-to-position, scroll wheel, click-drag select
- **Regex search**: use regex expressions in search and replace
- **Extended markdown support**: improve rendering of markdown script, such as showing **bold**, _italic_, and perhaps other features such as links and tables
- **Buffer switching**: Dan has multi-buffer plumbing but currently only supports a single buffer at a time. While useful, tools like tmux, screen, Zellij, and others make this less cruical and adds complexity, not least to the interface.
- **Vertical split**: view two files or two parts of the same file
- **Suspend & resume**: use SIGTSTP as a simple and effective way to switch between the editor and the terminal.
- **Configurable keybindings**: allow users to select their preferred keybindings
- **Multi-cursor**: allow multiple cursors to edit many lines/occurances simultaneously 
- **Built-in terminal** or shell pipe
- **Plugin interface**: in a v1, 'run external command, pipe selection'

---

**License**: GNU General Public License v3.0 (GPLv3)

# Mouse + Scroll Wheel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable click-to-place cursor, drag-to-select, and wheel scroll in normal editing, with `mouse = true` by default and a config escape hatch.

**Architecture:** Command-centric: `Event::Mouse` → `MouseDown`/`MouseDrag`/`MouseUp` (or existing scroll commands) → dispatch. Pure `screen_to_buffer` hit-test in `src/editor/mouse.rs`. `TerminalGuard::enter(mouse)` enables capture; restore/panic always disable it.

**Tech Stack:** Rust, crossterm mouse events, existing `Command` / `CursorSet` / `visual_rows_for` / `char_idx_for_visual_col`, `cargo test`.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-09-mouse-scroll-design.md`
- Mouse only in `Mode::Editing`; all other modes → `Noop`
- Wheel → existing `ScrollViewportUp` / `ScrollViewportDown`
- `mouse = false`: no capture; mouse commands early-return in dispatch
- No palette/prompt/chrome mouse; no double-click / right-click
- Indent with tabs (project style)
- TDD: failing test first for each behavior

## File map

| File | Responsibility |
|------|----------------|
| `src/config/mod.rs` | `mouse: bool` default `true` |
| `src/editor/commands.rs` | `MouseDown` / `MouseDrag` / `MouseUp` |
| `src/editor/mouse.rs` | `screen_to_buffer` + unit tests |
| `src/editor/mod.rs` | `mod mouse;` |
| `src/editor/dispatch/mouse.rs` | `cmd_mouse_down/drag/up` |
| `src/editor/dispatch/mod.rs` | wire mouse module + match arms |
| `src/input/mod.rs` | map `Event::Mouse`; table tests |
| `src/terminal_guard.rs` | `enter(mouse: bool)`; disable on restore |
| `src/main.rs` | pass `config.mouse`; panic hook disable |
| `src/render/mod.rs` | `Viewport::from_cached` for hit-test chrome math (optional extract) |
| `README.md` | config + mouse note |
| `doc/2026-07-09-codebase-audit.md` | check off mouse item |

---

### Task 1: Config `mouse = true`

**Files:**
- Modify: `src/config/mod.rs`
- Modify: `README.md` (config block only in this task’s docs step — or defer README to Task 7; include a minimal default assertion here)

**Interfaces:**
- Produces: `Config.mouse: bool` default `true`

- [ ] **Step 1: Write the failing test**

In `src/config/mod.rs`, if there is no tests module yet, add one; otherwise extend:

```rust
#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn mouse_defaults_to_true() {
		assert!(Config::default().mouse);
	}

	#[test]
	fn mouse_false_from_toml() {
		let c: Config = toml::from_str("mouse = false").unwrap();
		assert!(!c.mouse);
	}
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test mouse_defaults_to_true -- --nocapture`

Expected: FAIL (no field `mouse`)

- [ ] **Step 3: Add field**

```rust
/// Enable terminal mouse capture (click, drag-select, wheel).
pub mouse: bool,
```

In `Default`: `mouse: true`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test mouse_ -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/config/mod.rs
git commit -m "config: add mouse = true default"
```

---

### Task 2: Commands + input mapping

**Files:**
- Modify: `src/editor/commands.rs`
- Modify: `src/input/mod.rs`

**Interfaces:**
- Produces:
  - `Command::MouseDown { col: u16, row: u16 }`
  - `Command::MouseDrag { col: u16, row: u16 }`
  - `Command::MouseUp { col: u16, row: u16 }`
- Consumes: `crossterm::event::{MouseEvent, MouseEventKind, MouseButton}`

- [ ] **Step 1: Write failing input tests**

Add to `src/input/mod.rs` tests (import mouse types):

```rust
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

fn mouse(kind: MouseEventKind, col: u16, row: u16) -> Event {
	Event::Mouse(MouseEvent {
		kind,
		column: col,
		row,
		modifiers: KeyModifiers::NONE,
	})
}

#[test]
fn mouse_editing_maps_click_drag_wheel() {
	use MouseEventKind as K;
	assert_map(&[
		(
			"down",
			mouse(K::Down(MouseButton::Left), 3, 5),
			Mode::Editing,
			Command::MouseDown { col: 3, row: 5 },
		),
		(
			"drag",
			mouse(K::Drag(MouseButton::Left), 4, 6),
			Mode::Editing,
			Command::MouseDrag { col: 4, row: 6 },
		),
		(
			"up",
			mouse(K::Up(MouseButton::Left), 4, 6),
			Mode::Editing,
			Command::MouseUp { col: 4, row: 6 },
		),
		(
			"wheel up",
			mouse(K::ScrollUp, 0, 0),
			Mode::Editing,
			Command::ScrollViewportUp,
		),
		(
			"wheel down",
			mouse(K::ScrollDown, 0, 0),
			Mode::Editing,
			Command::ScrollViewportDown,
		),
		(
			"right click noop",
			mouse(K::Down(MouseButton::Right), 1, 1),
			Mode::Editing,
			Command::Noop,
		),
	]);
}

#[test]
fn mouse_ignored_in_palette_and_search() {
	use MouseEventKind as K;
	let ev = mouse(K::Down(MouseButton::Left), 2, 2);
	assert_eq!(map_event(&ev, Mode::Palette), Command::Noop);
	assert_eq!(map_event(&ev, Mode::Searching), Command::Noop);
	assert_eq!(map_event(&mouse(K::ScrollDown, 0, 0), Mode::Palette), Command::Noop);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test mouse_editing_maps -- --nocapture`

Expected: FAIL (unknown Command variants / Mouse still Noop)

- [ ] **Step 3: Add Command variants**

In `src/editor/commands.rs`, near motion/selection:

```rust
// -- Mouse --
MouseDown { col: u16, row: u16 },
MouseDrag { col: u16, row: u16 },
MouseUp { col: u16, row: u16 },
```

- [ ] **Step 4: Map mouse in `map_event`**

Replace the catch-all so mouse is handled:

```rust
Event::Paste(text) => Command::InsertString(text.clone()),
Event::Mouse(me) => map_mouse(me, mode),
_ => Command::Noop,
```

```rust
fn map_mouse(me: &crossterm::event::MouseEvent, mode: Mode) -> Command {
	if mode != Mode::Editing {
		return Command::Noop;
	}
	use crossterm::event::{MouseButton, MouseEventKind};
	match me.kind {
		MouseEventKind::Down(MouseButton::Left) => Command::MouseDown {
			col: me.column,
			row: me.row,
		},
		MouseEventKind::Drag(MouseButton::Left) => Command::MouseDrag {
			col: me.column,
			row: me.row,
		},
		MouseEventKind::Up(MouseButton::Left) => Command::MouseUp {
			col: me.column,
			row: me.row,
		},
		MouseEventKind::ScrollUp => Command::ScrollViewportUp,
		MouseEventKind::ScrollDown => Command::ScrollViewportDown,
		_ => Command::Noop,
	}
}
```

Update `dispatch/mod.rs` temporarily with `Command::MouseDown { .. } | Command::MouseDrag { .. } | Command::MouseUp { .. } => {}` empty arms so the project compiles until Task 4 — **or** add stub `cmd_*` in Task 4 immediately after. Prefer completing Task 4 next without leaving empty arms untested; if compile breaks, add:

```rust
Command::MouseDown { col, row } => self.cmd_mouse_down(col, row),
Command::MouseDrag { col, row } => self.cmd_mouse_drag(col, row),
Command::MouseUp { col, row } => self.cmd_mouse_up(col, row),
```

with stub methods that `let _ = (col, row);` until Task 4 fills them — only if needed to compile between tasks.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test mouse_ -- --nocapture`

Expected: PASS (input tests)

- [ ] **Step 6: Commit**

```bash
git add src/editor/commands.rs src/input/mod.rs src/editor/dispatch/mod.rs
git commit -m "input: map mouse click, drag, and wheel in Editing"
```

---

### Task 3: `screen_to_buffer` hit-test

**Files:**
- Create: `src/editor/mouse.rs`
- Modify: `src/editor/mod.rs` (`mod mouse;`)
- Modify: `src/render/mod.rs` — extract overlay counting so hit-test matches paint

**Interfaces:**
- Produces: `pub(crate) fn screen_to_buffer(editor: &Editor, screen_col: u16, screen_row: u16) -> Option<(usize, usize)>`
- Consumes: `Editor::gutter_width`, `text_area_width`, `visual_rows_for`, `char_idx_for_visual_col`, `line_len_no_newline` (via editor methods), chrome/overlay row counts

**Chrome math (must match render):**

Visible text rows are `0 .. height.saturating_sub(1 + overlay_rows)` where status is always 1 row at the bottom and overlays sit above it. Clicks with `screen_row >= visible_text_rows` → `None`.

Extract from `Viewport::from_editor`:

```rust
pub fn overlay_rows_for(editor: &Editor, width: u16, height: u16) -> u16 {
	let mut overlay: u16 = 0;
	if let Some(prompt_windows) = chrome::build_prompt(editor, width, height) {
		overlay += prompt_windows.len() as u16;
	} else if editor.show_help && !editor.palette.open {
		overlay += chrome::build_help_bar(editor, width, height).len() as u16;
	}
	if editor.info_banner_visible() {
		let base_y = height.saturating_sub(1 + overlay + 1);
		overlay += chrome::build_info_banner(editor, width, base_y).len() as u16;
	}
	overlay
}
```

Use this inside `Viewport::from_editor` and from `screen_to_buffer` with `editor.terminal_width/height` (do **not** call `terminal::size()` in hit-test).

- [ ] **Step 1: Write failing hit-test tests**

In `src/editor/mouse.rs`:

```rust
use super::Editor;

pub(crate) fn screen_to_buffer(editor: &Editor, screen_col: u16, screen_row: u16) -> Option<(usize, usize)> {
	todo!()
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::editor::commands::Command;

	fn editor_with_lines(lines: &[&str], width: u16, height: u16) -> Editor {
		let mut e = Editor::new();
		e.terminal_width = width;
		e.terminal_height = height;
		e.show_help = false;
		e.config.wrap_lines = false;
		e.config.line_numbers = true;
		e.config.mouse = true;
		// Clear default scratch and insert content via commands
		e.execute(Command::SelectAll);
		e.execute(Command::DeleteForward);
		for (i, line) in lines.iter().enumerate() {
			if i > 0 {
				e.execute(Command::InsertNewline);
			}
			for ch in line.chars() {
				e.execute(Command::InsertChar(ch));
			}
		}
		e.execute(Command::MoveBufferTop);
		e
	}

	#[test]
	fn chrome_click_is_none() {
		let e = editor_with_lines(&["hello"], 40, 10);
		// status row is y=9 when height=10, chrome_rows=1, no overlay
		assert_eq!(screen_to_buffer(&e, 5, 9), None);
	}

	#[test]
	fn nowrap_click_maps_to_char() {
		let e = editor_with_lines(&["abcdef"], 40, 10);
		let gw = e.gutter_width() + 1; // gutter + separator
		// Click first text cell of line 0 → col 0
		assert_eq!(screen_to_buffer(&e, gw as u16, 0), Some((0, 0)));
		// Click third text cell → col 2
		assert_eq!(screen_to_buffer(&e, (gw + 2) as u16, 0), Some((0, 2)));
	}

	#[test]
	fn gutter_click_goes_col_zero() {
		let e = editor_with_lines(&["abcdef"], 40, 10);
		assert_eq!(screen_to_buffer(&e, 0, 0), Some((0, 0)));
	}

	#[test]
	fn past_eol_clamps() {
		let e = editor_with_lines(&["ab"], 40, 10);
		let gw = e.gutter_width() + 1;
		assert_eq!(screen_to_buffer(&e, (gw + 50) as u16, 0), Some((0, 2)));
	}

	#[test]
	fn wrap_second_visual_row() {
		let mut e = editor_with_lines(&["abcdefghij"], 20, 10);
		e.config.wrap_lines = true;
		// Force narrow text area so the line wraps. With line numbers, text_area is small.
		e.terminal_width = 12;
		let gw = e.gutter_width() + 1;
		let text_w = e.text_area_width();
		assert!(text_w > 0 && text_w < 10, "precondition: wraps");
		// First visual row ends after text_w chars (ASCII). Second row starts at char text_w.
		assert_eq!(
			screen_to_buffer(&e, gw as u16, 1),
			Some((0, text_w))
		);
	}
}
```

Adjust wrap test numbers if gutter width for a 1-line buffer differs — assert via computing `visual_rows_for` in the test if needed.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib editor::mouse:: -- --nocapture`

Expected: FAIL (`todo!` panic or missing module)

- [ ] **Step 3: Implement `screen_to_buffer`**

Algorithm:

1. `width = editor.terminal_width`, `height = editor.terminal_height`
2. `overlay = crate::render::overlay_rows_for(editor, width, height)` (or equivalent)
3. `visible = height.saturating_sub(1 + overlay)` — if `screen_row >= visible` → `None`
4. `gutter = editor.gutter_width()`, separator = 1 if gutter logic matches render (`gutter_width + 1` for text start when line numbers on; when line numbers off, gutter is 0 and separator still 1 in render — **match `render/mod.rs`**: `gutter_width + 1` always for text start when computing text_area). Read `render/mod.rs` / `Editor::text_area_width` and mirror exactly.
5. If `screen_col < text_start` → treat as gutter: resolve which buffer line is at `screen_row`, return `(line, 0)`
6. `target_vcol` = `screen_col as usize - text_start + scroll_x` (nowrap) or without scroll_x on wrap rows
7. Walk visual rows from `(scroll_y, scroll_vrow)` for `screen_row` steps (wrap) or use `scroll_y + screen_row` (nowrap)
8. Use `char_idx_for_visual_col` for the target row
9. Past last line: clamp to last line end; empty buffer: `(0, 0)`

Wire `mod mouse` in `src/editor/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib editor::mouse:: -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/editor/mouse.rs src/editor/mod.rs src/render/mod.rs
git commit -m "editor: add screen_to_buffer mouse hit-test"
```

---

### Task 4: Dispatch mouse handlers

**Files:**
- Create: `src/editor/dispatch/mouse.rs`
- Modify: `src/editor/dispatch/mod.rs`

**Interfaces:**
- Produces: `cmd_mouse_down`, `cmd_mouse_drag`, `cmd_mouse_up` on `Editor`
- Consumes: `screen_to_buffer`, `CursorSet::{set_cursor, begin_selection}`, `config.mouse`

- [ ] **Step 1: Write failing dispatch tests**

Add tests in `src/editor/dispatch/mouse.rs` or `src/editor/mod.rs` tests:

```rust
#[test]
fn mouse_down_places_cursor() {
	let mut e = /* same helper as mouse tests, or duplicate minimal setup */;
	e.config.mouse = true;
	e.show_help = false;
	e.terminal_width = 40;
	e.terminal_height = 10;
	// ensure content "abcdef" on line 0
	let gw = (e.gutter_width() + 1) as u16;
	e.execute(Command::MouseDown { col: gw + 2, row: 0 });
	let c = e.buffer().cursors.cursor();
	assert_eq!((c.line, c.col), (0, 2));
	assert!(!e.buffer().cursors.has_selection());
}

#[test]
fn mouse_drag_selects_range() {
	let mut e = /* setup with "abcdef" */;
	let gw = (e.gutter_width() + 1) as u16;
	e.execute(Command::MouseDown { col: gw, row: 0 });
	e.execute(Command::MouseDrag { col: gw + 3, row: 0 });
	e.execute(Command::MouseUp { col: gw + 3, row: 0 });
	assert!(e.buffer().cursors.has_selection());
	let (a, b) = e.buffer().cursors.primary().ordered();
	assert_eq!((a.line, a.col), (0, 0));
	assert_eq!((b.line, b.col), (0, 3));
}

#[test]
fn mouse_disabled_is_noop() {
	let mut e = /* setup */;
	e.config.mouse = false;
	let before = e.buffer().cursors.cursor();
	e.execute(Command::MouseDown { col: 5, row: 0 });
	assert_eq!(e.buffer().cursors.cursor(), before);
}

#[test]
fn wheel_scrolls_without_moving_cursor() {
	let mut e = /* multi-line buffer, cursor at top */;
	e.buffer_mut().scroll_y = 5;
	let cur = e.buffer().cursors.cursor();
	e.execute(Command::ScrollViewportUp);
	assert_eq!(e.buffer().scroll_y, 4);
	assert_eq!(e.buffer().cursors.cursor(), cur);
}
```

Share the `editor_with_lines` helper: move it to `mouse.rs` tests as `pub(crate)` is awkward — duplicate a small private helper in dispatch tests or put `#[cfg(test)] pub(crate) fn test_editor_with_lines` in `editor/mod.rs`. Prefer a local helper in each test module to avoid test coupling.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test mouse_down_places -- --nocapture`

Expected: FAIL

- [ ] **Step 3: Implement dispatch**

`src/editor/dispatch/mouse.rs`:

```rust
use super::Editor;
use crate::editor::mouse::screen_to_buffer;

impl Editor {
	pub(crate) fn cmd_mouse_down(&mut self, col: u16, row: u16) {
		if !self.config.mouse {
			return;
		}
		let Some((line, c)) = screen_to_buffer(self, col, row) else {
			return;
		};
		self.buffer_mut().cursors.set_cursor(line, c);
		// Refresh desired_vcol for the true visual column
		let tab_w = self.tab_width();
		let vcol = crate::editor::visual_col::visual_col_at(
			self.buffer().text.line_slice(line).chars(),
			c,
			tab_w,
		);
		self.buffer_mut().cursors.primary_mut().head.desired_vcol = vcol;
		self.buffer_mut().cursors.begin_selection();
	}

	pub(crate) fn cmd_mouse_drag(&mut self, col: u16, row: u16) {
		if !self.config.mouse {
			return;
		}
		let Some((line, c)) = screen_to_buffer(self, col, row) else {
			return;
		};
		let tab_w = self.tab_width();
		let vcol = crate::editor::visual_col::visual_col_at(
			self.buffer().text.line_slice(line).chars(),
			c,
			tab_w,
		);
		let head = self.buffer_mut().cursors.primary_mut();
		head.head.line = line;
		head.head.set_col(c);
		head.head.desired_vcol = vcol;
	}

	pub(crate) fn cmd_mouse_up(&mut self, _col: u16, _row: u16) {
		// Selection already updated during drag; collapsed if never moved.
	}
}
```

Note: `set_col` already sets `desired_vcol = col` (char index); overwrite with true `vcol` after as shown.

Wire `mod mouse;` in `dispatch/mod.rs` and match arms.

Fix `begin_selection` semantics: after `set_cursor`, selection is collapsed; `begin_selection` pins anchor to head (no-op while collapsed per `CursorSet::begin_selection` — read implementation). Current code:

```rust
pub fn begin_selection(&mut self) {
	if self.selection.is_collapsed() {
		self.selection.anchor = self.selection.head;
	}
}
```

That does **not** prepare for drag by itself — head moves on drag while anchor stays. After `set_cursor`, anchor == head; drag must **only move head**, leaving anchor. So `begin_selection` is unnecessary if drag never calls `set_cursor`. **Do not call `clear_selection` on drag.** Implement drag as head-only update (as above). On mouse down, `set_cursor` is enough (anchor == head). Remove the `begin_selection()` call if it does nothing useful — keep the down handler as set_cursor + desired_vcol only.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test mouse_down_places mouse_drag_selects mouse_disabled wheel_scrolls -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/editor/dispatch/mouse.rs src/editor/dispatch/mod.rs
git commit -m "editor: dispatch mouse click and drag selection"
```

---

### Task 5: Terminal mouse capture

**Files:**
- Modify: `src/terminal_guard.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Produces: `TerminalGuard::enter(mouse: bool) -> io::Result<Self>`
- Always `DisableMouseCapture` on restore / Drop / panic hook

- [ ] **Step 1: Write a focused unit test if feasible**

`TerminalGuard` needs a real TTY — skip runtime enable test. Instead, compile-check by updating call sites and add a comment test isn’t applicable. Optional: assert `enter` signature via building `main`.

No failing test required for this task if TTY-bound; verify with `cargo build` / `cargo test`.

- [ ] **Step 2: Update `TerminalGuard`**

```rust
use crossterm::event::{
	DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
};

pub struct TerminalGuard {
	writer: BufWriter<io::Stdout>,
	raw_mode: bool,
	alt_screen: bool,
	bracketed_paste: bool,
	mouse: bool,
}

pub fn enter(mouse: bool) -> io::Result<Self> {
	// ... existing enable raw + alt + paste ...
	if mouse {
		guard.writer.get_mut().execute(EnableMouseCapture)?;
		guard.mouse = true;
	}
	Ok(guard)
}

pub fn restore(&mut self) {
	if self.mouse {
		let _ = self.writer.get_mut().execute(DisableMouseCapture);
		self.mouse = false;
	}
	// ... existing paste / alt / raw cleanup ...
}
```

- [ ] **Step 3: Update `main.rs`**

```rust
let mut terminal = terminal_guard::TerminalGuard::enter(editor.config.mouse)?;
```

Panic hook — after DisableBracketedPaste:

```rust
let _ = crossterm::ExecutableCommand::execute(
	&mut stdout,
	crossterm::event::DisableMouseCapture,
);
```

- [ ] **Step 4: Build and run full tests**

Run: `cargo test`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/terminal_guard.rs src/main.rs
git commit -m "terminal: enable mouse capture when config.mouse"
```

---

### Task 6: Docs + audit checkbox

**Files:**
- Modify: `README.md`
- Modify: `doc/2026-07-09-codebase-audit.md`

- [ ] **Step 1: README config**

In the config.toml example:

```toml
mouse = true                # Click, drag-select, wheel scroll (default: true)
```

Add a short note under shortcuts or Features:

- Mouse: click to place cursor, drag to select, wheel to scroll (disable with `mouse = false`).

- [ ] **Step 2: Audit**

Change:

```markdown
- [ ] **Mouse + scroll wheel**
```

to:

```markdown
- [x] **Mouse + scroll wheel** — Click-to-place, drag-select, wheel scroll; `mouse` config; ignored in palette/prompts. *(M)* — cleared YYYY-MM-DD
```

Add a Cleared table row.

- [ ] **Step 3: Commit**

```bash
git add README.md doc/2026-07-09-codebase-audit.md
git commit -m "docs: document mouse support and clear audit item"
```

---

### Task 7: Full verification

- [ ] **Step 1: Run full suite**

Run: `cargo test`

Expected: all PASS

- [ ] **Step 2: Manual smoke (optional, human)**

`cargo run -- /tmp/foo.txt` — click, drag, wheel; open palette and confirm mouse ignored; set `mouse = false` and confirm no capture weirdness.

- [ ] **Step 3: Final commit only if fixes were needed**

---

## Self-review vs spec

| Spec requirement | Task |
|------------------|------|
| `mouse = true` config | 1 |
| Commands + map_event Editing-only | 2 |
| Wheel → ScrollViewport* | 2 |
| `screen_to_buffer` | 3 |
| Dispatch down/drag/up + disabled gate | 4 |
| TerminalGuard + panic disable | 5 |
| README + audit | 6 |
| No palette mouse | 2 |
| Chrome click ignored | 3 |

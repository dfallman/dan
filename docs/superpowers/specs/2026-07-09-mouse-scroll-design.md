# Mouse + Scroll Wheel — Design

## Problem

Dan ignores all `Event::Mouse` input today (`map_event` falls through to `Noop`), and never enables terminal mouse capture. Click-to-place, drag-select, and wheel scroll are table stakes on modern terminals and are listed as a high-impact gap in the 2026-07-09 codebase audit.

## Goal

Add mouse support for normal editing: click to place the cursor, drag to select, and wheel to scroll the viewport. Default on, with a config escape hatch.

## Non-goals (v1)

- Mouse interaction with palette, prompts, search, help, or other chrome
- Double-click word select, triple-click line select
- Right-click / middle-click / context menus
- Horizontal scroll wheel
- Clickable status/help buttons
- Multi-cursor via mouse

## Behavior

### Config

```toml
mouse = true   # default; set false to disable capture and ignore mouse events
```

When `mouse = false`:

- Do not enable terminal mouse capture at startup
- `Event::Mouse` maps to `Noop` if any events still arrive

### Modes

Mouse events are handled only in normal editing (`Mode::Editing` and any other mode that is not a prompt/modal). Explicitly `Noop` in at least:

- `Palette`
- `Searching`, `ReplacingWith`, `ReplacingStep`
- `GoToLine`, `SaveAs`, `ConfirmQuit`, `ConfirmOverwrite`, `RecoverSwap`

Keyboard remains the only input for those surfaces in v1.

### Click

Left button press in the **text area** (including the gutter column):

1. Hit-test screen `(col, row)` → buffer `(line, col)` (or gutter → same line, col `0`).
2. Place cursor there and collapse selection.
3. Pin selection anchor at that position so a subsequent drag can extend.

Click on status / help / info chrome rows → ignore (`Noop`).
Click past end of line → clamp to line end (same as keyboard end-of-line).
Click past last buffer line → clamp to last line end (or empty buffer origin).

### Drag

While left button is held and the pointer moves over the text area:

- Keep the press-time anchor
- Move the selection head to the hit-tested buffer position
- Same selection model as shift-arrows (`Selection { anchor, head }`)

If the pointer moves over chrome or outside the terminal, keep the last valid text-area head (do not clear the selection).

### Release

Left button release finalizes the selection. If the head never moved from the anchor, the selection stays collapsed. No separate command side effects beyond updating cursor/selection state already applied during drag.

### Wheel

- Wheel up → `ScrollViewportUp` (existing command; one step per notch)
- Wheel down → `ScrollViewportDown`
- Does not move the cursor unless existing `scroll_off` logic pulls it during the next render pass
- Wheel while in non-editing modes → `Noop` (same as other mouse)

### Other mouse buttons / modifiers

Right, middle, and any modified clicks (Ctrl/Alt/Shift + mouse) → `Noop` in v1. Shift+click extend can wait for a later revision.

## Architecture

### Command-centric pipeline

Keep the existing `map_event → Command → Editor::execute` path (same as keys; unlike Resize).

New commands:

```text
MouseDown { col: u16, row: u16 }   // screen cells, 0-based
MouseDrag { col: u16, row: u16 }
MouseUp   { col: u16, row: u16 }
```

Wheel maps to existing `ScrollViewportUp` / `ScrollViewportDown` — no new scroll commands.

### Terminal enablement

Extend `TerminalGuard` to optionally enable mouse capture when entering, and always disable it on `restore` / `Drop` (and in the panic hook, mirror bracketed-paste cleanup).

Suggested shape:

- `TerminalGuard::enter()` stays as today for modes that are always on
- Add `enable_mouse(&mut self) -> io::Result<()>` called from `main` when `editor.config.mouse` is true
- Or pass a `mouse: bool` into `enter` — either is fine; prefer the smallest change that always disables on restore

Panic hook must also emit `DisableMouseCapture` so a crash does not leave the terminal eating mouse events for the shell.

### Input mapping

In `input::map_event`:

- Match `Event::Mouse(mouse)`
- If mode is not editing → `Noop`
- Map by `MouseEventKind`:
  - `Down(Left)` → `MouseDown { col, row }`
  - `Drag(Left)` → `MouseDrag { col, row }`
  - `Up(Left)` → `MouseUp { col, row }`
  - `ScrollUp` → `ScrollViewportUp`
  - `ScrollDown` → `ScrollViewportDown`
  - everything else → `Noop`

Config gating: either skip enabling capture when `mouse = false` (preferred primary gate), and/or have `map_event` / dispatch ignore mouse when disabled. Enabling capture only when configured is enough for normal terminals; still treat unexpected mouse events as `Noop` when disabled if the caller passes config or dispatch checks `config.mouse`.

Simplest consistent approach: `map_event` stays mode-aware only; `execute` for mouse commands early-returns when `!config.mouse`. Capture is not enabled when disabled, so this is belt-and-suspenders.

### Hit-test

Add a pure function (suggested location: `src/editor/viewport.rs` or a small `src/editor/mouse.rs`):

```text
screen_to_buffer(editor, screen_col: u16, screen_row: u16) -> Option<(line, col)>
```

Responsibilities:

1. Reject rows in the chrome band (`height - chrome_rows .. height`) and rows above the text area → `None`
2. Subtract gutter width (+ separator); if click is in the gutter, return `(line, 0)` for the line under that row
3. Account for `scroll_y` / `scroll_vrow` (wrap) or `scroll_y` + `scroll_x` (nowrap)
4. Map visual column → char index using the same width rules as rendering (`tab_width`, CJK via `char_width`)
5. Clamp past EOL / past EOF as described under Click

Reuse `visual_rows_for` and existing gutter / text-area width helpers so wrap and nowrap stay consistent with paint.

### Dispatch

New handlers in `src/editor/dispatch/` (motion or a tiny `mouse.rs` module):

- `cmd_mouse_down`: `screen_to_buffer` → `set_cursor` + `begin_selection` (anchor = head)
- `cmd_mouse_drag`: `screen_to_buffer` → move head only (do not move anchor); if `None`, leave state unchanged
- `cmd_mouse_up`: optional no-op / ensure selection state is consistent; if never dragged, remains collapsed

Do not clear selection on unrelated mouse no-ops.

## Testing

No real terminal required.

### Hit-test

- Nowrap: click maps to correct `(line, col)` given `scroll_y` / `scroll_x`
- Wrap: click on a wrapped visual row maps to the correct char index
- Gutter click → `(line, 0)`
- Chrome row → `None`
- Past EOL → clamped; empty buffer → `(0, 0)` or equivalent origin

### Input

- Wheel up/down → scroll commands in `Editing`
- Left down/drag/up → mouse commands in `Editing`
- Any mouse in `Palette` / `Searching` → `Noop`

### Dispatch

- Click places cursor and collapses prior selection
- Drag from `(l1,c1)` to `(l2,c2)` yields a non-collapsed selection with correct ordered range
- Wheel changes `scroll_y` without changing cursor (absent scroll_off pull on next render — assert pre-render scroll/cursor if testing at command level)

## Docs

- README keyboard/mouse section or shortcuts table: note click, drag-select, wheel
- README config block: `mouse = true`
- Check off **Mouse + scroll wheel** in `doc/2026-07-09-codebase-audit.md`

## Files (expected)

| Area | Files |
|------|--------|
| Config | `src/config/mod.rs`, README |
| Terminal | `src/terminal_guard.rs`, `src/main.rs` (panic hook + enable) |
| Commands | `src/editor/commands.rs` |
| Input | `src/input/mod.rs` |
| Hit-test | `src/editor/viewport.rs` and/or `src/editor/mouse.rs` |
| Dispatch | `src/editor/dispatch/mod.rs` + motion or `mouse.rs` |
| Audit | `doc/2026-07-09-codebase-audit.md` |

## Success criteria

- With default config, user can click to move the cursor, drag to select, and wheel-scroll in the editing view
- Palette and prompts remain keyboard-only
- `mouse = false` never enables capture and ignores mouse commands
- Terminal mouse mode is cleared on normal exit, signal exit, and panic
- Unit tests cover hit-test, mapping, and selection behavior without a TTY

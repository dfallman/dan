> **ARCHIVED — DONE (shipped 2026-07-09).** Design for sniff INFO banner.
> Live tracker: [`../../../BACKLOG.md`](../../../BACKLOG.md).

# Sniff INFO Banner — Design

## Problem

Opening a file can override `expand_tab` / `tab_width` via content sniffing (after `.editorconfig`). Users with explicit config (e.g. tabs + width 4) get silent overrides (often 8-space expand). There is no dedicated chrome telling them that happened.

## Goal

When sniff **actually changes** one or more indent settings, show a temporary full-width INFO bar above the bottom chrome until the next keypress.

## Non-goals

- Changing sniff priority vs config / `.editorconfig`
- Persisting the notice across sessions
- Making the banner interactive (no Esc-to-dismiss mode)

## Behavior

### Trigger

In `Editor::open_file`, after `apply_editorconfig` and before applying sniffed values:

1. Snapshot current `expand_tab` and `tab_width`.
2. Apply sniffed `expand_tab` / `tab_width` when `Some`.
3. If either value differs from the snapshot, queue an INFO banner describing the **resulting** indent style.
4. If sniff returns values that match the snapshot (or returns `None` for both), do nothing.

Switching to an already-open buffer does not re-sniff and must not show the banner.

### Message

i18n `Message` variant (EN + SV), hybrid wording:

- EN: `INFO: Sniffer detected indent using {desc}, overriding default settings`
- `{desc}` from the post-override settings:
  - `expand_tab == false` → `tabs`
  - `expand_tab == true` → `{tab_width} spaces` (e.g. `4 spaces`, `8 spaces`)

`INFO:` is bold; the remainder uses normal toolbar foreground. Prefix with the same `▌` glyph and toolbar background as the status bar. Paint the entire line(s). Wrap to additional rows when the message does not fit one terminal width.

Swedish translation should mirror the same structure (bold `INFO:` / equivalent label + description of sniffed indent).

### Placement

Dedicated overlay window(s), not `status_msg` inside the status bar.

Bottom → top stack:

1. Status toolbar (always)
2. Help bar **or** mode prompt (existing mutual exclusion)
3. INFO banner (when visible)

The banner sits immediately above help when help is visible; otherwise immediately above the status toolbar. When a mode prompt is showing instead of help, the banner sits above that prompt **only if** it is allowed to paint (see deferral).

Include banner row count in `Viewport::overlay_rows` so text scroll/clamp keeps the cursor above covered rows.

### Lifecycle / dismiss

- Clear on next `Key` or `Paste` event, using the same mode exceptions as `status_msg` clear in `main.rs` (do not clear while in `Searching`, `ConfirmQuit`, `SaveAs`, or `Palette`).
- Opening another file that also overrides replaces any pending or visible banner.
- Banner does not block input; the key that dismisses it is still processed normally.

### Deferral with `RecoverSwap`

If opening the file sets `Mode::RecoverSwap`:

- Store the banner as **pending** (do not paint).
- When leaving `RecoverSwap` (yes / no / quit paths), promote pending → visible if still set.
- If the user never leaves recovery via a path that continues editing (e.g. quit), pending is irrelevant.

Do not show the banner while `RecoverSwap` is active.

## State

Add on `Editor` something equivalent to:

```text
info_banner: Option<InfoBanner>
```

Where `InfoBanner` holds enough to render (e.g. indent description params, or pre-built translated body + bold prefix), plus a `pending: bool` (or a separate `pending_info_banner`) so recovery can defer paint without losing the message.

Do **not** reuse `status_msg` for this chrome.

Helpers:

- `set_info_banner(...)` / `clear_info_banner()`
- Call site in `open_file` after sniff apply
- Promote-pending call site(s) on `RecoverSwap` exit
- Clear call site next to existing `clear_status()` in the event loop

## Rendering

New builder in `render/chrome.rs` (e.g. `build_info_banner`), composed from `render_ui`:

- Same `toolbar_bg`, `status_bg` (▌), `toolbar_fg` as status bar
- Bold only on the `INFO:` label fragment
- Multi-line via existing overlay/window row stacking (one `Window` per wrapped line, or one multi-row window — match local overlay patterns)
- `z_index` above help/prompt so it is not covered by them incorrectly; still below palette modal if both could theoretically coexist (palette open should follow existing clear rules; banner typically already dismissed)

Update `Viewport::from_editor` overlay accounting to add banner rows when the banner is visible (not when merely pending).

## i18n

- Add `Message` variant(s) for the full sentence and/or `{desc}` pieces (`tabs` / `N spaces`) so EN and SV stay consistent.
- Extend the chrome message exhaustiveness test list in `ui/i18n.rs`.

## Testing

- Unit: given config tabs/4, sniff spaces/8 → banner queued with `8 spaces`; sniff tabs with no width change → no banner if `expand_tab` already false.
- Unit: sniff matching config → no banner.
- Unit/render or state: `RecoverSwap` keeps banner pending; after mode returns to editing, banner becomes visible.
- Clear-on-key: after a key event in editing mode, banner is `None`.

## Out of scope follow-ups

- Showing previous vs new values in the message
- Config flag to disable sniff or the banner

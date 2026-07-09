# Sniff INFO Banner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When file-open indent sniffing actually overrides `expand_tab` and/or `tab_width`, show a temporary full-width INFO chrome bar above the bottom toolbar/help until the next keypress (deferred while `RecoverSwap` is active).

**Architecture:** Add `Editor.info_banner: Option<InfoBanner>` with a `pending` flag. Queue it from `open_file` only when sniffed values differ from the post-editorconfig snapshot. Render via a new `build_info_banner` overlay (toolbar styling, bold `INFO:`). Clear beside `clear_status` in the event loop; promote pending→visible when leaving `RecoverSwap`.

**Tech Stack:** Rust, existing `OverlayBuilder` / `UiFragment` chrome, `Message` i18n (EN + SV), `cargo test`.

## Global Constraints

- Show banner only when sniff **changes** at least one of `expand_tab` / `tab_width` (not when sniff agrees or returns `None`).
- Message shape EN: `INFO: Sniffer detected indent using {desc}, overriding default settings` where `{desc}` is `tabs` or `N spaces` from **resulting** settings.
- `INFO:` bold; rest normal toolbar fg; `▌` + full-width `toolbar_bg`; wrap to multiple rows if needed.
- Do not reuse `status_msg` for this chrome.
- Defer paint while `Mode::RecoverSwap`; promote on accept/decline (quit paths may leave pending uncleared — fine).
- Clear on Key/Paste with the same mode exceptions as `status_msg` (`Searching`, `ConfirmQuit`, `SaveAs`, `Palette`).
- Spec: `docs/superpowers/specs/2026-07-09-sniff-info-banner-design.md`.

---

## File map

| File | Responsibility |
|---|---|
| `src/editor/mod.rs` | `InfoBanner` type, `info_banner` field, helpers, `open_file` trigger, unit tests |
| `src/ui/i18n.rs` | `Message` variants + EN/SV strings + sample list |
| `src/editor/dispatch/file.rs` | Promote pending banner on recover accept/decline |
| `src/main.rs` | `clear_info_banner()` next to `clear_status()` |
| `src/render/chrome.rs` | `build_info_banner`, compose in `render_ui` |
| `src/render/mod.rs` | Count visible banner rows in `Viewport::overlay_rows` |

---

### Task 1: i18n messages for the sniff INFO banner

**Files:**
- Modify: `src/ui/i18n.rs`
- Test: `src/ui/i18n.rs` (`all_message_samples`, existing Swedish coverage test)

**Interfaces:**
- Consumes: existing `Message` / `Locale` pattern
- Produces:
  - `Message::InfoBannerLabel` → `"INFO:"` (EN and SV; keep `INFO:` as the bold label in both)
  - `Message::InfoBannerIndentTabs` → `"tabs"` / Swedish equivalent
  - `Message::InfoBannerIndentSpaces(usize)` → `"{n} spaces"` / Swedish equivalent
  - `Message::InfoBannerBody(String)` → `" Sniffer detected indent using {desc}, overriding default settings"` (leading space included so label+body concatenate cleanly)

- [ ] **Step 1: Write the failing assertion for new variants**

Add to `all_message_samples()` in `src/ui/i18n.rs`:

```rust
Message::InfoBannerLabel,
Message::InfoBannerIndentTabs,
Message::InfoBannerIndentSpaces(4),
Message::InfoBannerBody("tabs".into()),
```

Add a focused test in the existing `tests` module:

```rust
#[test]
fn sniff_info_banner_english_shape() {
	let en = EnglishLocale;
	assert_eq!(en.translate(Message::InfoBannerLabel), "INFO:");
	assert_eq!(en.translate(Message::InfoBannerIndentTabs), "tabs");
	assert_eq!(en.translate(Message::InfoBannerIndentSpaces(8)), "8 spaces");
	assert_eq!(
		en.translate(Message::InfoBannerBody("4 spaces".into())),
		" Sniffer detected indent using 4 spaces, overriding default settings"
	);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test sniff_info_banner_english_shape -- --nocapture`

Expected: FAIL (unknown variant / compile error)

- [ ] **Step 3: Add enum variants and EN/SV translations**

In `Message` enum (after `StatusMessage` is fine):

```rust
InfoBannerLabel,
InfoBannerIndentTabs,
InfoBannerIndentSpaces(usize),
InfoBannerBody(String),
```

English:

```rust
Message::InfoBannerLabel => "INFO:".to_string(),
Message::InfoBannerIndentTabs => "tabs".to_string(),
Message::InfoBannerIndentSpaces(n) => format!("{} spaces", n),
Message::InfoBannerBody(desc) => {
	format!(" Sniffer detected indent using {}, overriding default settings", desc)
}
```

Swedish (same `INFO:` label; natural SV body / desc):

```rust
Message::InfoBannerLabel => "INFO:".to_string(),
Message::InfoBannerIndentTabs => "tabbar".to_string(),
Message::InfoBannerIndentSpaces(n) => format!("{} mellanslag", n),
Message::InfoBannerBody(desc) => {
	format!(" Sniffer upptäckte indrag med {}, åsidosätter standardinställningar", desc)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test sniff_info_banner_english_shape swedish_covers_every_message_variant -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/ui/i18n.rs
git commit -m "$(cat <<'EOF'
Add i18n strings for sniff indent INFO banner.

EOF
)"
```

---

### Task 2: `InfoBanner` state + queue on sniff override

**Files:**
- Modify: `src/editor/mod.rs` (struct field, `Editor::new`, helpers, `open_file`, tests)
- Test: `src/editor/mod.rs` `mod tests`

**Interfaces:**
- Consumes: `Message::{InfoBannerIndentTabs, InfoBannerIndentSpaces}` from Task 1
- Produces:
  - `pub struct InfoBanner { pub expand_tab: bool, pub tab_width: usize, pub pending: bool }`
  - `Editor.info_banner: Option<InfoBanner>`
  - `Editor::set_info_banner(expand_tab: bool, tab_width: usize, pending: bool)`
  - `Editor::clear_info_banner()`
  - `Editor::promote_info_banner()` — if `Some` and `pending`, set `pending = false`
  - `Editor::info_banner_visible(&self) -> bool` — `matches!(self.info_banner, Some(b) if !b.pending)`
  - `open_file` queues banner when sniffed values change settings

- [ ] **Step 1: Write failing unit tests**

Add to `src/editor/mod.rs` `mod tests`:

```rust
#[test]
fn sniff_override_queues_info_banner_spaces() {
	let mut e = Editor::new();
	e.config.expand_tab = false;
	e.config.tab_width = 4;

	let mut tmp = std::env::temp_dir();
	tmp.push(format!("dan_sniff_spaces_{}.txt", std::process::id()));
	// Majority space indents of width 8 → sniff expand_tab=true, tab_width=8
	let body = "        a\n        b\n        c\n        d\n";
	std::fs::write(&tmp, body).unwrap();

	e.open_file(&tmp).unwrap();
	let _ = std::fs::remove_file(&tmp);

	assert!(e.config.expand_tab);
	assert_eq!(e.config.tab_width, 8);
	let b = e.info_banner.as_ref().expect("banner queued");
	assert!(b.expand_tab);
	assert_eq!(b.tab_width, 8);
	assert!(!b.pending); // no RecoverSwap
}

#[test]
fn sniff_matching_config_does_not_queue_banner() {
	let mut e = Editor::new();
	e.config.expand_tab = true;
	e.config.tab_width = 4;

	let mut tmp = std::env::temp_dir();
	tmp.push(format!("dan_sniff_match_{}.txt", std::process::id()));
	let body = "    a\n    b\n    c\n    d\n";
	std::fs::write(&tmp, body).unwrap();

	e.open_file(&tmp).unwrap();
	let _ = std::fs::remove_file(&tmp);

	assert!(e.info_banner.is_none());
}

#[test]
fn sniff_tabs_when_already_tabs_no_banner() {
	let mut e = Editor::new();
	e.config.expand_tab = false;
	e.config.tab_width = 4;

	let mut tmp = std::env::temp_dir();
	tmp.push(format!("dan_sniff_tabs_{}.txt", std::process::id()));
	let body = "\ta\n\tb\n\tc\n\td\n";
	std::fs::write(&tmp, body).unwrap();

	e.open_file(&tmp).unwrap();
	let _ = std::fs::remove_file(&tmp);

	assert!(!e.config.expand_tab);
	assert!(e.info_banner.is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test sniff_override_queues_info_banner_spaces sniff_matching_config_does_not_queue_banner sniff_tabs_when_already_tabs_no_banner -- --nocapture`

Expected: FAIL (missing `info_banner` field / compile error)

- [ ] **Step 3: Add type, field, helpers**

Near the top of `src/editor/mod.rs` (after imports / before `Editor`):

```rust
/// Transient chrome notice when indent sniffing overrides config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoBanner {
	pub expand_tab: bool,
	pub tab_width: usize,
	/// When true, do not paint (e.g. while `RecoverSwap` is active).
	pub pending: bool,
}
```

On `Editor`:

```rust
/// Sniff-override INFO bar (above help/toolbar); cleared on next key.
pub info_banner: Option<InfoBanner>,
```

In `Editor::new()` init: `info_banner: None,`

Helpers next to `set_status` / `clear_status`:

```rust
pub fn set_info_banner(&mut self, expand_tab: bool, tab_width: usize, pending: bool) {
	self.info_banner = Some(InfoBanner {
		expand_tab,
		tab_width,
		pending,
	});
}

pub fn clear_info_banner(&mut self) {
	self.info_banner = None;
}

pub fn promote_info_banner(&mut self) {
	if let Some(b) = self.info_banner.as_mut() {
		b.pending = false;
	}
}

pub fn info_banner_visible(&self) -> bool {
	matches!(self.info_banner, Some(InfoBanner { pending: false, .. }))
}
```

- [ ] **Step 4: Wire `open_file`**

Replace the sniff-apply block in `open_file` with:

```rust
let (mut buffer, sniffed_expand_tab, sniffed_tab_width) = Buffer::from_file(path)?;

self.config.apply_editorconfig(path);

let before_expand = self.config.expand_tab;
let before_width = self.config.tab_width;

if let Some(et) = sniffed_expand_tab {
	self.config.expand_tab = et;
}
if let Some(tw) = sniffed_tab_width {
	self.config.tab_width = tw;
}

let changed = self.config.expand_tab != before_expand
	|| self.config.tab_width != before_width;

let swp_path = crate::recovery::get_swap_path(path);
let recovering = crate::recovery::check_recovery(&swp_path).is_some();
if recovering {
	self.mode = Mode::RecoverSwap;
}
buffer.swp_path = Some(swp_path);

if changed {
	self.set_info_banner(
		self.config.expand_tab,
		self.config.tab_width,
		recovering, // pending while RecoverSwap
	);
}

// ... rest unchanged (maybe_dispose_startup_scratch, push buffer, etc.)
```

Remove the old separate `if check_recovery { self.mode = RecoverSwap }` so recovery is decided once via `recovering`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test sniff_override_queues_info_banner_spaces sniff_matching_config_does_not_queue_banner sniff_tabs_when_already_tabs_no_banner -- --nocapture`

Expected: PASS

Note: if a temp path unexpectedly has a stale `.swp`, `pending` may be true — tests use unique temp names under `std::env::temp_dir()`; if flaky, assert `e.info_banner.is_some()` and `expand_tab`/`tab_width` only, or delete any sibling `.swp` before open.

- [ ] **Step 6: Commit**

```bash
git add src/editor/mod.rs
git commit -m "$(cat <<'EOF'
Queue INFO banner when indent sniff overrides config.

EOF
)"
```

---

### Task 3: Promote on recover exit + clear on key

**Files:**
- Modify: `src/editor/dispatch/file.rs`
- Modify: `src/main.rs`
- Test: `src/editor/mod.rs` `mod tests`

**Interfaces:**
- Consumes: `promote_info_banner`, `clear_info_banner`, `set_info_banner` from Task 2
- Produces: recover accept/decline promote pending; event loop clears banner with status

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn recover_swap_defers_then_promotes_info_banner() {
	let mut e = Editor::new();
	e.set_info_banner(true, 8, true);
	assert!(!e.info_banner_visible());
	e.promote_info_banner();
	assert!(e.info_banner_visible());
}

#[test]
fn clear_info_banner_removes_it() {
	let mut e = Editor::new();
	e.set_info_banner(false, 4, false);
	e.clear_info_banner();
	assert!(e.info_banner.is_none());
}
```

- [ ] **Step 2: Run tests — helpers already exist from Task 2, so these should PASS once written; if `promote_info_banner` missing, implement as in Task 2**

Run: `cargo test recover_swap_defers_then_promotes_info_banner clear_info_banner_removes_it -- --nocapture`

Expected: PASS

- [ ] **Step 3: Promote in recover handlers**

In `src/editor/dispatch/file.rs`, at the end of both `cmd_recover_swap_accept` and `cmd_recover_swap_decline` (after `self.mode = Mode::Editing` and `clear_status`):

```rust
self.promote_info_banner();
```

Do **not** promote on `ForceQuitAll` (process is exiting).

- [ ] **Step 4: Clear beside status in the event loop**

In `src/main.rs`, in **both** places that call `editor.clear_status()` inside the Key/Paste guards (~lines 265 and 287), also call:

```rust
editor.clear_info_banner();
```

Keep the same mode exceptions (`Searching`, `ConfirmQuit`, `SaveAs`, `Palette`). Do **not** add `RecoverSwap` to the exception list — while recovering, keys that dismiss recovery go through commands; unrelated keys are mostly `Noop`, but if a Key event fires in `RecoverSwap` and would clear status today, matching that behavior for the banner is OK because the banner is still `pending` and `clear_info_banner` would drop it. To avoid losing a deferred banner on accidental key noise during recovery, **add** `Mode::RecoverSwap` to the clear-guard exceptions in both places:

```rust
&& editor.mode != crate::editor::mode::Mode::RecoverSwap
```

(applies to both `clear_status` and `clear_info_banner` in those blocks).

- [ ] **Step 5: Run a broader test pass**

Run: `cargo test recover_swap_defers_then_promotes_info_banner clear_info_banner_removes_it sniff_ -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/editor/dispatch/file.rs src/main.rs src/editor/mod.rs
git commit -m "$(cat <<'EOF'
Promote deferred sniff banner after recovery; clear on key.

EOF
)"
```

---

### Task 4: Render the INFO banner chrome

**Files:**
- Modify: `src/render/chrome.rs`
- Modify: `src/render/mod.rs`
- Test: `src/render/chrome.rs` (new `#[cfg(test)]` module) **or** `src/editor/mod.rs` if preferring state-only — prefer a small chrome unit test that builds windows

**Interfaces:**
- Consumes: `Editor::info_banner_visible`, `InfoBanner { expand_tab, tab_width }`, i18n messages from Task 1
- Produces: `build_info_banner(editor, width, base_y) -> Vec<Window>`; `render_ui` includes it; `Viewport::from_editor` adds visible banner row count to `overlay_rows`

- [ ] **Step 1: Write a failing render/unit test**

Add at the bottom of `src/render/chrome.rs`:

```rust
#[cfg(test)]
mod info_banner_tests {
	use super::*;
	use crate::editor::InfoBanner;
	use crate::editor::Editor;

	#[test]
	fn build_info_banner_none_when_pending_or_absent() {
		let mut e = Editor::new();
		assert!(build_info_banner(&e, 80, 10).is_empty());
		e.info_banner = Some(InfoBanner {
			expand_tab: true,
			tab_width: 4,
			pending: true,
		});
		assert!(build_info_banner(&e, 80, 10).is_empty());
	}

	#[test]
	fn build_info_banner_paints_full_width_row() {
		let mut e = Editor::new();
		e.info_banner = Some(InfoBanner {
			expand_tab: true,
			tab_width: 4,
			pending: false,
		});
		let wins = build_info_banner(&e, 80, 10);
		assert!(!wins.is_empty());
		assert_eq!(wins[0].rect.width, 80);
		// First row should include bold INFO: fragment
		let has_bold_info = wins.iter().any(|w| {
			w.fragments.iter().any(|f| f.is_bold && f.text.contains("INFO"))
		});
		assert!(has_bold_info);
	}
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test build_info_banner_ -- --nocapture`

Expected: FAIL (missing `build_info_banner`)

- [ ] **Step 3: Implement `build_info_banner`**

In `src/render/chrome.rs`:

```rust
pub fn build_info_banner(editor: &Editor, width: u16, base_y: u16) -> Vec<Window> {
	let Some(banner) = editor.info_banner.as_ref() else {
		return Vec::new();
	};
	if banner.pending {
		return Vec::new();
	}

	let desc = if banner.expand_tab {
		editor
			.locale
			.translate(Message::InfoBannerIndentSpaces(banner.tab_width))
	} else {
		editor
			.locale
			.translate(Message::InfoBannerIndentTabs)
	};
	let label = editor.locale.translate(Message::InfoBannerLabel);
	let body = editor
		.locale
		.translate(Message::InfoBannerBody(desc));
	let prefix_str = editor.locale.translate(Message::ToolbarPrefix);

	let mut builder = OverlayBuilder::new(editor.theme.toolbar_bg, 1)
		.with_prefix(UiFragment {
			text: prefix_str.clone(),
			fg: editor.theme.status_bg,
			bg: editor.theme.toolbar_bg,
			is_flex: false,
			is_bold: false,
		})
		.with_overflow_prefix(UiFragment {
			text: prefix_str,
			fg: editor.theme.status_bg,
			bg: editor.theme.toolbar_bg,
			is_flex: false,
			is_bold: false,
		});

	builder.add_block(OverlayBlock {
		fragments: vec![
			UiFragment {
				text: " ".to_string(),
				fg: editor.theme.toolbar_fg,
				bg: editor.theme.toolbar_bg,
				is_flex: false,
				is_bold: false,
			},
			UiFragment {
				text: label,
				fg: editor.theme.toolbar_fg,
				bg: editor.theme.toolbar_bg,
				is_flex: false,
				is_bold: true,
			},
			UiFragment {
				text: body,
				fg: editor.theme.toolbar_fg,
				bg: editor.theme.toolbar_bg,
				is_flex: false,
				is_bold: false,
			},
		],
	});

	builder.build(width, base_y)
}
```

- [ ] **Step 4: Compose in `render_ui` and account for overlay rows**

In `render_ui` (`src/render/chrome.rs`), after building help/prompt into `windows`, compute how many bottom overlay rows exist **excluding** the status bar, then place the info banner above them:

```rust
windows.push(build_status_bar(editor, vp));

let prompt = build_prompt(editor, vp.width, vp.height);
let mut bottom_overlay: u16 = 0;

if prompt.is_none() && editor.show_help && !editor.palette.open {
	let help = build_help_bar(editor, vp.width, vp.height);
	bottom_overlay = help.len() as u16;
	windows.extend(help);
}

if let Some(p) = prompt {
	bottom_overlay = p.len() as u16;
	windows.extend(p);
}

// Status occupies 1 row at the bottom; help/prompt sit on top of it.
// Info banner sits immediately above that stack.
if editor.info_banner_visible() {
	let base_y = vp.height.saturating_sub(1 + bottom_overlay + 1);
	windows.extend(build_info_banner(editor, vp.width, base_y));
}

if editor.palette.open {
	windows.extend(build_palette_window(editor, vp.width, vp.height));
}
```

(Replace the previous help/prompt extend block with the above so `bottom_overlay` is known.)

In `src/render/mod.rs` `Viewport::from_editor`, after computing help/prompt `overlay`, add visible banner rows:

```rust
if let Some(prompt_windows) = chrome::build_prompt(&*editor, w, h) {
	overlay += prompt_windows.len() as u16;
} else if editor.show_help && !editor.palette.open {
	overlay += chrome::build_help_bar(&*editor, w, h).len() as u16;
}
if editor.info_banner_visible() {
	// Match render_ui base_y math: banner may wrap; measure via builder.
	let base_y = h.saturating_sub(1 + overlay + 1);
	overlay += chrome::build_info_banner(&*editor, w, base_y).len() as u16;
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test build_info_banner_ sniff_ -- --nocapture`

Expected: PASS

Also: `cargo test`

Expected: all PASS

- [ ] **Step 6: Commit**

```bash
git add src/render/chrome.rs src/render/mod.rs
git commit -m "$(cat <<'EOF'
Render sniff INFO banner above help/toolbar chrome.

EOF
)"
```

---

### Task 5: Manual verification checklist (no commit required unless fixes)

**Files:** none required

- [ ] **Step 1: Build release/dev binary**

Run: `cargo build`

Expected: success

- [ ] **Step 2: Manual checks**

1. Open a space-indented file that differs from config (`expand_tab = false`, `tab_width = 4`) → INFO bar appears with `N spaces`.
2. Press any editing key → bar disappears; key still applies.
3. Open a tab-indented file with tabs already configured → no bar.
4. If possible, open a file with a recovery `.swp` that also sniffs differently → recovery prompt first, no INFO; after ^Y/^N → INFO appears; next key clears it.
5. With help legend visible (`^H`), confirm INFO sits above the help bar and paints full width.

- [ ] **Step 3: Fix any issues found; commit if needed**

```bash
git add -u
git commit -m "$(cat <<'EOF'
Fix sniff INFO banner issues found in manual check.

EOF
)"
```

---

## Spec coverage (self-review)

| Spec requirement | Task |
|---|---|
| Only when sniff changes settings | Task 2 |
| Hybrid message + tabs / N spaces | Tasks 1–2, 4 |
| Bold INFO: + toolbar style + full width + wrap | Task 4 (`OverlayBuilder`) |
| Above help or above toolbar | Task 4 |
| `overlay_rows` includes banner | Task 4 |
| Clear on key (status exceptions + RecoverSwap guard) | Task 3 |
| Defer during RecoverSwap; promote on exit | Tasks 2–3 |
| Dedicated state, not `status_msg` | Task 2 |
| i18n EN+SV + sample list | Task 1 |
| Unit tests for queue / match / defer / clear | Tasks 2–3 |

No placeholders left in steps. Type names consistent: `InfoBanner`, `set_info_banner`, `clear_info_banner`, `promote_info_banner`, `info_banner_visible`, `build_info_banner`.

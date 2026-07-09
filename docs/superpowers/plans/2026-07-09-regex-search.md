# Regex Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Optional regex search via `/pattern/` syntax, with capture-aware `$n`/`$name` replace, without changing default literal `Ctrl+F` behavior.

**Architecture:** Parse `/…/` in the editor search layer; extend `TextRope` with a regex find path that materializes the haystack once per refresh; cache a compiled `Regex` on `Editor` for the current query; expand replacements via `Captures` in step/all replace. Literal `find_all` stays streaming and case-insensitive.

**Tech Stack:** Rust, `regex` crate (direct dep, ~1.12), existing `TextRope` / `Editor::refresh_search_matches` / replace dispatch, `cargo test`.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-09-regex-search-design.md`
- Regex iff query length ≥ 3, starts and ends with `/`, non-empty interior
- Regex case-sensitive by default; `(?i)` / `(?m)` / `(?s)` inline only — no trailing flags
- Invalid regex: clear matches, show `invalid regex` in search chrome, no panic, no stale highlights
- Skip zero-width matches when collecting
- Literal replace: no `$` expansion; regex replace: Rust `regex` expansion (`$0`, `$1`, `$name`, `$$`)
- Replace-all: remaining matches from `search_match_idx`, end→start, expand per match
- Indent with tabs (project style)
- TDD: failing test first for each behavior

## File map

| File | Responsibility |
|------|----------------|
| `Cargo.toml` / `Cargo.lock` | Direct `regex` dependency |
| `src/editor/search.rs` | `parse_search_query`, refresh with mode/cache/error |
| `src/buffer/rope.rs` | `find_all_regex(&Regex)` + tests; keep literal `find_all` |
| `src/editor/mod.rs` | `search_is_regex`, `cached_regex`, `search_regex_error` fields + init/clear |
| `src/editor/dispatch/search.rs` | Clear regex state on cancel/confirm; keep promote gated on matches |
| `src/editor/dispatch/replace.rs` | Expand captures for yes/all when `search_is_regex` |
| `src/ui/i18n.rs` | `Message::InvalidRegex` (+ Swedish + samples) |
| `src/render/chrome.rs` | Searching chrome shows invalid vs zero matches |
| `README.md` | Document `/pattern/` + `$n` replace |
| `doc/2026-07-09-codebase-audit.md` | Check off regex search |

---

### Task 1: Add `regex` dependency

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock` (via cargo)

**Interfaces:**
- Produces: direct dependency `regex = "1"` (resolve to current 1.x, e.g. 1.12.x)

- [ ] **Step 1: Add dependency**

```bash
cd /Users/dfallman/dev/dan
cargo add regex@1
```

- [ ] **Step 2: Verify build**

Run: `cargo check`

Expected: SUCCESS (no code using it yet)

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "deps: add regex for optional search patterns"
```

---

### Task 2: Parse `/pattern/` query helper

**Files:**
- Modify: `src/editor/search.rs`

**Interfaces:**
- Produces:
  - `pub(crate) enum ParsedSearch<'a> { Literal(&'a str), RegexPattern(&'a str) }`
  - `pub(crate) fn parse_search_query(query: &str) -> ParsedSearch<'_>`

- [ ] **Step 1: Write the failing tests**

Append to `src/editor/search.rs` (add `#[cfg(test)] mod tests` if absent):

```rust
#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_literal_plain() {
		assert!(matches!(parse_search_query("foo"), ParsedSearch::Literal("foo")));
	}

	#[test]
	fn parse_regex_wrapped() {
		assert!(matches!(parse_search_query("/foo/"), ParsedSearch::RegexPattern("foo")));
		assert!(matches!(parse_search_query("/a|b/"), ParsedSearch::RegexPattern("a|b")));
	}

	#[test]
	fn parse_not_regex_edge_cases() {
		assert!(matches!(parse_search_query("//"), ParsedSearch::Literal("//")));
		assert!(matches!(parse_search_query("/"), ParsedSearch::Literal("/")));
		assert!(matches!(parse_search_query("/foo"), ParsedSearch::Literal("/foo")));
		assert!(matches!(parse_search_query("foo/"), ParsedSearch::Literal("foo/")));
		assert!(matches!(parse_search_query(""), ParsedSearch::Literal("")));
	}
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib parse_ -- --nocapture`

Expected: FAIL (`parse_search_query` / `ParsedSearch` not found)

- [ ] **Step 3: Implement parser**

At top of `src/editor/search.rs` (above `impl Editor`):

```rust
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ParsedSearch<'a> {
	Literal(&'a str),
	RegexPattern(&'a str),
}

/// `/pattern/` with non-empty interior → regex; otherwise literal (incl. `//`).
pub(crate) fn parse_search_query(query: &str) -> ParsedSearch<'_> {
	let bytes = query.as_bytes();
	if bytes.len() >= 3 && bytes[0] == b'/' && bytes[bytes.len() - 1] == b'/' {
		let interior = &query[1..query.len() - 1];
		if !interior.is_empty() {
			return ParsedSearch::RegexPattern(interior);
		}
	}
	ParsedSearch::Literal(query)
}
```

Note: slicing by byte indices is safe here because the delimiters are ASCII `/`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib parse_ -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/editor/search.rs
git commit -m "search: parse /pattern/ as regex query syntax"
```

---

### Task 3: `TextRope::find_all_regex`

**Files:**
- Modify: `src/buffer/rope.rs`

**Interfaces:**
- Consumes: `regex::Regex`
- Produces: `TextRope::find_all_regex(&self, re: &Regex) -> Vec<(usize, usize)>` (char offsets; skips zero-width)

- [ ] **Step 1: Write the failing tests**

In `src/buffer/rope.rs` `#[cfg(test)] mod tests`, add:

```rust
use regex::Regex;

#[test]
fn find_all_regex_basic() {
	let r = TextRope::from_str("foo bar foo");
	let re = Regex::new("foo").unwrap();
	assert_eq!(r.find_all_regex(&re), vec![(0, 3), (8, 11)]);
}

#[test]
fn find_all_regex_case_sensitive_by_default() {
	let r = TextRope::from_str("Foo foo");
	let re = Regex::new("foo").unwrap();
	assert_eq!(r.find_all_regex(&re), vec![(4, 7)]);
}

#[test]
fn find_all_regex_inline_case_insensitive() {
	let r = TextRope::from_str("Foo foo");
	let re = Regex::new("(?i)foo").unwrap();
	assert_eq!(r.find_all_regex(&re), vec![(0, 3), (4, 7)]);
}

#[test]
fn find_all_regex_non_ascii_char_spans() {
	let r = TextRope::from_str("weiß weiß");
	let re = Regex::new("ß").unwrap();
	let hits = r.find_all_regex(&re);
	assert_eq!(hits.len(), 2);
	assert_eq!(hits[0].1 - hits[0].0, 1);
}

#[test]
fn find_all_regex_skips_zero_width() {
	// `\b` alone can produce zero-width hits; also `a*` at every position.
	let r = TextRope::from_str("aa");
	let re = Regex::new("a*").unwrap();
	let hits = r.find_all_regex(&re);
	assert!(hits.iter().all(|&(s, e)| s < e), "no zero-width: {:?}", hits);
	assert_eq!(hits, vec![(0, 2)]);
}

#[test]
fn find_all_literal_still_case_insensitive() {
	let r = TextRope::from_str("Hello");
	assert_eq!(r.find_all("hello"), vec![(0, 5)]);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib find_all_regex_ -- --nocapture`

Expected: FAIL (`find_all_regex` not found)

- [ ] **Step 3: Implement `find_all_regex`**

In `impl TextRope` after `find_all`:

```rust
/// Regex search over a fully materialized UTF-8 haystack.
/// Returns char-offset `(start, end)` pairs. Zero-width matches are skipped
/// (advance one char past the empty match so collection cannot loop forever).
pub fn find_all_regex(&self, re: &regex::Regex) -> Vec<(usize, usize)> {
	let haystack = self.to_string_full();
	let mut results = Vec::new();
	let mut search_from = 0usize; // byte offset
	while search_from <= haystack.len() {
		let Some(m) = re.find_at(&haystack, search_from) else {
			break;
		};
		let byte_start = m.start();
		let byte_end = m.end();
		if byte_start == byte_end {
			// Skip zero-width: advance one char (or one byte at EOF).
			let advance = haystack[byte_start..]
				.chars()
				.next()
				.map(|c| c.len_utf8())
				.unwrap_or(1);
			search_from = byte_start.saturating_add(advance);
			if search_from == byte_start {
				break;
			}
			continue;
		}
		let start_char = haystack[..byte_start].chars().count();
		let end_char = start_char + haystack[byte_start..byte_end].chars().count();
		results.push((start_char, end_char));
		search_from = byte_end;
	}
	results
}
```

Keep existing `find_all` unchanged.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib find_all_ -- --nocapture`

Expected: PASS (literal + regex)

- [ ] **Step 5: Commit**

```bash
git add src/buffer/rope.rs
git commit -m "rope: add find_all_regex with zero-width skip"
```

---

### Task 4: Editor regex state + wire `refresh_search_matches`

**Files:**
- Modify: `src/editor/mod.rs`
- Modify: `src/editor/search.rs`
- Modify: `src/editor/dispatch/search.rs`

**Interfaces:**
- Produces on `Editor`:
  - `pub search_is_regex: bool`
  - `pub(crate) cached_regex: Option<regex::Regex>`
  - `pub search_regex_error: bool`
- Updates: `refresh_search_matches` parses query, compiles/caches, sets error flag, calls literal or regex find
- Clears regex state when search query is cleared (cancel/confirm paths)

- [ ] **Step 1: Write the failing integration tests**

Add to `src/editor/mod.rs` tests module (near other editor tests):

```rust
#[test]
fn refresh_search_regex_finds_matches() {
	let mut e = Editor::new();
	e.buffer_mut().text = crate::buffer::rope::TextRope::from_str("foo bar foo");
	e.search_query = "/foo/".into();
	e.refresh_search_matches();
	assert!(e.search_is_regex);
	assert!(!e.search_regex_error);
	assert_eq!(e.buffer().search_matches, vec![(0, 3), (8, 11)]);
}

#[test]
fn refresh_search_invalid_regex_clears_matches() {
	let mut e = Editor::new();
	e.buffer_mut().text = crate::buffer::rope::TextRope::from_str("foo");
	e.search_query = "/foo/".into();
	e.refresh_search_matches();
	assert_eq!(e.buffer().search_matches.len(), 1);

	e.search_query = "/foo(/".into();
	e.refresh_search_matches();
	assert!(e.search_is_regex);
	assert!(e.search_regex_error);
	assert!(e.buffer().search_matches.is_empty());
}

#[test]
fn refresh_search_literal_unchanged() {
	let mut e = Editor::new();
	e.buffer_mut().text = crate::buffer::rope::TextRope::from_str("Hello");
	e.search_query = "hello".into();
	e.refresh_search_matches();
	assert!(!e.search_is_regex);
	assert!(!e.search_regex_error);
	assert_eq!(e.buffer().search_matches, vec![(0, 5)]);
}
```

Make `refresh_search_matches` visible to tests: it is already `pub(crate)` on `Editor` in the same crate — OK for `#[cfg(test)]` in `mod.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib refresh_search_ -- --nocapture`

Expected: FAIL (missing fields / still always literal)

- [ ] **Step 3: Add fields to `Editor`**

In `src/editor/mod.rs` near `search_query`:

```rust
/// True when the current `search_query` is `/pattern/` regex mode.
pub search_is_regex: bool,
/// Compiled pattern for the current regex query; cleared when the query changes.
pub(crate) cached_regex: Option<regex::Regex>,
/// True when regex mode failed to compile; chrome shows "invalid regex".
pub search_regex_error: bool,
```

In `Editor::new` / constructor init: `search_is_regex: false`, `cached_regex: None`, `search_regex_error: false`.

Add a small helper on `Editor` in `search.rs`:

```rust
pub(crate) fn clear_search_regex_state(&mut self) {
	self.search_is_regex = false;
	self.cached_regex = None;
	self.search_regex_error = false;
}
```

- [ ] **Step 4: Rewrite `refresh_search_matches`**

Replace body in `src/editor/search.rs`:

```rust
pub(crate) fn refresh_search_matches(&mut self) {
	self.search_regex_error = false;
	let matches = match parse_search_query(&self.search_query) {
		ParsedSearch::Literal(needle) => {
			self.search_is_regex = false;
			self.cached_regex = None;
			self.buffer().text.find_all(needle)
		}
		ParsedSearch::RegexPattern(pattern) => {
			self.search_is_regex = true;
			let need_compile = self
				.cached_regex
				.as_ref()
				.map(|re| re.as_str() != pattern)
				.unwrap_or(true);
			if need_compile {
				match regex::Regex::new(pattern) {
					Ok(re) => self.cached_regex = Some(re),
					Err(_) => {
						self.cached_regex = None;
						self.search_regex_error = true;
						self.buffer_mut().search_matches.clear();
						self.buffer_mut().search_match_idx = 0;
						self.clear_status();
						return;
					}
				}
			}
			let re = self.cached_regex.as_ref().unwrap();
			self.buffer().text.find_all_regex(re)
		}
	};

	self.buffer_mut().search_matches = matches;
	if self.buffer().search_matches.is_empty() {
		self.clear_status();
		return;
	}
	let anchor_pos = if let Some((line, col)) = self.buffer().search_saved_cursor {
		self.buffer().text.line_to_char(line) + col
	} else {
		0
	};
	let idx = self
		.buffer()
		.search_matches
		.iter()
		.position(|&(start, _)| start >= anchor_pos)
		.unwrap_or(0);
	self.buffer_mut().search_match_idx = idx;
	self.jump_to_search_match();
}
```

- [ ] **Step 5: Clear regex state when leaving search**

In `src/editor/dispatch/search.rs`, after clearing `search_query` in `cmd_search_confirm` and `cmd_search_cancel`, call `self.clear_search_regex_state();`.

In `cmd_search_convert_to_replace`, do **not** clear `search_is_regex` / `cached_regex` — replace needs them.

In `src/editor/dispatch/replace.rs` `cmd_replace_cancel` and paths that clear `search_query` and exit to Editing (`cmd_replace_with_confirm` empty, `cmd_replace_action_yes` empty, `cmd_replace_action_all`), call `self.clear_search_regex_state();`.

- [ ] **Step 6: Run tests**

Run: `cargo test --lib refresh_search_ -- --nocapture`

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src/editor/mod.rs src/editor/search.rs src/editor/dispatch/search.rs src/editor/dispatch/replace.rs
git commit -m "search: wire /pattern/ regex refresh with compile cache"
```

---

### Task 5: Chrome + i18n for `invalid regex`

**Files:**
- Modify: `src/ui/i18n.rs`
- Modify: `src/render/chrome.rs`

**Interfaces:**
- Produces: `Message::InvalidRegex`
- Chrome `Mode::Searching`: when `search_regex_error`, show InvalidRegex instead of ZeroMatches

- [ ] **Step 1: Write the failing i18n coverage update**

Add `Message::InvalidRegex` to the enum and to `all_message_samples()` in `src/ui/i18n.rs`. Add English + Swedish arms:

```rust
// English
Message::InvalidRegex => "invalid regex".to_string(),

// Swedish
Message::InvalidRegex => "ogiltigt regex".to_string(),
```

Include `Message::InvalidRegex` in `all_message_samples()`.

- [ ] **Step 2: Run locale tests (expect fail until arms complete)**

Run: `cargo test --lib i18n -- --nocapture`  
(or the existing Swedish/exhaustive tests in `src/ui/i18n.rs`)

Expected: FAIL on non-exhaustive match until both locales handle the variant

- [ ] **Step 3: Wire chrome**

In `src/render/chrome.rs` `Mode::Searching` branch, replace the empty-matches non-empty-query arm:

```rust
if editor.buffer().search_matches.is_empty() {
	if editor.search_query.is_empty() {
		info_suffix = format!(" {} ", editor.locale.translate(Message::EscToClose));
	} else if editor.search_regex_error {
		info_prefix = editor.locale.translate(Message::InvalidRegex);
		info_suffix = format!(" {} ", editor.locale.translate(Message::EscToClose));
	} else {
		info_prefix = editor.locale.translate(Message::ZeroMatches);
		info_suffix = editor.locale.translate(Message::SearchShortcuts);
	}
} else {
	// unchanged MatchFraction + SearchReplaceShortcuts
}
```

- [ ] **Step 4: Optional unit test for chrome prefix**

If easy: construct `Editor`, set `search_query = "/(/"`, `search_regex_error = true`, call whatever builds the search overlay and assert the translated invalid string appears. Skip if overlay builders are hard to invoke — i18n + manual path is enough when refresh tests cover the flag.

- [ ] **Step 5: Run tests**

Run: `cargo test --lib -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/ui/i18n.rs src/render/chrome.rs
git commit -m "chrome: show invalid regex in search prompt"
```

---

### Task 6: Capture-aware replace

**Files:**
- Modify: `src/editor/dispatch/replace.rs`
- Modify: `src/editor/search.rs` (optional helper `expand_regex_replacement`)

**Interfaces:**
- Produces: `Editor::expand_replacement_for_match(&self, start: usize, end: usize) -> String`
  - If `!search_is_regex` or no cached regex → `replace_with.clone()`
  - Else re-match substring (or full haystack at char range) and `caps.expand(&replace_with, &mut out)`

- [ ] **Step 1: Write the failing tests**

In `src/editor/mod.rs` tests:

```rust
#[test]
fn regex_replace_yes_expands_capture() {
	let mut e = Editor::new();
	e.buffer_mut().text = crate::buffer::rope::TextRope::from_str("foo_bar");
	e.search_query = "/(foo)_(bar)/".into();
	e.refresh_search_matches();
	assert_eq!(e.buffer().search_matches, vec![(0, 7)]);
	e.replace_with = "$2-$1".into();
	e.mode = crate::editor::mode::Mode::ReplacingStep;
	e.cmd_replace_action_yes();
	assert_eq!(e.buffer().text.to_string_full(), "bar-foo");
}

#[test]
fn literal_replace_does_not_expand_dollar() {
	let mut e = Editor::new();
	e.buffer_mut().text = crate::buffer::rope::TextRope::from_str("foo");
	e.search_query = "foo".into();
	e.refresh_search_matches();
	e.replace_with = "$1".into();
	e.mode = crate::editor::mode::Mode::ReplacingStep;
	e.cmd_replace_action_yes();
	assert_eq!(e.buffer().text.to_string_full(), "$1");
}

#[test]
fn regex_replace_all_expands_each_match() {
	let mut e = Editor::new();
	e.buffer_mut().text = crate::buffer::rope::TextRope::from_str("a1 a2");
	e.search_query = "/a(\\d)/".into();
	e.refresh_search_matches();
	assert_eq!(e.buffer().search_matches.len(), 2);
	e.replace_with = "X$1".into();
	e.mode = crate::editor::mode::Mode::ReplacingStep;
	e.buffer_mut().search_match_idx = 0;
	e.cmd_replace_action_all();
	assert_eq!(e.buffer().text.to_string_full(), "X1 X2");
}

#[test]
fn regex_replace_named_group_and_dollar_escape() {
	let mut e = Editor::new();
	e.buffer_mut().text = crate::buffer::rope::TextRope::from_str("ab");
	e.search_query = "/(?P<x>a)(b)/".into();
	e.refresh_search_matches();
	e.replace_with = "$$-$x".into(); // `$$` → `$`, `$x` → named group
	e.mode = crate::editor::mode::Mode::ReplacingStep;
	e.cmd_replace_action_yes();
	assert_eq!(e.buffer().text.to_string_full(), "$-a");
}
```

`cmd_replace_action_*` are `pub(crate)` on `Editor` — callable from `src/editor/mod.rs` tests. If a test fails on named-group expand syntax, switch `$x` to `${x}` (both are valid in the `regex` crate).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib regex_replace_ -- --nocapture`

Expected: FAIL (literal `$2-$1` inserted, or methods private — fix visibility/`pub(crate)` as needed)

- [ ] **Step 3: Implement expansion helper**

In `src/editor/search.rs`:

```rust
impl Editor {
	/// Expand `replace_with` for a match char range. Literal mode returns
	/// `replace_with` unchanged (no `$` expansion).
	pub(crate) fn expand_replacement_for_match(&self, start: usize, end: usize) -> String {
		if !self.search_is_regex {
			return self.replace_with.clone();
		}
		let Some(re) = self.cached_regex.as_ref() else {
			return self.replace_with.clone();
		};
		let matched = self.buffer().text.slice_to_string(start..end);
		let Some(caps) = re.captures(&matched) else {
			return self.replace_with.clone();
		};
		let mut out = String::new();
		caps.expand(&self.replace_with, &mut out);
		out
	}
}
```

Note: expanding against the match substring works for patterns that do not need lookbehind outside the match. For v1 this matches the spec. If a pattern relies on context outside the span, full-haystack `find_at` can be used later — not required now.

- [ ] **Step 4: Use helper in replace dispatch**

In `cmd_replace_action_yes`:

```rust
let replacement = self.expand_replacement_for_match(start, end);
```

In `cmd_replace_action_all`:

```rust
for &(start, end) in pending_matches.iter().rev() {
	let replacement = self.expand_replacement_for_match(start, end);
	self.buffer_mut().delete_range(start, end);
	self.buffer_mut().insert_str(start, &replacement);
}
```

Ensure `clear_search_regex_state()` runs when leaving replace to Editing (Task 4).

- [ ] **Step 5: Run tests**

Run: `cargo test --lib regex_replace_ literal_replace_ -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/editor/search.rs src/editor/dispatch/replace.rs src/editor/mod.rs
git commit -m "replace: expand $captures for regex search sessions"
```

---

### Task 7: Docs + audit checkbox

**Files:**
- Modify: `README.md`
- Modify: `doc/2026-07-09-codebase-audit.md`

**Interfaces:**
- None (docs only)

- [ ] **Step 1: Update README feature blurb**

Change the Fuzzy Search bullet (~line 56) to mention regex:

```markdown
- **Fuzzy Search & Destructive Replace**: Instant buffer-wide searching with `Ctrl-F`, easily promoted to find-and-replace with `Ctrl-R`. Wrap the query in `/pattern/` for regex (case-sensitive; use `(?i)` for insensitive). Regex replace supports `$0`, `$1`, `$name`, and `$$`.
```

- [ ] **Step 2: Update Search & replace shortcuts section**

After the shortcuts table (~line 113), add a short note:

```markdown
Wrap the search query in `/pattern/` to use regular expressions (Rust `regex` syntax). Invalid patterns clear matches and show `invalid regex`. To search for the literal text `/foo/`, use an escaped regex such as `/\/foo\//`.
```

- [ ] **Step 3: Check off audit item**

In `doc/2026-07-09-codebase-audit.md`:

- Change the Regex search checkbox to `[x]` with a cleared note dated today.
- Add a Cleared table row:

```markdown
| 2026-07-09 | Regex search | `/pattern/` activation; capture replace; invalid clears matches. |
```

- Update the peer gap table Regex row for Dan to `Yes`.

- [ ] **Step 4: Commit**

```bash
git add README.md doc/2026-07-09-codebase-audit.md
git commit -m "docs: document regex search and mark audit item done"
```

---

### Task 8: Full verification

**Files:**
- None (verification only)

- [ ] **Step 1: Run full test suite**

Run: `cargo test`

Expected: PASS

- [ ] **Step 2: Manual smoke (optional but recommended)**

```bash
cargo run -- README.md
```

1. `Ctrl+F`, type `dan` → literal hits  
2. Clear, type `/[Dd]an/` → regex hits  
3. Type `/foo(/` → `invalid regex`, no highlights  
4. `/(\w+)-(\w+)/` on a buffer with `a-b`, `Ctrl+R`, replace `$2_$1`, `^Y` / `^A`

- [ ] **Step 3: No further commit unless smoke found fixes**

If fixes needed, commit them separately with a focused message.

---

## Self-review (plan vs spec)

| Spec requirement | Task |
|------------------|------|
| `/pattern/` activation | Task 2 |
| Case-sensitive regex + `(?i)` | Task 3–4 |
| Invalid → clear + chrome | Task 4–5 |
| Zero-width skip | Task 3 |
| `$n` / `$name` / `$$` replace | Task 6 |
| Literal no `$` expand | Task 6 |
| Replace-all end→start from idx | Task 6 (keeps existing loop) |
| Direct `regex` dep | Task 1 |
| README + audit | Task 7 |
| No trailing flags / no fancy-regex | Global constraints (not implemented) |
| Materialize only in regex mode | Task 3 |

No TBD placeholders. Types: `ParsedSearch`, `find_all_regex`, `search_is_regex` / `cached_regex` / `search_regex_error`, `expand_replacement_for_match`, `Message::InvalidRegex` — consistent across tasks.

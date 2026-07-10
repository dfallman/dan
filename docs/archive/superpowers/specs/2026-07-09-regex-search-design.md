> **ARCHIVED — DONE (shipped 2026-07-09).** Design for optional regex search.
> Live tracker: [`../../../BACKLOG.md`](../../../BACKLOG.md).

# Regex Search — Design

## Problem

Dan’s search is literal and always case-insensitive (`TextRope::find_all`). That is fine for logs, but code workflows need patterns (word boundaries, alternation, capture-based rewrite). The 2026-07-09 codebase audit lists optional regex search as a high-impact peer gap (Micro/Helix have it; Dan does not).

## Goal

Add optional regex search and capture-aware replace without changing the default literal `Ctrl+F` experience. Activation is syntactic: wrap the query in `/…/`.

## Non-goals (v1)

- Trailing flags (`/pattern/i`) — use inline `(?i)`, `(?m)`, `(?s)` instead
- Case-sensitivity toggle for literal search
- Streaming / non-materializing regex over the rope
- `fancy-regex` (lookaround / backrefs beyond the `regex` crate)
- Find-next / find-prev from `Editing` without reopening search
- Separate “Find (regex)” command or toggle keybinding
- Escape hatch to literally search for the text `/foo/` without using a regex

## Behavior

### Activation

A query is regex mode iff **all** of:

1. Length ≥ 3
2. Starts with `/`
3. Ends with `/`
4. Interior (between the slashes) is non-empty

Examples:

| Query | Mode | Pattern / needle |
|-------|------|------------------|
| `foo` | Literal | `foo` |
| `/foo/` | Regex | `foo` |
| `/a|b/` | Regex | `a\|b` |
| `//` | Literal | `//` (interior empty) |
| `/foo` | Literal | `/foo` (no closing `/`) |
| `foo/` | Literal | `foo/` |

No trailing flags. Inline flags in the pattern are allowed (`(?i)foo`, `(?m)^todo`, etc.).

Literal search for the characters `/foo/` is out of scope as a special case: use a regex such as `/\/foo\//`.

### Case sensitivity

| Mode | Default |
|------|---------|
| Literal | Case-insensitive (unchanged) |
| Regex | Case-sensitive (`regex` crate default); use `(?i)` for insensitive |

### Invalid patterns

On every search refresh, if the query is regex mode and `Regex::new` fails:

- Clear `search_matches`
- Show a short chrome/status error: `invalid regex`
- Do not panic
- Do not keep stale highlights from a previous valid pattern

While the user is mid-edit (e.g. `/foo(/`), matches stay empty until the pattern compiles.

### Matching semantics

- Match list remains `Vec<(usize, usize)>` of **char** offsets (same as today).
- Non-overlapping matches (engine advances past each match end).
- **Skip zero-width matches** (e.g. `/a*/` empty hits) so next/prev/replace cannot infinite-loop; advance the search cursor past an empty match when collecting.
- Wrap-around next/prev, match fraction `N/M`, confirm/cancel, and promote-to-replace (`Ctrl+R` only when matches exist) stay as today.
- Invalid regex never has matches → `Ctrl+R` stays a no-op naturally.

### Replace

Flow unchanged: `Searching` → `Ctrl+R` → `ReplacingWith` → `Enter` → `ReplacingStep` (`^Y` / `^N` / `^A`).

| Search mode at promote | Replacement |
|------------------------|-------------|
| Literal | Plain string; `$` is literal (no expansion) |
| Regex | Rust `regex` expansion: `$0`, `$1`…, `$name`, `$$` → `$` |

Missing groups follow the `regex` crate (typically empty string) — do not invent custom rules.

**Step yes (`^Y`):** expand `replace_with` for the current match via `Captures`, delete the match char range, insert the expanded string, refresh matches, advance (same undo grouping as today).

**Replace all (`^A`):** remaining matches from `search_match_idx` onward, processed **end → start** so earlier char offsets stay valid. Expand each match independently (do not use a single whole-buffer `replace_all` that would diverge from step semantics).

### Chrome / UX

- Prompt label remains `Search:` (optional small `.*` / regex hint when active — polish, not required).
- Invalid: show `invalid regex` instead of `N/M matches`; no highlights.
- Valid regex with hits: same `N/M matches` and key hints as literal.
- Help bar: no new shortcut (syntax-driven). Document in README.

### Persistence

`last_search_query` continues to store the raw query including surrounding `/…/` when regex was used, so the next `Ctrl+F` pre-fills the same pattern.

## Architecture

### Approach

Extend the existing find pipeline with a mode rather than a parallel scanner:

1. Parse query → `(SearchMode, needle_or_pattern)` in the editor/search layer.
2. `TextRope` gains a mode-aware find entry (extend `find_all` or a thin wrapper used only from `refresh_search_matches`).
3. Literal path: unchanged char-iterator, case-insensitive.
4. Regex path: compile once per query change; materialize rope → `String` once per refresh; collect char spans; skip zero-width.

### Dependencies

Add a **direct** `regex` dependency in `Cargo.toml` (already present transitively via syntect). Prefer the crate’s default features unless a smaller feature set is clearly sufficient.

### State

On `Editor` (or search helper state):

- `search_is_regex: bool` — true for the current query / replace session when mode is regex
- `cached_regex: Option<Regex>` — compiled pattern for the current query; cleared whenever `search_query` changes or search cancels

`Buffer.search_matches` / `search_match_idx` / `search_saved_cursor` / `last_search_query` unchanged in shape.

### Data flow

```
Ctrl+F → Searching
  keystroke updates search_query
       → parse /…/ ?
       → Literal: TextRope::find_all (existing)
       → Regex:   Regex::new → find on haystack String → char spans
       → invalid: clear matches + "invalid regex"
Ctrl+R (matches non-empty)
       → ReplacingWith → ReplacingStep
       → if search_is_regex: expand $… via Captures per match
       → else: insert replace_with literally
```

### Haystack cost

Regex mode materializes the full buffer to `String` on each refresh (every keystroke while searching). Literal mode stays streaming. Acceptable for v1; document as the cost of regex. No debounce in v1.

### Modules to touch

| Area | Path | Change |
|------|------|--------|
| Deps | `Cargo.toml` | Direct `regex` |
| Rope find | `src/buffer/rope.rs` | Mode-aware find; regex tests |
| Search refresh | `src/editor/search.rs` | Parse `/…/`, compile/cache, invalid path |
| Editor state | `src/editor/mod.rs` | `search_is_regex`, `cached_regex` |
| Replace | `src/editor/dispatch/replace.rs` | Capture expansion for step/all |
| Chrome | `src/render/chrome.rs` + `src/ui/i18n.rs` | `invalid regex` message |
| Docs | `README.md`, audit tracker | Document syntax + check off feature |

Input mapping and `Command` variants need no new keys for v1.

## Error handling

| Case | Behavior |
|------|----------|
| Invalid regex | Clear matches; chrome `invalid regex`; stay in `Searching` |
| Empty interior `//` | Literal search for `//` |
| Zero-width match | Skip when collecting |
| Missing capture in replace | Empty string (crate default) |
| Regex compile panic | Must not happen — always handle `Err` |

## Testing

- Parse helper: `/a/`, `/ab/`, `//`, `/`, `a`, `/a`, `a/` → mode classification
- Regex find: simple match, case-sensitive default, `(?i)` works, multi-match, non-ASCII char spans, zero-width skipped
- Invalid pattern: empty matches; no panic
- Replace expand: `$1`, `$0`, `$$`, named group; literal mode does **not** expand `$1`
- Replace-all from current index, end→start, with variable-length expansions
- Existing literal `find_all` tests remain green

## Documentation

README Features / shortcuts:

- `/pattern/` enables regex (case-sensitive; `(?i)` for insensitive)
- Replace supports `$0`, `$1`, `$name`, `$$`
- Literal `/foo/` via escaped regex e.g. `/\/foo\//`
- Invalid patterns clear matches and show an error

Check off **Regex search** in `doc/2026-07-09-codebase-audit.md` when shipped.

## Open follow-ups (not v1)

- Trailing flags `/pat/i`
- Literal case toggle
- Streaming regex / chunked search for huge files
- Optional chrome `.*` indicator
- `fancy-regex` if users need lookaround

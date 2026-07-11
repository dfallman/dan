//! Palette state machine: query, items, filter, selection, scroll.

use nucleo::Matcher;
use crate::palette::items::{ActionId, PaletteItem};
use crate::palette::match_::score;

/// One rendered row of the palette result list: a section header, or a result
/// item (carrying its index into `filtered`). Headers occupy a visible row but
/// are never selectable — selection always lands on an `Item`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteRow {
    /// Section header for group 0 = Buffers, 1 = Files, 2 = Commands.
    Section(u8),
    Item(usize),
}

/// Visual grouping used to place section dividers. Three groups, in display
/// order: buffers (incl. "New buffer"), recent/project files, then commands.
/// A divider is drawn wherever consecutive results change group.
fn group_of(item: &PaletteItem) -> u8 {
    match item {
        PaletteItem::Buffer { .. } => 0,
        PaletteItem::Action { id: ActionId::NewBuffer, .. } => 0,
        PaletteItem::File { .. } => 1,
        PaletteItem::Action { .. } => 2,
    }
}

#[allow(dead_code)]
pub struct PaletteState {
    pub open: bool,
    pub query: String,
    pub query_cursor: usize,
    pub all_items: Vec<PaletteItem>,
    /// (index into all_items, score). Sorted by score desc, kind, then original order.
    pub filtered: Vec<(usize, u32)>,
    pub selection: usize,
    pub scroll: usize,
    /// When Some(idx), the palette is showing the dirty-buffer save/discard prompt
    /// for the buffer at `idx` instead of the normal result list.
    pub close_prompt_idx: Option<usize>,
    /// Hidden until needed: nucleo allocates internal buffers; reuse across keystrokes.
    matcher: Matcher,
}

#[allow(dead_code)]
impl PaletteState {
    pub fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
            query_cursor: 0,
            all_items: Vec::new(),
            filtered: Vec::new(),
            selection: 0,
            scroll: 0,
            close_prompt_idx: None,
            matcher: Matcher::new(nucleo::Config::DEFAULT),
        }
    }

    /// Initialize for a freshly-opened palette. Caller provides the items
    /// (assembled in editor.open_palette() from buffers + recent + index + actions).
    pub fn open_with(&mut self, items: Vec<PaletteItem>) {
        self.open = true;
        self.query.clear();
        self.query_cursor = 0;
        self.all_items = items;
        self.selection = 0;
        self.scroll = 0;
        self.refilter();
    }

    pub fn close(&mut self) {
        self.open = false;
        self.all_items.clear();
        self.filtered.clear();
        self.query.clear();
        self.query_cursor = 0;
        self.selection = 0;
        self.scroll = 0;
        self.close_prompt_idx = None;
    }

    pub fn insert_char(&mut self, ch: char) {
        self.query.insert(self.query_cursor, ch);
        self.query_cursor += ch.len_utf8();
        self.refilter();
    }

    /// Insert a (already-sanitized) string at the query cursor.
    pub fn insert_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.query.insert_str(self.query_cursor, text);
        self.query_cursor += text.len();
        self.refilter();
    }

    pub fn delete_char(&mut self) {
        if self.query_cursor == 0 { return; }
        // Walk back one char boundary.
        let mut idx = self.query_cursor;
        while idx > 0 {
            idx -= 1;
            if self.query.is_char_boundary(idx) { break; }
        }
        self.query.replace_range(idx..self.query_cursor, "");
        self.query_cursor = idx;
        self.refilter();
    }

    pub fn move_down(&mut self, visible_rows: usize) {
        if self.filtered.is_empty() { return; }
        if self.selection + 1 < self.filtered.len() {
            self.selection += 1;
            // Scroll in visual-row space so divider rows are accounted for (1).
            self.ensure_selection_visible(visible_rows);
        }
    }

    pub fn move_up(&mut self) {
        if self.selection > 0 {
            self.selection -= 1;
            // Only the top bound can change when moving up.
            let vis = self.visual_index(self.selection);
            if vis < self.scroll {
                self.scroll = vis;
            }
        }
    }

    pub fn page_down(&mut self, visible_rows: usize) {
        for _ in 0..visible_rows { self.move_down(visible_rows); }
    }

    pub fn page_up(&mut self, visible_rows: usize) {
        for _ in 0..visible_rows { self.move_up(); }
    }

    pub fn selected_item(&self) -> Option<&PaletteItem> {
        self.filtered.get(self.selection).map(|&(i, _)| &self.all_items[i])
    }

    /// The full sequence of rendered rows (items interleaved with section
    /// dividers), in display order. Single source of truth shared by the
    /// renderer and by scroll math so divider rows can't desync the two.
    pub fn display_rows(&self) -> Vec<PaletteRow> {
        let mut rows = Vec::with_capacity(self.filtered.len() + 2);
        let mut prev_group: Option<u8> = None;
        for (vis_i, &(all_idx, _)) in self.filtered.iter().enumerate() {
            let g = group_of(&self.all_items[all_idx]);
            if prev_group.is_some_and(|p| p != g) {
                rows.push(PaletteRow::Section(g));
            }
            rows.push(PaletteRow::Item(vis_i));
            prev_group = Some(g);
        }
        rows
    }

    /// Visual row index (position in `display_rows`) of the item at
    /// `filtered_idx`, i.e. its `filtered` index plus the dividers above it.
    pub fn visual_index(&self, filtered_idx: usize) -> usize {
        let mut dividers = 0;
        let last = filtered_idx.min(self.filtered.len().saturating_sub(1));
        for i in 1..=last {
            let g = group_of(&self.all_items[self.filtered[i].0]);
            let pg = group_of(&self.all_items[self.filtered[i - 1].0]);
            if g != pg {
                dividers += 1;
            }
        }
        filtered_idx + dividers
    }

    /// Adjust `scroll` (a visual-row offset) so the selected item's visual row
    /// stays within the visible window.
    fn ensure_selection_visible(&mut self, visible_rows: usize) {
        let visible_rows = visible_rows.max(1);
        let vis = self.visual_index(self.selection);
        if vis < self.scroll {
            self.scroll = vis;
        } else if vis >= self.scroll + visible_rows {
            self.scroll = vis + 1 - visible_rows;
        }
    }

    pub fn refilter(&mut self) {
        self.filtered.clear();
        if self.query.is_empty() {
            // Empty query: include everything, in original (kind+recency) order.
            for (i, _item) in self.all_items.iter().enumerate() {
                self.filtered.push((i, 0));
            }
        } else {
            for (i, item) in self.all_items.iter().enumerate() {
                let s = score(&mut self.matcher, &self.query, item.search_text());
                if s > 0 {
                    self.filtered.push((i, s));
                }
            }
            // Four sections, in display order:
            //   0: Buffer items (top)
            //   1: NewBuffer action (sits at the bottom of the buffer list)
            //   2: recent / project Files
            //   3: every other command (Action)
            // Files and commands are kept in separate sections (not interleaved
            // by score) so the recent-files | commands divider reads cleanly
            // even while filtering (2). Within each section: score DESC, then
            // kind_rank ASC, then index.
            let all = &self.all_items;
            let section = |idx: usize| -> u8 {
                use crate::palette::{PaletteItem, ActionId};
                match &all[idx] {
                    PaletteItem::Buffer { .. } => 0,
                    PaletteItem::Action { id: ActionId::NewBuffer, .. } => 1,
                    PaletteItem::File { .. } => 2,
                    PaletteItem::Action { .. } => 3,
                }
            };
            self.filtered.sort_by(|a, b| {
                section(a.0).cmp(&section(b.0))
                    .then_with(|| b.1.cmp(&a.1))
                    .then_with(|| all[a.0].kind_rank().cmp(&all[b.0].kind_rank()))
                    .then_with(|| a.0.cmp(&b.0))
            });
        }
        self.selection = 0;
        self.scroll = 0;
    }
}

impl Default for PaletteState {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::items::{PaletteItem, ActionId};

    fn action(label: &str) -> PaletteItem {
        PaletteItem::Action { id: ActionId::Save, label: label.into(), hint: None }
    }

    fn buffer(name: &str) -> PaletteItem {
        PaletteItem::Buffer { idx: 0, dirty: false, path_display: name.into(), is_current: true }
    }

    fn file(name: &str) -> PaletteItem {
        PaletteItem::File { path: name.into(), display: name.into(), last_opened: None }
    }

    #[test]
    fn display_rows_inserts_section_headers_between_groups() {
        // buffers | files | commands — a labeled header at each boundary.
        let mut p = PaletteState::new();
        p.open_with(vec![buffer("b"), file("f"), action("cmd")]);
        let rows = p.display_rows();
        assert_eq!(
            rows,
            vec![
                PaletteRow::Item(0),
                PaletteRow::Section(1),
                PaletteRow::Item(1),
                PaletteRow::Section(2),
                PaletteRow::Item(2),
            ]
        );
    }

    #[test]
    fn display_rows_single_section_when_files_absent() {
        // Buffer then commands, no files → one "Commands" header.
        let mut p = PaletteState::new();
        p.open_with(vec![buffer("b"), action("c1"), action("c2")]);
        let rows = p.display_rows();
        assert_eq!(
            rows,
            vec![
                PaletteRow::Item(0),
                PaletteRow::Section(2),
                PaletteRow::Item(1),
                PaletteRow::Item(2),
            ]
        );
    }

    #[test]
    fn visual_index_accounts_for_sections_above() {
        let mut p = PaletteState::new();
        p.open_with(vec![buffer("b"), file("f"), action("cmd")]);
        assert_eq!(p.visual_index(0), 0); // buffer
        assert_eq!(p.visual_index(1), 2); // file, after 1 section header
        assert_eq!(p.visual_index(2), 4); // command, after 2 section headers
    }

    #[test]
    fn scroll_keeps_selection_visible_with_sections() {
        // A section header consumes a visible row, so scrolling must track the
        // selected item's *visual* row, not its item index.
        let mut p = PaletteState::new();
        let mut items = vec![buffer("b")];
        for i in 0..5 {
            items.push(action(&format!("a{i}")));
        }
        p.open_with(items); // groups: [buffer, cmd, cmd, cmd, cmd, cmd]
        let visible_rows = 4;
        for _ in 0..10 {
            p.move_down(visible_rows);
        }
        assert_eq!(p.selection, 5, "should land on the last item");

        // The selected item's true visual row (from display_rows) must be in
        // the window [scroll, scroll + visible_rows).
        let rows = p.display_rows();
        let true_vis = rows
            .iter()
            .position(|r| matches!(r, PaletteRow::Item(i) if *i == p.selection))
            .expect("selection must map to a visual row");
        assert!(
            p.scroll <= true_vis && true_vis < p.scroll + visible_rows,
            "selected visual row {} not in window [{}, {})",
            true_vis,
            p.scroll,
            p.scroll + visible_rows
        );
    }

    #[test]
    fn filtered_keeps_files_before_commands() {
        // (2): the divider only reads cleanly if files always precede commands,
        // even when a query interleaves them by score.
        let mut p = PaletteState::new();
        p.open_with(vec![action("save"), file("save.txt")]);
        p.insert_char('s');
        p.insert_char('a');
        let order: Vec<u8> = p.filtered.iter()
            .map(|&(i, _)| match &p.all_items[i] {
                PaletteItem::File { .. } => 1,
                _ => 2,
            })
            .collect();
        assert_eq!(order, vec![1, 2], "files must sort before commands");
    }

    #[test]
    fn empty_query_includes_all_items() {
        let mut p = PaletteState::new();
        p.open_with(vec![action("foo"), action("bar"), action("baz")]);
        assert_eq!(p.filtered.len(), 3);
    }

    #[test]
    fn typing_filters_to_matching_items() {
        let mut p = PaletteState::new();
        p.open_with(vec![action("save"), action("load"), action("salvage")]);
        p.insert_char('s');
        p.insert_char('a');
        // "save" and "salvage" should both match; "load" should not.
        assert_eq!(p.filtered.len(), 2);
    }

    #[test]
    fn move_down_clamps_at_end() {
        let mut p = PaletteState::new();
        p.open_with(vec![action("a"), action("b")]);
        p.move_down(10); p.move_down(10); p.move_down(10);
        assert_eq!(p.selection, 1);
    }

    #[test]
    fn move_up_clamps_at_top() {
        let mut p = PaletteState::new();
        p.open_with(vec![action("a"), action("b")]);
        p.move_up();
        assert_eq!(p.selection, 0);
    }

    #[test]
    fn delete_char_handles_multibyte() {
        let mut p = PaletteState::new();
        p.open_with(vec![action("anything")]);
        p.insert_char('é');
        assert_eq!(p.query, "é");
        assert_eq!(p.query_cursor, 2); // 'é' is 2 bytes
        p.delete_char();
        assert_eq!(p.query, "");
        assert_eq!(p.query_cursor, 0);
    }

    #[test]
    fn close_resets_state() {
        let mut p = PaletteState::new();
        p.open_with(vec![action("a")]);
        p.insert_char('a');
        p.close();
        assert!(!p.open);
        assert_eq!(p.query, "");
        assert!(p.all_items.is_empty());
    }

    #[test]
    fn refilter_resets_selection_to_zero() {
        let mut p = PaletteState::new();
        p.open_with(vec![action("aaa"), action("bbb"), action("ccc")]);
        p.move_down(10); p.move_down(10);
        assert_eq!(p.selection, 2);
        p.insert_char('a');
        assert_eq!(p.selection, 0);
    }
}

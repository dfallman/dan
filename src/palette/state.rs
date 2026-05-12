//! Palette state machine: query, items, filter, selection, scroll.

use nucleo::Matcher;
use crate::palette::items::PaletteItem;
use crate::palette::match_::score;

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
            if self.selection >= self.scroll + visible_rows {
                self.scroll = self.selection + 1 - visible_rows;
            }
        }
    }

    pub fn move_up(&mut self) {
        if self.selection > 0 {
            self.selection -= 1;
            if self.selection < self.scroll {
                self.scroll = self.selection;
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
            // Three sections:
            //   0: Buffer items (top)
            //   1: NewBuffer action (sits at the bottom of the buffer list)
            //   2: everything else
            // Within each section: score DESC, then kind_rank ASC, then index.
            let all = &self.all_items;
            let section = |idx: usize| -> u8 {
                use crate::palette::{PaletteItem, ActionId};
                match &all[idx] {
                    PaletteItem::Buffer { .. } => 0,
                    PaletteItem::Action { id: ActionId::NewBuffer, .. } => 1,
                    _ => 2,
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

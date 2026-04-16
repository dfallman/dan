use crate::ui::layout::{Gravity, Rect, UiFragment, Window};
use crossterm::style::Color;

pub struct OverlayBlock {
    pub fragments: Vec<UiFragment>,
}

pub struct OverlayBuilder {
    pub blocks: Vec<OverlayBlock>,
    pub prefix: Option<UiFragment>,
    pub overflow_prefix: Option<UiFragment>,
    pub flex_bg: Color,
    pub z_index: u8,
    pub cursor_offset: Option<usize>,
    pub trailing: Option<UiFragment>,
}

impl OverlayBuilder {
    pub fn new(flex_bg: Color, z_index: u8) -> Self {
        Self {
            blocks: Vec::new(),
            prefix: None,
            overflow_prefix: None,
            flex_bg,
            z_index,
            cursor_offset: None,
            trailing: None,
        }
    }

    pub fn with_prefix(mut self, prefix: UiFragment) -> Self {
        self.prefix = Some(prefix);
        self
    }

    pub fn with_overflow_prefix(mut self, prefix: UiFragment) -> Self {
        self.overflow_prefix = Some(prefix);
        self
    }

    pub fn with_trailing(mut self, trailing: UiFragment) -> Self {
        self.trailing = Some(trailing);
        self
    }

    pub fn with_cursor(mut self, offset: usize) -> Self {
        self.cursor_offset = Some(offset);
        self
    }

    pub fn add_block(&mut self, block: OverlayBlock) {
        self.blocks.push(block);
    }

    pub fn build(mut self, width: u16, base_y: u16) -> Vec<Window> {
        let blocks = std::mem::take(&mut self.blocks);
        let mut windows = Vec::new();
        let prefix_width = self.prefix.as_ref().map_or(0, |p| p.text.chars().count() as u16);
        let overflow_prefix_width = self.overflow_prefix.as_ref().map_or(prefix_width, |p| p.text.chars().count() as u16);
        let eff_width = width;
        let mut cursor_window_idx: Option<usize> = None;
        let mut cursor_x_final: Option<u16> = None;

        let mut current_row_fragments = Vec::new();
        let mut current_x = prefix_width;
        let mut string_idx = 0; // Maps text dimensions independently of prefix

        for block in blocks {
            let block_chars: usize = block.fragments.iter().map(|f| f.text.chars().count()).sum();
            
            if !current_row_fragments.is_empty() && current_x + (block_chars as u16) > eff_width {
                windows.push(self.emit_row(current_row_fragments, width, 0, Some(self.flex_bg), windows.is_empty()));
                current_row_fragments = Vec::new();
                current_x = overflow_prefix_width;
            }

            for frag in block.fragments {
                let text_chars = frag.text.chars().count();
                
                if current_x + (text_chars as u16) > eff_width && !frag.is_flex {
                    let mut text_rem: Vec<char> = frag.text.chars().collect();
                    
                    while !text_rem.is_empty() {
                        let space_left = (eff_width.saturating_sub(current_x)) as usize;
                        if space_left == 0 {
                            windows.push(self.emit_row(current_row_fragments, width, 0, Some(self.flex_bg), windows.is_empty()));
                            current_row_fragments = Vec::new();
                            current_x = overflow_prefix_width;
                            continue;
                        }

                        let take_len = text_rem.len().min(space_left);
                        let take_str: String = text_rem.iter().take(take_len).collect();
                        text_rem.drain(0..take_len);

                        if let Some(co) = self.cursor_offset {
                            if co >= string_idx && co <= string_idx + take_len {
                                cursor_window_idx = Some(windows.len());
                                cursor_x_final = Some(current_x + (co - string_idx) as u16);
                            }
                        }

                        current_row_fragments.push(UiFragment {
                            text: take_str,
                            fg: frag.fg,
                            bg: frag.bg,
                            is_flex: false,
                        });
                        
                        current_x += take_len as u16;
                        string_idx += take_len;
                    }
                } else {
                    if let Some(co) = self.cursor_offset {
                        if co >= string_idx && co <= string_idx + text_chars {
                            cursor_window_idx = Some(windows.len());
                            cursor_x_final = Some(current_x + (co - string_idx) as u16);
                        }
                    }

                    current_row_fragments.push(frag);
                    current_x += text_chars as u16;
                    string_idx += text_chars;
                }
            }
        }

        if let Some(t) = self.trailing.take() {
            let t_len = t.text.chars().count() as u16;
            if current_x + t_len > eff_width {
                windows.push(self.emit_row(current_row_fragments, width, 0, None, windows.is_empty()));
                current_row_fragments = Vec::new();
            }
            current_row_fragments.push(UiFragment { text: String::new(), fg: self.flex_bg, bg: self.flex_bg, is_flex: true });
            current_row_fragments.push(t);
            windows.push(self.emit_row(current_row_fragments, width, 0, None, windows.is_empty()));
        } else if !current_row_fragments.is_empty() || windows.is_empty() {
            windows.push(self.emit_row(current_row_fragments, width, 0, Some(self.flex_bg), windows.is_empty()));
        }

        let total_rows = windows.len() as u16;
        for (i, win) in windows.iter_mut().enumerate() {
            let row_idx = i as u16; 
            win.rect.y = base_y.saturating_sub(total_rows.saturating_sub(1).saturating_sub(row_idx));

            if cursor_window_idx == Some(i) {
                if let Some(cx) = cursor_x_final {
                    win.cursor_bounds = Some((cx, 0));
                }
            }
        }
        windows
    }

    fn emit_row(&self, fragments: Vec<UiFragment>, width: u16, y: u16, trailing_flex: Option<Color>, is_first_row: bool) -> Window {
        let mut final_frags = Vec::new();
        if is_first_row {
            if let Some(p) = &self.prefix {
                final_frags.push(p.clone());
            }
        } else {
            if let Some(p) = &self.overflow_prefix {
                final_frags.push(p.clone());
            } else if let Some(p) = &self.prefix {
                final_frags.push(p.clone());
            }
        }
        final_frags.extend(fragments);
        
        if let Some(bg) = trailing_flex {
            final_frags.push(UiFragment {
                text: String::new(),
                fg: bg,
                bg: bg,
                is_flex: true,
            });
        }

        Window {
            rect: Rect { x: 0, y, width, height: 1 },
            gravity: Gravity::BottomLeft,
            z_index: self.z_index,
            cursor_bounds: None, 
            fragments: final_frags,
        }
    }
}

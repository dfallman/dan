use crossterm::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gravity {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
    Fill,
}

#[derive(Debug, Clone)]
pub struct UiFragment {
    pub text: String,
    pub fg: Color,
    pub bg: Color,
    pub is_flex: bool,
}

#[derive(Debug, Clone)]
pub struct Window {
    pub rect: Rect,
    pub gravity: Gravity,
    pub z_index: u8,
    pub cursor_bounds: Option<(u16, u16)>,
    pub fragments: Vec<UiFragment>,
}

pub fn calculate_viewport(
    _len: usize, 
    cursor: usize, 
    width: usize, 
    view_start: &mut usize
) -> usize {
    if cursor < *view_start {
        *view_start = cursor;
    } else if cursor >= *view_start + width {
        *view_start = cursor.saturating_sub(width).saturating_add(1);
    }
    cursor.saturating_sub(*view_start)
}

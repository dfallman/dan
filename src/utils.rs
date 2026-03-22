/// Utility functions for the editor.

/// Approximate display width of a character.
/// Uses a simple heuristic — ASCII is 1, tab is configurable,
/// wide CJK characters are 2. (Simplified version without unicode-width crate.)
pub fn char_width(ch: char, tab_width: usize) -> usize {
    match ch {
        '\t' => tab_width,
        '\n' | '\r' => 0,
        // CJK Unified Ideographs and other wide blocks
        c if is_wide(c) => 2,
        _ => 1,
    }
}

/// Simple heuristic for wide characters (CJK ranges).
fn is_wide(c: char) -> bool {
    let cp = c as u32;
    // CJK Unified Ideographs
    (0x4E00..=0x9FFF).contains(&cp)
        // CJK Compatibility Ideographs
        || (0xF900..=0xFAFF).contains(&cp)
        // CJK Unified Ideographs Extension A
        || (0x3400..=0x4DBF).contains(&cp)
        // Halfwidth/Fullwidth Forms
        || (0xFF01..=0xFF60).contains(&cp)
        || (0xFFE0..=0xFFE6).contains(&cp)
        // Hangul Syllables
        || (0xAC00..=0xD7AF).contains(&cp)
}

/// Clamp a value to a range.
pub fn clamp(val: usize, min: usize, max: usize) -> usize {
    if val < min {
        min
    } else if val > max {
        max
    } else {
        val
    }
}

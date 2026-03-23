/// Utility functions for the editor.
use unicode_width::UnicodeWidthChar;

/// Display width of a character in terminal columns.
///
/// Uses the `unicode-width` crate for correct handling of:
/// - CJK Unified Ideographs (all extensions)
/// - Fullwidth / Halfwidth Forms
/// - Hangul Syllables
/// - Emoji with `Emoji_Presentation`
/// - Zero-width joiners, combining marks (width 0)
/// - Control characters (width 0)
pub fn char_width(ch: char, tab_width: usize) -> usize {
    match ch {
        '\t' => tab_width,
        '\n' | '\r' => 0,
        c => UnicodeWidthChar::width(c).unwrap_or(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_width() {
        assert_eq!(char_width('a', 4), 1);
        assert_eq!(char_width('Z', 4), 1);
        assert_eq!(char_width('0', 4), 1);
        assert_eq!(char_width(' ', 4), 1);
        assert_eq!(char_width('!', 4), 1);
    }

    #[test]
    fn tab_uses_configured_width() {
        assert_eq!(char_width('\t', 4), 4);
        assert_eq!(char_width('\t', 8), 8);
        assert_eq!(char_width('\t', 2), 2);
    }

    #[test]
    fn newline_cr_zero() {
        assert_eq!(char_width('\n', 4), 0);
        assert_eq!(char_width('\r', 4), 0);
    }

    #[test]
    fn cjk_ideographs_are_wide() {
        assert_eq!(char_width('中', 4), 2);
        assert_eq!(char_width('文', 4), 2);
        assert_eq!(char_width('字', 4), 2);
    }

    #[test]
    fn hangul_is_wide() {
        assert_eq!(char_width('한', 4), 2);
        assert_eq!(char_width('글', 4), 2);
    }

    #[test]
    fn fullwidth_forms_are_wide() {
        // Fullwidth Latin 'Ａ' (U+FF21)
        assert_eq!(char_width('Ａ', 4), 2);
    }

    #[test]
    fn combining_marks_zero_width() {
        // U+0301 COMBINING ACUTE ACCENT
        assert_eq!(char_width('\u{0301}', 4), 0);
    }

    #[test]
    fn control_chars_zero_width() {
        // Null, BEL, ESC
        assert_eq!(char_width('\0', 4), 0);
        assert_eq!(char_width('\x07', 4), 0);
        assert_eq!(char_width('\x1B', 4), 0);
    }
}

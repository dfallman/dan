// SPDX-License-Identifier: GPL-3.0-or-later

//! Sanitization for terminal-injection vectors. The renderer calls
//! `sanitize_char` on every cell so file/buffer content can never emit a
//! raw escape sequence to the terminal; the paste path calls `sanitize_paste`
//! at storage entry to strip the same vectors from clipboard content.

/// Check if a character is a control character that should be sanitized.
///
/// Control characters (except tab, newline, carriage return) are replaced
/// with visible glyphs to prevent terminal escape injection. This also
/// covers Unicode bidirectional formatting characters that enable the
/// "Trojan Source" attack (CVE-2021-42574): visually reordering source
/// content so what the user sees does not match what the file contains.
#[inline]
pub fn is_control_char(ch: char) -> bool {
    let cp = ch as u32;
    // ASCII C0 (0x00-0x1F) — except tab, newline, CR.
    if cp < 0x20 {
        return ch != '\t' && ch != '\n' && ch != '\r';
    }
    // DEL.
    if cp == 0x7F {
        return true;
    }
    // ASCII C1 (0x80-0x9F).
    if (0x80..=0x9F).contains(&cp) {
        return true;
    }
    // Unicode bidi formatting characters — Trojan Source attack vector.
    // Embedding/override pair: LRE, RLE, PDF, LRO, RLO.
    if (0x202A..=0x202E).contains(&cp) {
        return true;
    }
    // Isolate pair: LRI, RLI, FSI, PDI.
    if (0x2066..=0x2069).contains(&cp) {
        return true;
    }
    false
}

/// Sanitize a single character for safe terminal display.
///
/// Returns `(display_char, is_sanitized)`. The renderer styles sanitized
/// cells distinctly (magenta + bold) so they stand out from any literal
/// instance of the substitute glyph the user might have typed.
///
/// C0 controls (U+0000..U+001F, except tab/newline/CR which pass through)
/// map 1:1 onto the Unicode "Control Pictures" block U+2400..U+241F so
/// each substitution remains a single column wide. DEL maps to U+2421
/// (SYMBOL FOR DELETE). C1 controls and Unicode bidi formatting map to
/// the middle-dot `·` — there is no symmetric Unicode block for them.
#[inline]
pub fn sanitize_char(ch: char) -> (char, bool) {
    if !is_control_char(ch) {
        return (ch, false);
    }
    let cp = ch as u32;
    let glyph = if cp < 0x20 {
        // U+2400 + cp mirrors U+0000..U+001F as printable pictures.
        char::from_u32(0x2400 + cp).unwrap_or('·')
    } else if cp == 0x7F {
        '\u{2421}' // SYMBOL FOR DELETE
    } else {
        '·'
    };
    (glyph, true)
}

/// Sanitize a string for safe terminal display.
///
/// Replaces control characters with visible glyphs to prevent terminal
/// escape injection attacks. Returns the sanitized string and count of
/// replaced characters.
pub fn sanitize_str(s: &str) -> (String, usize) {
    let mut result = String::with_capacity(s.len());
    let mut sanitized_count = 0;
    
    for ch in s.chars() {
        let (sanitized_ch, was_sanitized) = sanitize_char(ch);
        result.push(sanitized_ch);
        if was_sanitized {
            sanitized_count += 1;
        }
    }
    
    (result, sanitized_count)
}

/// Sanitize pasted text before inserting into the buffer.
///
/// This prevents terminal escape sequences in clipboard content from
/// being executed when pasted into the editor.
pub fn sanitize_paste(text: &str) -> String {
    let (sanitized, _) = sanitize_str(text);
    sanitized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_chars_are_sanitized() {
        // ASCII control characters
        assert!(is_control_char('\x00'));  // NUL
        assert!(is_control_char('\x01'));  // SOH
        assert!(is_control_char('\x1B'));  // ESC
        assert!(is_control_char('\x7F'));  // DEL
        
        // But these are NOT control chars (should be let through)
        assert!(!is_control_char('\t'));   // tab
        assert!(!is_control_char('\n'));   // newline
        assert!(!is_control_char('\r'));   // carriage return
    }

    #[test]
    fn test_escape_sequence_stopped() {
        // A file containing \x1b[2J must not actually clear the screen.
        let input = "\x1b[2J\x1b[31mPWNED\x1b[0m\nhello\n";
        let (sanitized, count) = sanitize_str(input);

        assert_eq!(count, 3);
        assert!(!sanitized.contains('\x1b'));
        // ESC → U+241B (SYMBOL FOR ESCAPE).
        assert!(sanitized.contains('\u{241B}'));
    }

    #[test]
    fn test_normal_text_passes_through() {
        let input = "Hello, world! 你好 🌍";
        let (sanitized, count) = sanitize_str(input);
        
        assert_eq!(sanitized, input);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_paste_sanitization() {
        let malicious = "\x1b[31mRed\x1b[0m and \x1b[1mBold\x1b[0m";
        let clean = sanitize_paste(malicious);

        assert!(!clean.contains('\x1b'));
        assert!(clean.contains('\u{241B}'));
    }

    #[test]
    fn esc_and_del_are_distinct_glyphs() {
        // Previously both mapped to '^', so a literal '^' in a file was
        // indistinguishable from a stripped ESC or DEL.
        let (esc_glyph, _) = sanitize_char('\x1b');
        let (del_glyph, _) = sanitize_char('\x7f');
        let (caret, was_sanitized) = sanitize_char('^');
        assert_eq!(esc_glyph, '\u{241B}');
        assert_eq!(del_glyph, '\u{2421}');
        assert_eq!(caret, '^');
        assert!(!was_sanitized, "literal '^' must pass through unchanged");
        assert_ne!(esc_glyph, del_glyph);
        assert_ne!(esc_glyph, caret);
    }

    #[test]
    fn c0_controls_use_control_pictures_block() {
        // U+2400 + cp for cp in 0x00..=0x1F (except tab/newline/CR which pass through).
        for cp in 0x00u32..=0x1F {
            let ch = char::from_u32(cp).unwrap();
            if ch == '\t' || ch == '\n' || ch == '\r' {
                continue;
            }
            let (glyph, sanitized) = sanitize_char(ch);
            assert!(sanitized);
            assert_eq!(glyph as u32, 0x2400 + cp, "U+{:04X} should map to U+{:04X}", cp, 0x2400 + cp);
        }
    }

    #[test]
    fn bidi_overrides_are_sanitized() {
        // Trojan Source (CVE-2021-42574). Without sanitization a terminal
        // with bidi support visually reorders these so what's shown differs
        // from what's stored.
        for cp in 0x202A..=0x202E {
            let ch = char::from_u32(cp).unwrap();
            assert!(is_control_char(ch), "U+{:04X} should be flagged", cp);
        }
        for cp in 0x2066..=0x2069 {
            let ch = char::from_u32(cp).unwrap();
            assert!(is_control_char(ch), "U+{:04X} should be flagged", cp);
        }
    }

    #[test]
    fn bidi_payload_round_trips_to_dot() {
        // From the canonical Trojan Source comment trick.
        let payload = "// admin\u{202E} ;)pmuD";
        let (clean, count) = sanitize_str(payload);
        assert_eq!(count, 1);
        assert!(!clean.contains('\u{202E}'));
        assert!(clean.contains('·'));
    }

    #[test]
    fn bidi_isolate_chars_sanitized() {
        let payload = "before\u{2066}after\u{2069}end";
        let (clean, count) = sanitize_str(payload);
        assert_eq!(count, 2);
        assert!(!clean.contains('\u{2066}'));
        assert!(!clean.contains('\u{2069}'));
    }

    #[test]
    fn ordinary_unicode_passes_through() {
        // The bidi check shouldn't false-positive other CJK / accented chars.
        let s = "café 你好 🌍 ß";
        let (clean, count) = sanitize_str(s);
        assert_eq!(clean, s);
        assert_eq!(count, 0);
    }
}

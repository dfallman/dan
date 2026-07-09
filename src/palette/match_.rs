//! Thin wrapper around the nucleo fuzzy matcher.

use nucleo::Matcher;
use nucleo::pattern::{Pattern, CaseMatching, Normalization, AtomKind};

/// Score `query` against `haystack`. Higher = better match. Returns 0 if no match.
#[allow(dead_code)]
pub fn score(matcher: &mut Matcher, query: &str, haystack: &str) -> u32 {
    if query.is_empty() {
        return 0;
    }
    let pattern = Pattern::new(query, CaseMatching::Smart, Normalization::Smart, AtomKind::Fuzzy);
    let mut buf = Vec::new();
    let h = nucleo::Utf32Str::new(haystack, &mut buf);
    pattern.score(h, matcher).unwrap_or(0)
}

/// Score and also return matched-char indices (for highlighting in the UI).
pub fn score_with_indices(matcher: &mut Matcher, query: &str, haystack: &str) -> Option<(u32, Vec<u32>)> {
    if query.is_empty() {
        return Some((0, Vec::new()));
    }
    let pattern = Pattern::new(query, CaseMatching::Smart, Normalization::Smart, AtomKind::Fuzzy);
    let mut buf = Vec::new();
    let h = nucleo::Utf32Str::new(haystack, &mut buf);
    let mut indices = Vec::new();
    let s = pattern.indices(h, matcher, &mut indices)?;
    Some((s, indices))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matcher() -> Matcher {
        Matcher::new(nucleo::Config::DEFAULT)
    }

    #[test]
    fn empty_query_returns_zero_score() {
        let mut m = matcher();
        assert_eq!(score(&mut m, "", "anything"), 0);
    }

    #[test]
    fn substring_match_scores_positive() {
        let mut m = matcher();
        assert!(score(&mut m, "main", "src/main.rs") > 0);
    }

    #[test]
    fn subsequence_match_scores_positive() {
        let mut m = matcher();
        assert!(score(&mut m, "smr", "src/main.rs") > 0);
    }

    #[test]
    fn no_match_scores_zero() {
        let mut m = matcher();
        assert_eq!(score(&mut m, "xyz", "src/main.rs"), 0);
    }

    #[test]
    fn contiguous_scores_higher_than_scattered() {
        let mut m = matcher();
        let contig = score(&mut m, "main", "src/main.rs");
        let scattered = score(&mut m, "main", "m_a_i_n");
        assert!(contig > scattered, "contig={} scattered={}", contig, scattered);
    }
}

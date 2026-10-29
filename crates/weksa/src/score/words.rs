//! The one way the scorer reads words: UAX #29 word segmentation, then NFC and
//! full Unicode case folding. The novel-coverage diagnostic reads text through
//! here, so every script is treated by one rule.

use caseless::default_case_fold_str;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

/// NFC, then full default case folding.
pub(super) fn normalise(s: &str) -> String {
    default_case_fold_str(&s.nfc().collect::<String>())
}

/// The pieces of one segment: split at `_` and `.` (which are dropped) and
/// at every lowercase-to-uppercase transition, so `kick_the_bucket`,
/// `kick.the.bucket` and `kickTheBucket` read as `kick the bucket`.
fn pieces(segment: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut previous: Option<char> = None;
    for (at, c) in segment.char_indices() {
        let separator = c == '_' || c == '.';
        let camel = previous.is_some_and(char::is_lowercase) && c.is_uppercase();
        if separator || camel {
            out.push(&segment[start..at]);
            start = if separator { at + c.len_utf8() } else { at };
        }
        previous = Some(c);
    }
    out.push(&segment[start..]);
    out
}

/// The normalised words of `text` in order: UAX #29 segments containing an
/// alphanumeric char (digits kept), each split by `pieces`, empty or
/// non-alphanumeric pieces dropped.
pub(super) fn words(text: &str) -> Vec<String> {
    text.split_word_bounds()
        .filter(|segment| segment.chars().any(char::is_alphanumeric))
        .flat_map(pieces)
        .filter(|piece| piece.chars().any(char::is_alphanumeric))
        .map(normalise)
        .collect()
}

//! Pins for private functions of `run.rs` that the doors reach only through a
//! whole run.

use super::*;

/// A line is probed only when it has at least four words, and then it is
/// split in two near its byte middle, the pieces trimmed.
#[test]
fn a_recall_line_splits_from_four_words_and_not_before() {
    assert_eq!(recall_split("one two three"), None);
    assert_eq!(recall_split(""), None);
    assert_eq!(
        recall_split("one two three four"),
        Some(("one two".to_string(), "three four".to_string()))
    );
    // No spaces: each ideograph is a word, the split falls at the middle.
    assert_eq!(
        recall_split("天地不仁以万物为刍狗"),
        Some(("天地不仁以".to_string(), "万物为刍狗".to_string()))
    );
}

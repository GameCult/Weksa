//! Pins for `unmark` that the import door only reaches through a whole work.

use super::*;

/// A form that opens another markup is malformed, though its entity id is
/// sound: the model wrote a nested or unclosed mark, not a name.
#[test]
fn a_form_that_opens_a_mark_is_malformed() {
    assert_eq!(unmark("[[entity:orsa|Or[[sa]] came"), None);
    // The same line without the nested opener unmarks.
    let (plain, names) = unmark("[[entity:orsa|Orsa]] came").unwrap();
    assert_eq!((plain.as_str(), names.len()), ("Orsa came", 1));
}

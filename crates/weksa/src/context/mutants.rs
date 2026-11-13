//! Pins for private reach of `context.rs` that no run can produce.

use super::*;

/// A corpus with no live Train item of the panel's languages holds an empty
/// train. (A run refuses a panel without Train items at open, so only the
/// builder shows it.)
#[test]
fn no_train_item_of_the_panel_is_an_empty_train() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = CorpusStore::open(&dir.path().join("c.cc")).unwrap();
    let train = TrainItems::from_corpus(&corpus, &["en-US".to_string()]).unwrap();
    assert_eq!((train.len(), train.is_empty()), (0, true));
}

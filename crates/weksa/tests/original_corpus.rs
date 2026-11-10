//! The original corpus (`data/original`): passages written for the
//! non-memorised golf-loop target. It holds no public-domain text, so a model
//! cannot have it in pretraining, and every item says who wrote it.

use std::path::Path;

use weksa::corpus::{ModelFamily, Source};
use weksa::split::{Split, SplitScheme};
use weksa::{CorpusStore, EntityStore, InventoryStore};

fn stores() -> (CorpusStore, EntityStore) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/original");
    let tmp = tempfile::tempdir().unwrap();
    for f in ["corpus.cc", "entity.cc", "inventory.cc"] {
        std::fs::copy(dir.join(f), tmp.path().join(f)).unwrap();
    }
    let keep = tmp.keep();
    (
        CorpusStore::open(&keep.join("corpus.cc")).unwrap(),
        EntityStore::open(&keep.join("entity.cc")).unwrap(),
    )
}

#[test]
fn every_original_item_records_its_author_and_none_is_public_domain() {
    let (corpus, entities) = stores();
    let items = corpus.all().unwrap();
    assert_eq!(items.len(), 6);
    for item in &items {
        match &item.source {
            Source::ModelWritten {
                family, generator, ..
            } => {
                assert_eq!(*family, ModelFamily::Anthropic, "{}", item.id);
                let generator = generator.as_deref().unwrap_or_default();
                assert!(generator.contains("claude-sonnet-5-5"), "{}", item.id);
            }
            other => panic!("{}: not model-written: {other:?}", item.id),
        }
        assert!(entities.get(&item.speaker).unwrap().is_some(), "{}", item.id);
    }
}

#[test]
fn the_original_items_spread_over_train_and_validation_and_none_is_in_test() {
    let (corpus, _) = stores();
    let splits: Vec<Option<Split>> = corpus
        .all()
        .unwrap()
        .iter()
        .map(|i| SplitScheme::V1.live(i))
        .collect();
    let count = |s| splits.iter().filter(|x| **x == Some(s)).count();
    assert_eq!((count(Split::Train), count(Split::Validation)), (3, 3));
    assert_eq!(count(Split::Test), 0);
}

#[test]
fn the_original_inventory_is_the_dao_revision_three() {
    let dao = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/daodejing/inventory.cc");
    let own = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/original/inventory.cc");
    assert_eq!(std::fs::read(dao).unwrap(), std::fs::read(&own).unwrap());
    let tmp = tempfile::tempdir().unwrap();
    let copy = tmp.path().join("inventory.cc");
    std::fs::copy(&own, &copy).unwrap();
    assert!(InventoryStore::open(&copy).is_ok());
}

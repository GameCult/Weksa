//! The committed seed stores replay through today's doors, so a rule that
//! tightens cannot leave a seed the stores would now refuse.
mod common;
use std::path::Path;
use weksa::{CorpusStore, EntityStore};

#[test]
fn committed_seeds_replay_through_the_doors() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let seeded_entities = EntityStore::open(&data.join("entity/entity.cc")).unwrap();
    let seeded_corpus = CorpusStore::open(&data.join("corpus/corpus.cc")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut entities = EntityStore::open(&dir.path().join("e.cc")).unwrap();
    let all = seeded_entities.all().unwrap();
    assert!(!all.is_empty());
    for e in all {
        entities.put(e).unwrap();
    }
    let mut corpus = CorpusStore::open(&dir.path().join("c.cc")).unwrap();
    let items = seeded_corpus.all().unwrap();
    assert!(!items.is_empty());
    for item in items {
        let withdrawn = item.withdrawn;
        let id = item.id.clone();
        corpus
            .add(
                weksa::CorpusItem {
                    withdrawn: false,
                    ..item
                },
                &entities,
            )
            .unwrap();
        if withdrawn {
            corpus.withdraw(&id).unwrap();
        }
    }
}

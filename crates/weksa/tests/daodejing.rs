//! The Dao De Jing test bed (`data/daodejing`): its Legge items carry full
//! public-domain provenance, and its packets are admitted and lower to the
//! English the first golf-loop run scored (`docs/targets/dao-de-jing-test-bed.md`).

use std::path::Path;

use weksa::daodejing::packets;
use weksa::{CorpusStore, EntityStore, InventoryStore, PacketStore, Source, lower};

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/daodejing")
}

#[test]
fn every_legge_item_names_licence_jurisdiction_translator_and_edition_year() {
    let items = CorpusStore::open(&root().join("corpus.cc"))
        .unwrap()
        .all()
        .unwrap();
    assert_eq!(items.len(), 5);
    for item in items {
        let Source::PublicDomain {
            licence,
            jurisdiction,
            translator,
            edition_year,
            ..
        } = item.source
        else {
            panic!("{} is not a public-domain item", item.id);
        };
        assert!(
            !licence.is_empty() && !jurisdiction.is_empty(),
            "{}",
            item.id
        );
        assert_eq!((translator.as_str(), edition_year), ("James Legge", 1891));
    }
}

#[test]
fn the_slice_packets_are_admitted_and_lower_to_the_scored_english() {
    let inventory = InventoryStore::open(&root().join("inventory.cc")).unwrap();
    let corpus = CorpusStore::open(&root().join("corpus.cc")).unwrap();
    let entities = EntityStore::open(&root().join("entity.cc")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut store = PacketStore::open(&dir.path().join("p.cc")).unwrap();
    let inv = inventory.inventory().unwrap();
    let lowered: Vec<String> = packets()
        .into_iter()
        .map(|p| {
            let view = store.admit(p, &inventory, &corpus, &entities).unwrap();
            lower(&view, &inv, &entities).unwrap()
        })
        .collect();
    assert_eq!(
        lowered,
        [
            "The sage empties the minds. The sage fills the bellies. The sage weakens the wills. The sage strengthens the bones.",
            "The heaven is long-enduring. The earth continues long.",
            "The highest excellence is like the water.",
            "The words that are strictly true seem paradoxical.",
            "The space between heaven and earth may be compared to the bellows.",
        ]
    );
    // The committed packet store holds exactly what the module builds.
    let stored = PacketStore::open(&root().join("packets.cc")).unwrap();
    for p in packets() {
        assert_eq!(stored.get(&p.id.to_string()).unwrap(), Some(p));
    }
}

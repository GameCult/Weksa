//! Pins for behaviour of `context.rs`, `encode.rs` and `run.rs` found unpinned
//! by mutation testing. Each test states what the code does in domain terms.

mod common;

use std::path::{Path, PathBuf};

use weksa::context::Prompt;
use weksa::entity::{Entity, EntityOrigin, LanguageForms};
use weksa::packet::{EntityId, proof_packet};
use weksa::{CorpusItemId, CorpusStore, EntityForms, EntityStore, InventoryStore, PacketStore};

fn data(sub: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").join(sub)
}

struct Held(Vec<Entity>);

impl EntityForms for Held {
    fn entity(&self, id: &EntityId) -> anyhow::Result<Option<Entity>> {
        Ok(self.0.iter().find(|e| &e.id == id).cloned())
    }
}

fn spoken(slug: &str, form: &str) -> Entity {
    Entity {
        id: EntityId::parse(&format!("entity:{slug}")).unwrap(),
        forms: vec![LanguageForms {
            language: "en-US".into(),
            forms: vec![form.into()],
            pronunciation: None,
        }],
        origin: EntityOrigin::Cast { persona: slug.into(), fixture: "f.yaml".into() },
    }
}

/// A speaker whom no referent cites contributes no entry to the lowering
/// prompt, even when another entity is cited: only the cited entity is listed.
#[test]
fn an_uncited_speaker_is_absent_from_the_lowering_prompt() {
    let cast_inv = InventoryStore::open(&data("interlingua/inventory.cc")).unwrap();
    let entities = EntityStore::open(&data("entity/entity.cc")).unwrap();
    let corpus = CorpusStore::open(&data("corpus/corpus.cc")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let view = PacketStore::open(&dir.path().join("p.cc"))
        .unwrap()
        .admit(proof_packet(), &cast_inv, &corpus, &entities)
        .unwrap();
    let state = weksa::SpeakerStateStore::open(&dir.path().join("s.cc")).unwrap();
    let view = view.with_scene(&cast_inv.inventory().unwrap(), &state, &corpus).unwrap();
    let held = Held(vec![spoken("nibu", "Nibu"), spoken("other", "SPEAKERCANARY")]);
    let item = CorpusItemId::parse("item:nibu:1").unwrap();
    let speaker = EntityId::parse("entity:other").unwrap();
    let prompt = Prompt::lower(
        &view,
        &item,
        &speaker,
        "en-US",
        &cast_inv.inventory().unwrap(),
        &held,
        &weksa::Ontology::empty(),
    )
    .unwrap();
    let text = prompt.text();
    assert!(text.contains("\n\nentities: {\"entity:nibu\":[\"Nibu\"]}\n\n"), "{text}");
    assert!(!text.contains("SPEAKERCANARY"), "{text}");
}

/// A run refuses a panel language that lacks a live Train or a live Validation
/// item: one no item carries, one with only a Test item, one with only Train.
#[test]
fn a_panel_language_no_item_carries_is_refused_at_open() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    // Builds the corpus, entity and ontology stores the run reads.
    drop(common::run_over(d));
    let entities = common::entities_in(d);
    let mut corpus = CorpusStore::open(&d.join("c.cc")).unwrap();
    for (key, language, bucket) in [("item:jp:1", "ja-JP", 90), ("item:pt:1", "pt-BR", 10)] {
        let mut item = common::item_in(key, &format!("line of {key}"), &common::group_in_bucket(bucket), common::cast());
        item.language = language.into();
        corpus.add(item, &entities).unwrap();
    }
    drop(corpus);
    let create = |panel: &[&str]| {
        weksa::run::RunStores::create(
            &d.join(format!("run-{}", panel.join("-"))),
            "t",
            weksa::run::RunPaths {
                corpus: &d.join("c.cc"),
                inventory: &d.join("i.cc"),
                entities: &d.join("entities.cc"),
                ontology: &d.join("ontology.cc"),
                speaker_state: &common::speaker_state_beside(&d.join("ontology.cc")),
            },
            "fake-model",
            "bag",
            weksa::run::LowererReach::ToolLessRelay,
            weksa::corpus::ModelFamily::Unrecorded,
            &panel.iter().map(|t| (*t).to_string()).collect::<Vec<_>>(),
        )
    };
    assert!(create(&["en-US"]).is_ok());
    for panel in [&["ja-JP"][..], &["pt-BR"], &["xx-XX"], &["en-US", "ja-JP"]] {
        assert!(matches!(create(panel), Err(weksa::run::RunError::Panel)), "{panel:?}");
    }
}

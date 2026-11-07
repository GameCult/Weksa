//! The speaker's relation to the addressee as a lowering input: typed feature
//! values of slot `Scene`, checked against the inventory at the view's revision,
//! never wording, and only for a packet that names an addressee.

mod common;
use std::path::Path;

use common::{grade, iv, lab};
use weksa::inventory::{proof_revision, scene_revision, seed};
use weksa::packet::{FeatValue, FeatureValue, Packet, proof_packet};
use weksa::{
    CorpusStore, EntityStore, Inventory, InventoryStore, LoweringInput, PacketStore, Refusal, lower,
};

fn store() -> (InventoryStore, Inventory, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = InventoryStore::open(&dir.path().join("i.cc")).unwrap();
    for (rev, entries) in [seed(), proof_revision(), scene_revision()] {
        store.admit_revision(rev, entries).unwrap();
    }
    let inv = store.inventory().unwrap();
    (store, inv, dir)
}

fn admitted(packet: &Packet, inventory: &InventoryStore) -> LoweringInput {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let corpus = CorpusStore::open(&root.join("data/corpus/corpus.cc")).unwrap();
    let entities = EntityStore::open(&root.join("data/entity/entity.cc")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    PacketStore::open(&dir.path().join("p.cc"))
        .unwrap()
        .admit(packet.clone(), inventory, &corpus, &entities)
        .unwrap()
}

fn at_rev(rev: u32) -> Packet {
    let mut p = proof_packet();
    p.inventory_rev = rev;
    p
}

fn feat(id: &str, value: FeatValue) -> FeatureValue {
    FeatureValue {
        feature: iv(id),
        value,
    }
}

fn status(inv: &Inventory) -> String {
    inv.entries()
        .find(|e| e.gloss == "relative_status")
        .unwrap()
        .id
        .to_string()
}

fn id_of(inv: &Inventory, gloss: &str) -> String {
    inv.entries()
        .find(|e| e.gloss == gloss)
        .unwrap()
        .id
        .to_string()
}

fn rules(r: Result<LoweringInput, Vec<Refusal>>) -> Vec<&'static str> {
    r.unwrap_err().iter().map(Refusal::rule).collect()
}

#[test]
fn a_scene_relation_reaches_the_lowering_view_and_changes_no_english() {
    let (store, inv, _dir) = store();
    let plain = admitted(&at_rev(3), &store);
    assert!(plain.scene().is_empty());
    let scene = vec![
        feat(&status(&inv), FeatValue::Option(lab("above"))),
        feat(&id_of(&inv, "intimacy"), FeatValue::Grade(grade(200))),
        feat(&id_of(&inv, "in_group"), FeatValue::Flag(false)),
    ];
    let with = plain.clone().with_scene(&inv, scene.clone()).unwrap();
    assert_eq!(with.scene(), scene.as_slice());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let entities = EntityStore::open(&root.join("data/entity/entity.cc")).unwrap();
    assert_eq!(
        lower(&with, &inv, &entities).unwrap(),
        lower(&plain, &inv, &entities).unwrap()
    );
}

#[test]
fn a_relation_needs_an_addressee() {
    let (store, inv, _dir) = store();
    let mut p = at_rev(3);
    p.discourse.addressee = None;
    let view = admitted(&p, &store);
    let scene = vec![feat(&id_of(&inv, "in_group"), FeatValue::Flag(true))];
    assert_eq!(rules(view.clone().with_scene(&inv, scene)), ["V13"]);
    assert!(view.with_scene(&inv, vec![]).unwrap().scene().is_empty());
}

#[test]
fn a_relation_value_is_checked_like_any_feature() {
    let (store, inv, _dir) = store();
    let view = admitted(&at_rev(3), &store);
    let status = status(&inv);
    let intimacy = id_of(&inv, "intimacy");
    let take = |scene| view.clone().with_scene(&inv, scene);
    // A label the feature does not declare.
    assert_eq!(
        rules(take(vec![feat(&status, FeatValue::Option(lab("lordly")))])),
        ["V7"]
    );
    // A value of the wrong kind.
    assert_eq!(
        rules(take(vec![feat(&intimacy, FeatValue::Flag(true))])),
        ["V7"]
    );
    // A feature declared for another slot: seed feature 1 sits in Predication.
    assert_eq!(
        rules(take(vec![feat(
            "feature:1",
            FeatValue::Option(lab("negative"))
        )])),
        ["V8"]
    );
    // An id that is not a feature at all.
    assert_eq!(
        rules(take(vec![feat("concept:3", FeatValue::Flag(true))])),
        ["V6"]
    );
    // Twice in one list.
    let twice = vec![
        feat(&intimacy, FeatValue::Grade(grade(1))),
        feat(&intimacy, FeatValue::Grade(grade(2))),
    ];
    assert_eq!(rules(take(twice)), ["V11"]);
}

#[test]
fn a_view_of_a_revision_before_the_scene_vocabulary_cannot_carry_one() {
    let (store, inv, _dir) = store();
    // Revision 2 predates the Scene features, so their ids do not resolve there.
    let view = admitted(&at_rev(2), &store);
    let scene = vec![feat(&id_of(&inv, "intimacy"), FeatValue::Grade(grade(1)))];
    assert_eq!(rules(view.with_scene(&inv, scene)), ["V5"]);
}

#[test]
fn a_stance_feature_is_not_a_scene_feature() {
    let (store, inv, _dir) = store();
    let view = admitted(&at_rev(3), &store);
    let anger = id_of(&inv, "anger");
    assert_eq!(
        rules(view.with_scene(&inv, vec![feat(&anger, FeatValue::Grade(grade(1)))])),
        ["V8"]
    );
}

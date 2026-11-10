//! The speaker's relation to the addressee as a lowering input: the relation of
//! a `weksa.scene` document from the speaker to the addressee referent's
//! entity, typed feature values of slot `Scene` checked against the inventory
//! at the view's revision, never wording.

mod common;
use std::path::Path;

use common::{eid, grade, iv, lab, nd};
use weksa::inventory::{proof_revision, seed, speaker_scene_revision};
use weksa::packet::{FeatValue, FeatureValue, Packet, proof_packet};
use weksa::speaker::{Annotator, PairRelation, Party, Scene};
use weksa::{
    CorpusStore, EntityId, EntityStore, Inventory, InventoryStore, LoweringInput, PacketStore,
    Refusal, lower,
};

fn store() -> (InventoryStore, Inventory, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = InventoryStore::open(&dir.path().join("i.cc")).unwrap();
    for (rev, entries) in [seed(), proof_revision(), speaker_scene_revision()] {
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

/// The proof packet at `rev`. Its referent `n1` has no entity and `n3` is
/// `entity:nibu`; `addressee` names which one is spoken to, or none.
fn packet(rev: u32, addressee: Option<&str>) -> Packet {
    let mut p = proof_packet();
    p.inventory_rev = rev;
    p.discourse.addressee = addressee.map(nd);
    p
}

fn feat(inv: &Inventory, gloss: &str, value: FeatValue) -> FeatureValue {
    let id = inv
        .entries()
        .find(|e| e.gloss == gloss)
        .unwrap_or_else(|| panic!("no feature {gloss}"))
        .id;
    FeatureValue { feature: id, value }
}

fn rules(r: Result<LoweringInput, Vec<Refusal>>) -> Vec<&'static str> {
    r.unwrap_err().iter().map(Refusal::rule).collect()
}

fn speaker() -> EntityId {
    eid("brokkr")
}

/// Brokkr speaks; the scene holds a relation to Nibu and one to an unnamed
/// party, with different values.
fn scene(inv: &Inventory) -> Scene {
    scene_with(vec![
        PairRelation {
            from: speaker(),
            to: Party::Named(eid("nibu")),
            features: vec![
                feat(inv, "relative_status", FeatValue::Option(lab("above"))),
                feat(inv, "intimacy", FeatValue::Grade(grade(200))),
                feat(inv, "in_group", FeatValue::Flag(false)),
            ],
        },
        PairRelation {
            from: speaker(),
            to: Party::Unnamed,
            features: vec![feat(inv, "intimacy", FeatValue::Grade(grade(7)))],
        },
    ])
}

fn scene_with(relations: Vec<PairRelation>) -> Scene {
    Scene {
        group: "g".into(),
        n: 1,
        inventory_rev: 3,
        participants: vec![speaker(), eid("nibu")],
        relations,
        annotator: Annotator::Operator,
    }
}

fn selected(view: LoweringInput, inv: &Inventory, s: &Scene) -> Vec<FeatureValue> {
    view.with_scene(inv, s, &speaker())
        .unwrap()
        .scene()
        .to_vec()
}

#[test]
fn a_scene_relation_reaches_the_lowering_view_and_changes_no_english() {
    let (store, inv, _dir) = store();
    let plain = admitted(&packet(3, Some("n3")), &store);
    assert!(plain.scene().is_empty());
    let s = scene(&inv);
    let with = plain.clone().with_scene(&inv, &s, &speaker()).unwrap();
    assert_eq!(with.scene(), s.relations[0].features.as_slice());
    assert!(!with.scene().is_empty());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let entities = EntityStore::open(&root.join("data/entity/entity.cc")).unwrap();
    assert_eq!(
        lower(&with, &inv, &entities).unwrap(),
        lower(&plain, &inv, &entities).unwrap()
    );
}

#[test]
fn with_scene_selects_by_addressee() {
    let (store, inv, _dir) = store();
    let s = scene(&inv);
    // A referent with an entity selects the relation to that entity.
    let named = selected(admitted(&packet(3, Some("n3")), &store), &inv, &s);
    assert_eq!(named, s.relations[0].features);
    // A referent without one selects the relation to an unnamed party.
    let unnamed = selected(admitted(&packet(3, Some("n1")), &store), &inv, &s);
    assert_eq!(unnamed, s.relations[1].features);
    // No addressee selects nothing.
    assert!(selected(admitted(&packet(3, None), &store), &inv, &s).is_empty());
    // A named addressee the scene lacks selects nothing, though an unnamed
    // relation exists.
    let only_unnamed = scene_with(vec![s.relations[1].clone()]);
    assert!(
        selected(
            admitted(&packet(3, Some("n3")), &store),
            &inv,
            &only_unnamed
        )
        .is_empty()
    );
    // Another speaker's relations are not selected.
    let view = admitted(&packet(3, Some("n3")), &store);
    let theirs = Scene {
        participants: vec![speaker(), eid("nibu"), eid("hero")],
        ..s.clone()
    };
    assert!(
        view.with_scene(&inv, &theirs, &eid("hero"))
            .unwrap()
            .scene()
            .is_empty()
    );
    // A replacement replaces: an empty selection clears the earlier one.
    let first = admitted(&packet(3, Some("n3")), &store)
        .with_scene(&inv, &s, &speaker())
        .unwrap();
    assert!(
        first
            .with_scene(&inv, &only_unnamed, &speaker())
            .unwrap()
            .scene()
            .is_empty()
    );
}

#[test]
fn with_scene_refusals() {
    let (store, inv, _dir) = store();
    let view = admitted(&packet(3, Some("n3")), &store);
    let to_nibu = |features| {
        scene_with(vec![PairRelation {
            from: speaker(),
            to: Party::Named(eid("nibu")),
            features,
        }])
    };
    let take = |s: &Scene| view.clone().with_scene(&inv, s, &speaker());
    // A speaker the scene does not list.
    assert_eq!(
        view.clone()
            .with_scene(&inv, &scene(&inv), &eid("hero"))
            .map(|_| ())
            .unwrap_err(),
        [Refusal::V16SpeakerNotInScene]
    );
    // A label the feature does not declare.
    let lordly = feat(&inv, "relative_status", FeatValue::Option(lab("lordly")));
    assert_eq!(rules(take(&to_nibu(vec![lordly]))), ["V7"]);
    // A value of the wrong kind.
    let flag = feat(&inv, "intimacy", FeatValue::Flag(true));
    assert_eq!(rules(take(&to_nibu(vec![flag]))), ["V7"]);
    // A stance feature in a relation.
    let anger = feat(&inv, "anger", FeatValue::Grade(grade(1)));
    assert_eq!(rules(take(&to_nibu(vec![anger]))), ["V8"]);
    // A speaker feature in a relation.
    let age = feat(&inv, "education", FeatValue::Grade(grade(1)));
    assert_eq!(rules(take(&to_nibu(vec![age]))), ["V8"]);
    // Twice in one list.
    let twice = vec![
        feat(&inv, "intimacy", FeatValue::Grade(grade(1))),
        feat(&inv, "intimacy", FeatValue::Grade(grade(2))),
    ];
    assert_eq!(rules(take(&to_nibu(twice))), ["V11"]);
    // Revision 2 predates the Scene features, so their ids do not resolve there.
    let old = admitted(&packet(2, Some("n3")), &store);
    assert_eq!(
        rules(old.with_scene(&inv, &scene(&inv), &speaker())),
        ["V5", "V5", "V5"]
    );
    // An unselected relation is not checked: only the selected one is.
    let bad_elsewhere = scene_with(vec![
        PairRelation {
            from: speaker(),
            to: Party::Unnamed,
            features: vec![feat(&inv, "anger", FeatValue::Grade(grade(1)))],
        },
        PairRelation {
            from: speaker(),
            to: Party::Named(eid("nibu")),
            features: vec![],
        },
    ]);
    assert!(take(&bad_elsewhere).unwrap().scene().is_empty());
}

#[test]
fn feature_ids_are_stable() {
    let (_store, inv, _dir) = store();
    let ids: Vec<_> = [
        "relative_status",
        "intimacy",
        "in_group",
        "socioeconomic_standing",
        "education",
        "age_cohort",
    ]
    .map(|g| feat(&inv, g, FeatValue::Flag(true)).feature)
    .into();
    let want: Vec<_> = (19..=24).map(|n| iv(&format!("feature:{n}"))).collect();
    assert_eq!(ids, want);
}

//! The speaker's relation to the addressee as a lowering input. `with_scene`
//! names neither speaker nor scene: the view knows its corpus item, so the
//! speaker is the item's speaker and the scene is the store's latest for the
//! item's group. The relation is the one from that speaker to the addressee
//! referent's entity. Typed feature values only, never wording.

mod common;
use common::{cast, eid, entities_in, grade, item_in, lab, nd};
use weksa::inventory::{proof_revision, seed, speaker_scene_revision};
use weksa::packet::{FeatValue, FeatureValue, Packet, proof_packet};
use weksa::speaker::{Annotator, PairRelation, Party, Scene};
use weksa::{
    CorpusStore, EntityStore, Inventory, InventoryStore, LoweringInput, NameSpan, PacketStore,
    Refusal, SpeakerStateStore, StoreError, lower,
};

struct W {
    dir: tempfile::TempDir,
    inventory: InventoryStore,
    corpus: CorpusStore,
    entities: EntityStore,
    state: SpeakerStateStore,
}

/// Group `g` holds `item:nibu:1`: Nibu says "Brokkr, come here." Group `h`
/// holds `item:nibu:2`, also spoken by Nibu.
fn world() -> W {
    let dir = tempfile::tempdir().unwrap();
    let mut inventory = InventoryStore::open(&dir.path().join("i.cc")).unwrap();
    for (rev, entries) in [seed(), proof_revision(), speaker_scene_revision()] {
        inventory.admit_revision(rev, entries).unwrap();
    }
    let entities = entities_in(dir.path());
    let mut corpus = CorpusStore::open(&dir.path().join("c.cc")).unwrap();
    let mut first = item_in("item:nibu:1", "Brokkr, come here.", "g", cast());
    first.names = vec![NameSpan {
        entity: eid("brokkr"),
        start: 0,
        end: 6,
    }];
    corpus.add(first, &entities).unwrap();
    corpus
        .add(item_in("item:nibu:2", "Go away.", "h", cast()), &entities)
        .unwrap();
    let state = SpeakerStateStore::open(&dir.path().join("s.cc")).unwrap();
    W {
        dir,
        inventory,
        corpus,
        entities,
        state,
    }
}

fn inv(w: &W) -> Inventory {
    w.inventory.inventory().unwrap()
}

fn feat(gloss: &str, value: FeatValue) -> FeatureValue {
    let feature = seed()
        .1
        .into_iter()
        .chain(speaker_scene_revision().1)
        .find(|e| e.gloss == gloss)
        .unwrap_or_else(|| panic!("no feature {gloss}"))
        .id;
    FeatureValue { feature, value }
}

fn intimacy(v: u16) -> Vec<FeatureValue> {
    vec![feat("intimacy", FeatValue::Grade(grade(v)))]
}

fn status() -> Vec<FeatureValue> {
    vec![
        feat("relative_status", FeatValue::Option(lab("above"))),
        feat("intimacy", FeatValue::Grade(grade(200))),
        feat("in_group", FeatValue::Flag(false)),
    ]
}

fn relation(from: &str, to: Party, features: Vec<FeatureValue>) -> PairRelation {
    PairRelation {
        from: eid(from),
        to,
        features,
    }
}

/// Nibu's relations to Brokkr and to an unnamed party, and Brokkr's to Nibu.
fn scene(group: &str, n: u32, participants: &[&str], relations: Vec<PairRelation>) -> Scene {
    Scene {
        group: group.into(),
        n,
        inventory_rev: 3,
        participants: participants.iter().map(|p| eid(p)).collect(),
        relations,
        annotator: Annotator::Operator,
    }
}

fn standard(group: &str, n: u32) -> Scene {
    scene(
        group,
        n,
        &["nibu", "brokkr"],
        vec![
            relation("nibu", Party::Named(eid("brokkr")), status()),
            relation("nibu", Party::Unnamed, intimacy(7)),
            relation("brokkr", Party::Named(eid("nibu")), intimacy(99)),
        ],
    )
}

fn put(w: &mut W, s: Scene) {
    w.state
        .put_scene(s, &w.inventory, &w.corpus, &w.entities)
        .unwrap();
}

/// The line of `item:nibu:1` at `rev`, its addressee node `addressee` (`n1`
/// has no entity, `n3` is Brokkr), through the packet door.
fn view_of(w: &W, rev: u32, addressee: Option<&str>) -> LoweringInput {
    let mut p: Packet = proof_packet();
    p.inventory_rev = rev;
    p.referents[2].entity = Some(eid("brokkr"));
    p.discourse.addressee = addressee.map(nd);
    // A packet key holds its packet for good, so each view gets a fresh store.
    let scratch = tempfile::tempdir().unwrap();
    PacketStore::open(&scratch.path().join("p.cc"))
        .unwrap()
        .admit(p, &w.inventory, &w.corpus, &w.entities)
        .unwrap()
}

fn rules(r: Result<LoweringInput, StoreError>) -> Vec<&'static str> {
    match r.map(|_| ()).unwrap_err() {
        StoreError::Invalid(rs) => rs.iter().map(Refusal::rule).collect(),
        other => panic!("not a rule refusal: {other}"),
    }
}

#[test]
fn a_scene_relation_reaches_the_lowering_view_and_changes_no_english() {
    let mut w = world();
    put(&mut w, standard("g", 1));
    let plain = view_of(&w, 3, Some("n3"));
    assert!(plain.scene().is_empty());
    let with = plain.clone().with_scene(&inv(&w), &w.state).unwrap();
    assert_eq!(with.scene(), status().as_slice());
    assert_eq!(
        lower(&with, &inv(&w), &w.entities).unwrap(),
        lower(&plain, &inv(&w), &w.entities).unwrap()
    );
}

#[test]
fn with_scene_selects_by_addressee() {
    let mut w = world();
    put(&mut w, standard("g", 1));
    let take = |w: &W, addressee| {
        view_of(w, 3, addressee)
            .with_scene(&inv(w), &w.state)
            .unwrap()
            .scene()
            .to_vec()
    };
    // A referent with an entity selects the relation to that entity; a
    // referent without one, the relation to an unnamed party; and the
    // speaker's relation, never Brokkr's to Nibu.
    assert_eq!(take(&w, Some("n3")), status());
    assert_eq!(take(&w, Some("n1")), intimacy(7));
    // No addressee selects nothing.
    assert!(take(&w, None).is_empty());
}

#[test]
fn a_named_addressee_the_scene_lacks_never_falls_back_to_unnamed() {
    let mut w = world();
    // Brokkr is not a participant and Nibu has only an unnamed relation.
    put(
        &mut w,
        scene(
            "g",
            1,
            &["nibu", "player"],
            vec![relation("nibu", Party::Unnamed, intimacy(7))],
        ),
    );
    let got = view_of(&w, 3, Some("n3"))
        .with_scene(&inv(&w), &w.state)
        .unwrap();
    assert!(got.scene().is_empty());
    // A participant without a relation behaves the same.
    put(
        &mut w,
        scene(
            "g",
            2,
            &["nibu", "brokkr"],
            vec![relation("nibu", Party::Unnamed, intimacy(7))],
        ),
    );
    let got = view_of(&w, 3, Some("n3"))
        .with_scene(&inv(&w), &w.state)
        .unwrap();
    assert!(got.scene().is_empty());
}

#[test]
fn the_scene_is_the_items_group_and_its_latest_revision() {
    let mut w = world();
    // Group h holds a different relation for the same pair.
    put(
        &mut w,
        scene(
            "h",
            1,
            &["nibu", "brokkr"],
            vec![relation("nibu", Party::Named(eid("brokkr")), intimacy(1))],
        ),
    );
    // No scene for the item's group selects nothing, whatever other groups hold.
    let got = view_of(&w, 3, Some("n3"))
        .with_scene(&inv(&w), &w.state)
        .unwrap();
    assert!(got.scene().is_empty());
    // Revisions: the highest n is read.
    put(&mut w, standard("g", 1));
    let mut second = standard("g", 2);
    second.relations[0].features = intimacy(5);
    put(&mut w, second);
    let got = view_of(&w, 3, Some("n3"))
        .with_scene(&inv(&w), &w.state)
        .unwrap();
    assert_eq!(got.scene(), intimacy(5).as_slice());
    // A replacement replaces: a store without the scene clears the selection.
    let empty = SpeakerStateStore::open(&w.dir.path().join("empty.cc")).unwrap();
    let cleared = got.with_scene(&inv(&w), &empty).unwrap();
    assert!(cleared.scene().is_empty());
}

#[test]
fn with_scene_refusals() {
    let mut w = world();
    put(&mut w, standard("g", 1));
    // The scene was admitted at revision 3; a view of revision 2 predates the
    // Scene features, so the selected relation's ids do not resolve there.
    assert_eq!(
        rules(view_of(&w, 2, Some("n3")).with_scene(&inv(&w), &w.state)),
        ["V5", "V5", "V5"]
    );
    // A scene that does not list the item's speaker: group k admitted while
    // Brokkr was its only speaker, then an item of Nibu's joins it.
    let mut item = item_in("item:brokkr:1", "Hello.", "k", cast());
    item.speaker = eid("brokkr");
    w.corpus.add(item, &w.entities).unwrap();
    put(&mut w, scene("k", 1, &["brokkr"], vec![]));
    w.corpus
        .add(
            item_in("item:nibu:3", "Hello there.", "k", cast()),
            &w.entities,
        )
        .unwrap();
    let mut p = proof_packet();
    p.inventory_rev = 3;
    p.provenance.corpus_item = weksa::CorpusItemId::parse("item:nibu:3").unwrap();
    p.referents[2].entity = Some(eid("nibu"));
    let view = PacketStore::open(&w.dir.path().join("p3.cc"))
        .unwrap()
        .admit(p, &w.inventory, &w.corpus, &w.entities)
        .unwrap();
    assert_eq!(rules(view.with_scene(&inv(&w), &w.state)), ["V16"]);
}

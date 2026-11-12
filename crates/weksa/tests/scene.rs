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
    Refusal, SceneRead, SpeakerStateStore, StoreError, lower,
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
    for (rev, entries) in [seed(), proof_revision(), speaker_scene_revision(3)] {
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
        .chain(speaker_scene_revision(3).1)
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
    view_to(w, rev, addressee, "brokkr")
}

/// As `view_of`, with referent `n3` naming `entity` (Nibu is the item's
/// speaker, so he may be named on the line; Brokkr is named by its text).
fn view_to(w: &W, rev: u32, addressee: Option<&str>, entity: &str) -> LoweringInput {
    let mut p: Packet = proof_packet();
    p.inventory_rev = rev;
    p.referents[2].entity = Some(eid(entity));
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

fn read(w: &W, view: LoweringInput) -> SceneRead {
    view.with_scene(&inv(w), &w.state, &w.corpus)
        .unwrap()
        .scene_read()
        .clone()
}

fn selected(rev: u32, features: Vec<FeatureValue>) -> SceneRead {
    SceneRead::Selected { rev, features }
}

#[test]
fn a_scene_relation_reaches_the_lowering_view_and_changes_no_english() {
    let mut w = world();
    put(&mut w, standard("g", 1));
    let plain = view_of(&w, 3, Some("n3"));
    assert_eq!(plain.scene_read(), &SceneRead::Unread);
    assert!(plain.scene().is_empty());
    let with = plain
        .clone()
        .with_scene(&inv(&w), &w.state, &w.corpus)
        .unwrap();
    assert_eq!(with.scene(), status().as_slice());
    assert_eq!(with.scene_read(), &selected(1, status()));
    assert_eq!(
        lower(&with, &inv(&w), &w.entities).unwrap(),
        lower(&plain, &inv(&w), &w.entities).unwrap()
    );
}

#[test]
fn with_scene_says_which_of_five_things_it_found() {
    let mut w = world();
    // No scene for the group.
    assert_eq!(read(&w, view_of(&w, 3, Some("n3"))), SceneRead::NoScene);
    put(&mut w, standard("g", 1));
    // A referent with an entity selects the relation to that entity; a
    // referent without one, the relation to an unnamed party; and the
    // speaker's relation, never Brokkr's to Nibu.
    assert_eq!(read(&w, view_of(&w, 3, Some("n3"))), selected(1, status()));
    assert_eq!(
        read(&w, view_of(&w, 3, Some("n1"))),
        selected(1, intimacy(7))
    );
    // The scene was read and the packet names no addressee.
    assert_eq!(
        read(&w, view_of(&w, 3, None)),
        SceneRead::NoAddressee { rev: 1 }
    );
    // The addressee is the speaker.
    assert_eq!(
        read(&w, view_to(&w, 3, Some("n3"), "nibu")),
        SceneRead::SelfAddressed { rev: 1 }
    );
    // The scene holds no relation to this addressee.
    put(
        &mut w,
        scene(
            "g",
            2,
            &["nibu", "brokkr"],
            vec![relation("nibu", Party::Unnamed, intimacy(7))],
        ),
    );
    assert_eq!(
        read(&w, view_of(&w, 3, Some("n3"))),
        SceneRead::NoRelation { rev: 2 }
    );
    // A relation with no features is found, not absent.
    put(
        &mut w,
        scene(
            "g",
            3,
            &["nibu", "brokkr"],
            vec![relation("nibu", Party::Named(eid("brokkr")), vec![])],
        ),
    );
    assert_eq!(read(&w, view_of(&w, 3, Some("n3"))), selected(3, vec![]));
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
    assert_eq!(
        read(&w, view_of(&w, 3, Some("n3"))),
        SceneRead::NoRelation { rev: 1 }
    );
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
    assert_eq!(
        read(&w, view_of(&w, 3, Some("n3"))),
        SceneRead::NoRelation { rev: 2 }
    );
}

#[test]
fn the_speaker_is_the_items_not_the_first_participant() {
    let mut w = world();
    // Brokkr is listed first and has an unnamed relation of his own.
    put(
        &mut w,
        scene(
            "g",
            1,
            &["brokkr", "nibu"],
            vec![
                relation("brokkr", Party::Unnamed, intimacy(99)),
                relation("nibu", Party::Unnamed, intimacy(7)),
            ],
        ),
    );
    assert_eq!(
        read(&w, view_of(&w, 3, Some("n1"))),
        selected(1, intimacy(7))
    );
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
    // No scene for the item's group, whatever other groups hold.
    assert_eq!(read(&w, view_of(&w, 3, Some("n3"))), SceneRead::NoScene);
    // Revisions: the highest n is read.
    put(&mut w, standard("g", 1));
    let mut second = standard("g", 2);
    second.relations[0].features = intimacy(5);
    put(&mut w, second);
    let got = view_of(&w, 3, Some("n3"))
        .with_scene(&inv(&w), &w.state, &w.corpus)
        .unwrap();
    assert_eq!(got.scene_read(), &selected(2, intimacy(5)));
    assert_eq!(got.scene(), intimacy(5).as_slice());
    // A read replaces the earlier one: a store without the scene says so.
    let empty = SpeakerStateStore::open(&w.dir.path().join("empty.cc")).unwrap();
    let cleared = got.with_scene(&inv(&w), &empty, &w.corpus).unwrap();
    assert_eq!(cleared.scene_read(), &SceneRead::NoScene);
    assert!(cleared.scene().is_empty());
}

#[test]
fn with_scene_refusals() {
    let mut w = world();
    put(&mut w, standard("g", 1));
    // The scene was admitted at revision 3; a view of revision 2 predates the
    // Scene features, so the selected relation's ids do not resolve there.
    assert_eq!(
        rules(view_of(&w, 2, Some("n3")).with_scene(&inv(&w), &w.state, &w.corpus)),
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
    for addressee in [Some("n3"), None] {
        let mut p = proof_packet();
        p.inventory_rev = 3;
        p.provenance.corpus_item = weksa::CorpusItemId::parse("item:nibu:3").unwrap();
        p.referents[2].entity = Some(eid("nibu"));
        p.discourse.addressee = addressee.map(nd);
        let scratch = tempfile::tempdir().unwrap();
        let view = PacketStore::open(&scratch.path().join("p.cc"))
            .unwrap()
            .admit(p, &w.inventory, &w.corpus, &w.entities)
            .unwrap();
        assert_eq!(
            rules(view.with_scene(&inv(&w), &w.state, &w.corpus)),
            ["V16"],
            "{addressee:?}"
        );
    }
}

#[test]
fn read_refuses_a_scene_that_stopped_covering_a_live_speaker_of_its_group() {
    // Group g's scene covers Nibu alone, and Nibu's is the only item: admitted.
    let mut w = world();
    put(&mut w, scene("g", 1, &["nibu"], vec![]));
    assert_eq!(
        read(&w, view_of(&w, 3, None)),
        SceneRead::NoAddressee { rev: 1 }
    );
    // Brokkr then speaks in g. The corpus door lets him in; the view of Nibu's
    // own line, whose speaker the scene still lists, reads the stale scene no more.
    let mut item = item_in("item:brokkr:1", "Hello.", "g", cast());
    item.speaker = eid("brokkr");
    w.corpus.add(item, &w.entities).unwrap();
    assert_eq!(
        rules(view_of(&w, 3, None).with_scene(&inv(&w), &w.state, &w.corpus)),
        ["V16"]
    );
    // The put door applies the same rule, so the repair is a revision that
    // covers him.
    put(&mut w, scene("g", 2, &["nibu", "brokkr"], vec![]));
    assert_eq!(
        read(&w, view_of(&w, 3, None)),
        SceneRead::NoAddressee { rev: 2 }
    );
}

#[test]
fn only_live_speakers_of_the_views_group_and_the_views_own_speaker_must_be_covered() {
    let mut w = world();
    put(&mut w, scene("g", 1, &["nibu"], vec![]));
    let ok = SceneRead::NoAddressee { rev: 1 };
    // A withdrawn speaker the scene lacks does not count.
    let mut gone = item_in("item:brokkr:1", "Hello.", "g", cast());
    gone.speaker = eid("brokkr");
    w.corpus.add(gone, &w.entities).unwrap();
    w.corpus.withdraw("item:brokkr:1").unwrap();
    assert_eq!(read(&w, view_of(&w, 3, None)), ok);
    // Nor does a live speaker the scene lacks in another group.
    let mut other = item_in("item:brokkr:2", "Hello you.", "h", cast());
    other.speaker = eid("brokkr");
    w.corpus.add(other, &w.entities).unwrap();
    assert_eq!(read(&w, view_of(&w, 3, None)), ok);
    // The view's own speaker must be listed even when its item was withdrawn
    // after the view was built: scene g@2 lists Brokkr alone.
    let view = view_of(&w, 3, None);
    w.corpus.withdraw("item:nibu:1").unwrap();
    put(&mut w, scene("g", 2, &["brokkr"], vec![]));
    assert_eq!(
        rules(view.with_scene(&inv(&w), &w.state, &w.corpus)),
        ["V16"]
    );
}

mod select {
    use super::*;
    use weksa::speaker::Annotator;
    use weksa::{Condition, Layer, OntologyEntry, OntologyStore, Slot, Test, select};

    fn scene_feature(gloss: &str) -> weksa::InvId {
        speaker_scene_revision(3).1.into_iter().find(|e| e.gloss == gloss).unwrap().id
    }

    fn entry(n: u32, slot: Slot, gloss: &str, test: Test) -> OntologyEntry {
        OntologyEntry {
            language: "ja-JP".into(),
            n,
            inventory_rev: 3,
            layer: Layer::Grammar,
            when: vec![Condition { slot, feature: scene_feature(gloss), test }],
            choice: "a choice".into(),
            examples: vec![],
            annotator: Annotator::Operator,
        }
    }

    #[test]
    fn select_reads_the_scene_relation() {
        let mut w = world();
        put(&mut w, standard("g", 1));
        let mut store = OntologyStore::open(&w.dir.path().join("o.cc")).unwrap();
        store
            .admit(
                vec![
                    entry(1, Slot::Scene, "relative_status", Test::Is(FeatValue::Option(lab("above")))),
                    entry(2, Slot::Scene, "intimacy", Test::AtLeast(grade(150))),
                ],
                &w.inventory,
            )
            .unwrap();
        let ontology = store.ontology().unwrap();
        let keys = |read: SceneRead, w: &W| {
            let view = view_of(w, 3, Some("n3")).with_scene(&inv(w), &w.state, &w.corpus).unwrap();
            assert_eq!(view.scene_read(), &read);
            select(&ontology, "ja-JP", &view).iter().map(|e| e.n).collect::<Vec<_>>()
        };
        // Brokkr is above-status at intimacy 200: both Scene entries select.
        assert_eq!(keys(selected(1, status()), &w), [1, 2]);
        // No scene for the group: neither selects.
        let empty = world();
        assert_eq!(keys(SceneRead::NoScene, &empty), Vec::<u32>::new());
        // No addressee, and no relation to the addressee: neither selects.
        let view = view_of(&w, 3, None).with_scene(&inv(&w), &w.state, &w.corpus).unwrap();
        assert_eq!(view.scene_read(), &SceneRead::NoAddressee { rev: 1 });
        assert!(select(&ontology, "ja-JP", &view).is_empty());
        let mut lone = world();
        put(&mut lone, scene("g", 1, &["nibu", "brokkr"], vec![relation("brokkr", Party::Named(eid("nibu")), intimacy(99))]));
        assert_eq!(keys(SceneRead::NoRelation { rev: 1 }, &lone), Vec::<u32>::new());
    }
}

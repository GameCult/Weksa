//! The speaker-state store: speaker and scene documents are append-only
//! revisions, checked against the inventory, the entity store and the corpus.

mod common;
use common::{cast, eid, entities_in, grade, item_in, lab};
use weksa::inventory::{proof_revision, seed, speaker_scene_revision};
use weksa::packet::{FeatValue, FeatureValue};
use weksa::speaker::{
    Annotator, MAX_PARTICIPANTS, PairRelation, Party, Scene, Speaker, scene_key, speaker_key,
};
use weksa::{
    AddOutcome, CorpusStore, EntityStore, InventoryStore, Refusal, SpeakerStateStore, StoreError,
    StoreRefusal,
};

struct W {
    dir: tempfile::TempDir,
    inventory: InventoryStore,
    corpus: CorpusStore,
    entities: EntityStore,
    state: SpeakerStateStore,
}

/// Group `g` holds two live items (speakers nibu and brokkr) and one withdrawn
/// item (speaker hero); group `g2` holds one live item of nibu's.
fn world() -> W {
    let dir = tempfile::tempdir().unwrap();
    let mut inventory = InventoryStore::open(&dir.path().join("i.cc")).unwrap();
    for (rev, entries) in [seed(), proof_revision(), speaker_scene_revision(3)] {
        inventory.admit_revision(rev, entries).unwrap();
    }
    let entities = entities_in(dir.path());
    let mut corpus = CorpusStore::open(&dir.path().join("c.cc")).unwrap();
    for (key, text, speaker) in [
        ("item:nibu:1", "Come here.", "nibu"),
        ("item:nibu:2", "Go there.", "brokkr"),
        ("item:nibu:3", "Stay away.", "hero"),
        ("item:nibu:4", "Elsewhere.", "g2"),
    ] {
        let mut item = item_in(key, text, "g", cast());
        item.speaker = eid(speaker);
        if key == "item:nibu:4" {
            item.group = "g2".into();
            item.speaker = eid("nibu");
        }
        corpus.add(item, &entities).unwrap();
    }
    corpus.withdraw("item:nibu:3").unwrap();
    let state = SpeakerStateStore::open(&dir.path().join("s.cc")).unwrap();
    W {
        dir,
        inventory,
        corpus,
        entities,
        state,
    }
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

fn speaker(n: u32, age: &str) -> Speaker {
    Speaker {
        entity: eid("nibu"),
        n,
        inventory_rev: 3,
        features: vec![feat("age_cohort", FeatValue::Option(lab(age)))],
        annotator: Annotator::Operator,
    }
}

fn scene(n: u32, intimacy: u16) -> Scene {
    Scene {
        group: "g".into(),
        n,
        inventory_rev: 3,
        participants: vec![eid("nibu"), eid("brokkr")],
        relations: vec![PairRelation {
            from: eid("nibu"),
            to: Party::Named(eid("brokkr")),
            features: vec![feat("intimacy", FeatValue::Grade(grade(intimacy)))],
        }],
        annotator: Annotator::Operator,
    }
}

fn put_speaker(w: &mut W, s: Speaker) -> Result<AddOutcome, StoreError> {
    w.state.put_speaker(s, &w.inventory, &w.entities)
}

fn put_scene(w: &mut W, s: Scene) -> Result<AddOutcome, StoreError> {
    w.state.put_scene(s, &w.inventory, &w.corpus, &w.entities)
}

fn refused<T: std::fmt::Debug>(r: Result<T, StoreError>) -> StoreRefusal {
    match r.unwrap_err() {
        StoreError::Refused(r) => r,
        other => panic!("not a store refusal: {other}"),
    }
}

fn malformed(r: Result<AddOutcome, StoreError>, key: &str, why: &str) {
    match refused(r) {
        StoreRefusal::Malformed { key: k, why: w } => assert_eq!((k.as_str(), w), (key, why)),
        other => panic!("not Malformed: {other:?}"),
    }
}

fn rules(r: Result<AddOutcome, StoreError>) -> Vec<&'static str> {
    match r.unwrap_err() {
        StoreError::Invalid(rs) => rs.iter().map(Refusal::rule).collect(),
        other => panic!("not a rule refusal: {other}"),
    }
}

fn not_next(key: &str, expected: u32, got: u32) -> StoreRefusal {
    StoreRefusal::RevisionNotNext {
        key: key.into(),
        expected,
        got,
    }
}

#[test]
fn keys_derive_from_the_document() {
    assert_eq!(speaker_key(&eid("nibu"), 2), "speaker:nibu@2");
    assert_eq!(scene_key("g", 3), "scene:g@3");
}

#[test]
fn speaker_revisions_append() {
    let mut w = world();
    let key = |n| speaker_key(&eid("nibu"), n);
    assert_eq!(
        refused(put_speaker(&mut w, speaker(2, "adult"))),
        not_next(&key(2), 1, 2)
    );
    assert!(w.state.speaker(&eid("nibu")).unwrap().is_none());
    assert_eq!(
        put_speaker(&mut w, speaker(1, "adult")).unwrap(),
        AddOutcome::Added
    );
    assert_eq!(
        put_speaker(&mut w, speaker(1, "adult")).unwrap(),
        AddOutcome::Unchanged
    );
    assert_eq!(
        refused(put_speaker(&mut w, speaker(1, "elder"))),
        not_next(&key(1), 2, 1)
    );
    assert_eq!(
        refused(put_speaker(&mut w, speaker(3, "elder"))),
        not_next(&key(3), 2, 3)
    );
    assert_eq!(
        put_speaker(&mut w, speaker(2, "elder")).unwrap(),
        AddOutcome::Added
    );
    assert_eq!(
        w.state.speaker(&eid("nibu")).unwrap(),
        Some(speaker(2, "elder"))
    );
    assert_eq!(
        w.state.get_speaker(&key(1)).unwrap(),
        Some(speaker(1, "adult"))
    );
    // Another entity's revisions count from 1 and stay apart.
    let mut other = speaker(1, "child");
    other.entity = eid("brokkr");
    assert_eq!(put_speaker(&mut w, other).unwrap(), AddOutcome::Added);
    assert_eq!(w.state.speaker(&eid("nibu")).unwrap().unwrap().n, 2);
    // Revision 10 sorts after 2 in the listing.
    for n in 3..=10 {
        put_speaker(&mut w, speaker(n, "adult")).unwrap();
    }
    let listed: Vec<_> = w
        .state
        .all_speakers()
        .unwrap()
        .iter()
        .map(|s| (s.entity.to_string(), s.n))
        .collect();
    assert_eq!(listed.len(), 11);
    assert_eq!(listed[0], ("entity:brokkr".into(), 1));
    assert_eq!(listed[10], ("entity:nibu".into(), 10));
    assert_eq!(w.state.speaker(&eid("nibu")).unwrap().unwrap().n, 10);
    // The documents survive a reopen.
    let again = SpeakerStateStore::open(&w.dir.path().join("s.cc")).unwrap();
    assert_eq!(again.all_speakers().unwrap().len(), 11);
}

#[test]
fn scene_revisions_append() {
    let mut w = world();
    let key = |n| scene_key("g", n);
    assert_eq!(
        refused(put_scene(&mut w, scene(2, 5))),
        not_next(&key(2), 1, 2)
    );
    assert!(w.state.scene("g").unwrap().is_none());
    assert_eq!(put_scene(&mut w, scene(1, 5)).unwrap(), AddOutcome::Added);
    assert_eq!(
        put_scene(&mut w, scene(1, 5)).unwrap(),
        AddOutcome::Unchanged
    );
    assert_eq!(
        refused(put_scene(&mut w, scene(1, 6))),
        not_next(&key(1), 2, 1)
    );
    assert_eq!(put_scene(&mut w, scene(2, 6)).unwrap(), AddOutcome::Added);
    assert_eq!(w.state.scene("g").unwrap(), Some(scene(2, 6)));
    assert_eq!(w.state.get_scene(&key(1)).unwrap(), Some(scene(1, 5)));
    for n in 3..=10 {
        put_scene(&mut w, scene(n, 6)).unwrap();
    }
    assert_eq!(w.state.scene("g").unwrap().unwrap().n, 10);
    let listed: Vec<u32> = w.state.all_scenes().unwrap().iter().map(|s| s.n).collect();
    assert_eq!(listed, (1..=10).collect::<Vec<_>>());
    // Numbering is per group: the first scene of another group is revision 1
    // whatever g holds, and its second is 2.
    let other = |n| Scene {
        group: "g2".into(),
        participants: vec![eid("nibu")],
        relations: vec![],
        ..scene(n, 1)
    };
    assert_eq!(
        refused(put_scene(&mut w, other(2))),
        not_next(&scene_key("g2", 2), 1, 2)
    );
    assert_eq!(put_scene(&mut w, other(1)).unwrap(), AddOutcome::Added);
    assert_eq!(
        refused(put_scene(&mut w, other(3))),
        not_next(&scene_key("g2", 3), 2, 3)
    );
    assert_eq!(put_scene(&mut w, other(2)).unwrap(), AddOutcome::Added);
    assert_eq!(w.state.scene("g2").unwrap().unwrap().n, 2);
    assert_eq!(w.state.scene("g").unwrap().unwrap().n, 10);
}

#[test]
fn speaker_needs_a_known_entity() {
    let mut w = world();
    let mut s = speaker(1, "adult");
    s.entity = eid("nobody");
    assert_eq!(
        refused(put_speaker(&mut w, s)),
        StoreRefusal::UnknownEntity {
            entity: eid("nobody")
        }
    );
    assert!(w.state.all_speakers().unwrap().is_empty());
}

#[test]
fn annotation_features_are_checked_like_packet_features() {
    let mut w = world();
    let age = || feat("age_cohort", FeatValue::Option(lab("adult")));
    let with = |features: Vec<FeatureValue>, rev| Speaker {
        features,
        inventory_rev: rev,
        ..speaker(1, "adult")
    };
    // A Scene feature and a Stance feature do not apply to a speaker.
    let f = vec![feat("intimacy", FeatValue::Grade(grade(1)))];
    assert_eq!(rules(put_speaker(&mut w, with(f, 3))), ["V8"]);
    let f = vec![feat("anger", FeatValue::Grade(grade(1)))];
    assert_eq!(rules(put_speaker(&mut w, with(f, 3))), ["V8"]);
    // A wrong kind and an undeclared label.
    let f = vec![feat("education", FeatValue::Flag(true))];
    assert_eq!(rules(put_speaker(&mut w, with(f, 3))), ["V7"]);
    let f = vec![feat("age_cohort", FeatValue::Option(lab("ancient")))];
    assert_eq!(rules(put_speaker(&mut w, with(f, 3))), ["V7"]);
    // Feature 22 does not exist at revision 2; revision 9 has no record.
    assert_eq!(rules(put_speaker(&mut w, with(vec![age()], 2))), ["V5"]);
    assert_eq!(rules(put_speaker(&mut w, with(vec![age()], 9))), ["V10"]);
    // A repeat.
    assert_eq!(
        rules(put_speaker(&mut w, with(vec![age(), age()], 3))),
        ["V11"]
    );
    // A Speaker feature in a scene relation.
    let mut s = scene(1, 1);
    s.relations[0].features = vec![age()];
    assert_eq!(rules(put_scene(&mut w, s)), ["V8"]);
    // Every relation is checked, and a scene of an unknown revision is V10.
    let mut s = scene(1, 1);
    s.relations.push(PairRelation {
        from: eid("brokkr"),
        to: Party::Unnamed,
        features: vec![age()],
    });
    assert_eq!(rules(put_scene(&mut w, s)), ["V8"]);
    let mut s = scene(1, 1);
    s.inventory_rev = 9;
    assert_eq!(rules(put_scene(&mut w, s)), ["V10"]);
    let mut s = scene(1, 1);
    s.inventory_rev = 9;
    s.relations.clear();
    assert_eq!(rules(put_scene(&mut w, s)), ["V10"]);
    assert!(w.state.all_speakers().unwrap().is_empty());
    assert!(w.state.all_scenes().unwrap().is_empty());
}

#[test]
fn scene_shape_rules() {
    let mut w = world();
    let key = scene_key("g", 1);
    let relation = |from: &str, to: Party| PairRelation {
        from: eid(from),
        to,
        features: vec![],
    };
    let named = |to: &str| Party::Named(eid(to));
    let base = || scene(1, 1);

    let mut s = base();
    s.group = "Not A Slug".into();
    malformed(
        put_scene(&mut w, s),
        &scene_key("Not A Slug", 1),
        "a scene group is a slug",
    );
    let mut s = base();
    s.group = "elsewhere".into();
    malformed(
        put_scene(&mut w, s),
        &scene_key("elsewhere", 1),
        "no corpus item holds the scene's group",
    );
    let mut s = base();
    s.participants.clear();
    s.relations.clear();
    malformed(
        put_scene(&mut w, s),
        &key,
        "a scene has 1 to 64 participants",
    );
    // 64 participants are fine as far as the count goes (the rest fail later);
    // 65 are refused for the count.
    let many = |count: usize| -> Vec<_> { (0..count).map(|i| eid(&format!("p{i}"))).collect() };
    let mut s = base();
    s.participants = many(MAX_PARTICIPANTS + 1);
    s.relations.clear();
    malformed(
        put_scene(&mut w, s),
        &key,
        "a scene has 1 to 64 participants",
    );
    let mut s = base();
    s.participants = many(MAX_PARTICIPANTS);
    s.relations.clear();
    assert!(matches!(
        refused(put_scene(&mut w, s)),
        StoreRefusal::UnknownEntity { .. }
    ));
    let mut s = base();
    s.participants.push(eid("nibu"));
    malformed(put_scene(&mut w, s), &key, "a participant is named once");
    let mut s = base();
    s.participants.push(eid("nobody"));
    assert_eq!(
        refused(put_scene(&mut w, s)),
        StoreRefusal::UnknownEntity {
            entity: eid("nobody")
        }
    );
    let mut s = base();
    s.relations = vec![relation("hero", named("nibu"))];
    malformed(
        put_scene(&mut w, s),
        &key,
        "a relation's from is a participant",
    );
    let mut s = base();
    s.relations = vec![relation("nibu", named("hero"))];
    malformed(
        put_scene(&mut w, s),
        &key,
        "a relation's named to is another participant",
    );
    let mut s = base();
    s.relations = vec![relation("nibu", named("nibu"))];
    malformed(
        put_scene(&mut w, s),
        &key,
        "a relation's named to is another participant",
    );
    let mut s = base();
    s.relations = vec![
        relation("nibu", named("brokkr")),
        relation("nibu", named("brokkr")),
    ];
    malformed(
        put_scene(&mut w, s),
        &key,
        "an ordered pair has one relation",
    );
    let mut s = base();
    s.relations = vec![
        relation("nibu", Party::Unnamed),
        relation("nibu", Party::Unnamed),
    ];
    malformed(
        put_scene(&mut w, s),
        &key,
        "an ordered pair has one relation",
    );
    // The reverse pair, and an unnamed party beside a named one, are distinct.
    let mut s = base();
    s.relations = vec![
        relation("nibu", named("brokkr")),
        relation("brokkr", named("nibu")),
        relation("nibu", Party::Unnamed),
    ];
    assert_eq!(put_scene(&mut w, s).unwrap(), AddOutcome::Added);
}

#[test]
fn a_scene_holds_every_live_speaker_of_its_group() {
    let mut w = world();
    // Brokkr speaks a live item of the group.
    let mut s = scene(1, 1);
    s.participants = vec![eid("nibu")];
    s.relations.clear();
    assert_eq!(
        refused(put_scene(&mut w, s)),
        StoreRefusal::SceneSpeakerMissing {
            group: "g".into(),
            entity: eid("brokkr")
        }
    );
    // Hero speaks only a withdrawn item, so the scene may omit them.
    assert_eq!(put_scene(&mut w, scene(1, 1)).unwrap(), AddOutcome::Added);
    // The group is held by a withdrawn item alone: still a group.
    w.corpus.withdraw("item:nibu:1").unwrap();
    w.corpus.withdraw("item:nibu:2").unwrap();
    let mut s = scene(2, 1);
    s.participants = vec![eid("player")];
    s.relations.clear();
    assert_eq!(put_scene(&mut w, s).unwrap(), AddOutcome::Added);
}

#[test]
fn model_annotator_attests_its_generator() {
    let mut w = world();
    let model = |generator: &str| Annotator::Model {
        family: weksa::corpus::ModelFamily::Anthropic,
        generator: generator.into(),
    };
    let key = speaker_key(&eid("nibu"), 1);
    for bad in ["", "   ", "gen\u{7}erator", "gen\nerator", "\u{200d}"] {
        let mut s = speaker(1, "adult");
        s.annotator = model(bad);
        malformed(
            put_speaker(&mut w, s),
            &key,
            "a model annotator attests its generator",
        );
        let mut s = scene(1, 1);
        s.annotator = model(bad);
        malformed(
            put_scene(&mut w, s),
            &scene_key("g", 1),
            "a model annotator attests its generator",
        );
    }
    let mut s = speaker(1, "adult");
    s.annotator = model(&"x".repeat(201));
    malformed(
        put_speaker(&mut w, s),
        &key,
        "a model annotator attests its generator",
    );
    let mut s = speaker(1, "adult");
    s.annotator = model("claude-sonnet 5.5");
    assert_eq!(put_speaker(&mut w, s).unwrap(), AddOutcome::Added);
    let mut s = scene(1, 1);
    s.annotator = model("claude-sonnet 5.5");
    assert_eq!(put_scene(&mut w, s).unwrap(), AddOutcome::Added);
    let mut s = speaker(2, "adult");
    s.annotator = Annotator::Operator;
    assert_eq!(put_speaker(&mut w, s).unwrap(), AddOutcome::Added);
}

#[test]
fn refusal_displays_print_numbers_and_no_identifiers() {
    let r = not_next("speaker:secretname@2", 1, 2);
    assert_eq!(r.to_string(), "RevisionNotNext expected 1 got 2");
    let r = StoreRefusal::SceneSpeakerMissing {
        group: "secretgroup".into(),
        entity: eid("secretname"),
    };
    assert_eq!(r.to_string(), "SceneSpeakerMissing");
}

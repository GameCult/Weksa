mod common;
use common::*;
use tempfile::TempDir;
use weksa::corpus::{CorpusItem, ModelFamily, Source, Turn};
use weksa::inventory::{Change, seed};
use weksa::packet::PacketId;
use weksa::{
    AddOutcome, CorpusStore, EntityStore, Family, InventoryEntry, InventoryRevision,
    InventoryStore, PacketStore, Refusal, Slot, SplitScheme, StoreError, StoreRefusal, ValueKind,
};

fn item(key: &str, text: &str) -> CorpusItem {
    CorpusItem {
        prior_turns: vec![Turn {
            speaker: eid("player"),
            text: "hello".into(),
        }],
        ..item_in(key, text, "scene-1", cast())
    }
}

fn refusal(r: Result<impl std::fmt::Debug, StoreError>) -> StoreRefusal {
    match r {
        Err(StoreError::Refused(r)) => r,
        other => panic!("expected a store refusal, got {other:?}"),
    }
}

fn ents(d: &TempDir) -> EntityStore {
    entities_in(d.path())
}

fn dir() -> TempDir {
    tempfile::tempdir().unwrap()
}

fn empty_corpus(d: &TempDir) -> CorpusStore {
    CorpusStore::open(&d.path().join("empty-corpus.cc")).unwrap()
}

fn malformed(key: &str, why: &'static str) -> StoreRefusal {
    StoreRefusal::Malformed {
        key: key.into(),
        why,
    }
}

#[test]
fn s1_item_immutable() {
    let d = dir();
    let path = d.path().join("c.cc");
    let mut store = CorpusStore::open(&path).unwrap();
    assert_eq!(
        store
            .add(item("item:nibu:1", "Mind the hatch."), &ents(&d))
            .unwrap(),
        AddOutcome::Added
    );
    // An equal item is idempotent, even from a reopened store.
    let mut store = CorpusStore::open(&path).unwrap();
    assert_eq!(
        store
            .add(item("item:nibu:1", "Mind the hatch."), &ents(&d))
            .unwrap(),
        AddOutcome::Unchanged
    );
    let refused = || StoreRefusal::S1ItemImmutable {
        key: "item:nibu:1".into(),
    };
    // Changed text, and every other field, is refused; the stored item stays.
    let base = item("item:nibu:1", "Mind the hatch.");
    let mut variants = vec![item("item:nibu:1", "Mind the hatch!")];
    let mut v = base.clone();
    v.source = model();
    variants.push(v);
    let mut v = base.clone();
    v.source = Source::OperatorWritten;
    variants.push(v);
    let mut v = base.clone();
    v.language = "en-GB".into();
    variants.push(v);
    let mut v = base.clone();
    v.speaker = eid("brokkr");
    variants.push(v);
    let mut v = base.clone();
    v.group = "scene-2".into();
    variants.push(v);
    let mut v = base.clone();
    v.prior_turns.clear();
    variants.push(v);
    let mut v = base.clone();
    v.admitted_on = "2026-10-30".into();
    variants.push(v);
    let mut v = base.clone();
    v.supersedes = Some("item:nibu:9".into());
    variants.push(v);
    for v in variants {
        assert_eq!(refusal(store.add(v.clone(), &ents(&d))), refused(), "{v:?}");
    }
    assert_eq!(store.get("item:nibu:1").unwrap(), Some(base.clone()));
    // A withdrawn item is a different item: re-adding it live is refused too.
    store.withdraw("item:nibu:1").unwrap();
    assert_eq!(refusal(store.add(base, &ents(&d))), refused());
}

#[test]
fn corpus_keys_are_canonical() {
    let d = dir();
    let mut store = CorpusStore::open(&d.path().join("c.cc")).unwrap();
    let long_slug = format!("item:a{}:1", "b".repeat(48));
    let not_canonical = [
        "item:a:01",
        "item:a:+1",
        "item:a:0",
        "item:A:1",
        "item:a-:1",
        "item:-a:1",
        "item:a:b:1",
        "item::1",
        "item:a:",
        "item:a:\u{661}",
        "item:a:4294967296",
        &long_slug,
    ];
    for (i, key) in not_canonical.into_iter().enumerate() {
        assert_eq!(
            refusal(store.add(item(key, &format!("line {i}")), &ents(&d))),
            malformed(key, "item key is not item:<slug>:<n>"),
            "{key}"
        );
    }
    assert!(store.all().unwrap().is_empty());
    // The edges of the canonical form are admitted.
    let longest = format!("item:{}:4294967295", "a".repeat(48));
    let canonical = ["item:a:1", "item:a-b:10", "item:7:2", "item:ab:9", &longest];
    for (i, key) in canonical.into_iter().enumerate() {
        assert_eq!(
            store
                .add(item(key, &format!("fine {i}")), &ents(&d))
                .unwrap(),
            AddOutcome::Added,
            "{key}"
        );
    }
    // An alias of a stored key is not a corpus item id at all.
    assert!(weksa::CorpusItemId::parse("item:nibu:01").is_err());
}

#[test]
fn s6_text_held_by_another_key() {
    let d = dir();
    let mut store = CorpusStore::open(&d.path().join("c.cc")).unwrap();
    store
        .add(item("item:nibu:1", "Mind the hatch."), &ents(&d))
        .unwrap();
    let elsewhere = |key: &str, text: &str| CorpusItem {
        group: "scene-2".into(),
        ..item(key, text)
    };
    let held = |key: &str, by: &str| StoreRefusal::S6TextHeld {
        key: key.into(),
        held_by: by.into(),
    };
    // The same text in another group, and in the same group, is refused.
    assert_eq!(
        refusal(store.add(elsewhere("item:nibu:2", "Mind the hatch."), &ents(&d))),
        held("item:nibu:2", "item:nibu:1")
    );
    assert_eq!(
        refusal(store.add(item("item:nibu:2", "Mind the hatch."), &ents(&d))),
        held("item:nibu:2", "item:nibu:1")
    );
    // Case, spacing and punctuation variants are the same text.
    for text in [
        "MIND THE HATCH",
        "mind   the\thatch...",
        "  Mind-the-hatch!  ",
        "Mind the hatch.\n",
    ] {
        assert_eq!(
            refusal(store.add(elsewhere("item:nibu:2", text), &ents(&d))),
            held("item:nibu:2", "item:nibu:1"),
            "{text:?}"
        );
    }
    store
        .add(
            item("item:nibu:3", "\u{c4}rger \u{fc}ber \u{d6}l"),
            &ents(&d),
        )
        .unwrap();
    assert_eq!(
        refusal(store.add(
            elsewhere("item:nibu:4", "\u{e4}rger \u{dc}BER \u{f6}l"),
            &ents(&d)
        )),
        held("item:nibu:4", "item:nibu:3")
    );
    // Text that differs in a letter or digit is a different text.
    store
        .add(item("item:nibu:5", "Mind the hatch 2"), &ents(&d))
        .unwrap();
    store
        .add(item("item:nibu:6", "Mind the hatches."), &ents(&d))
        .unwrap();
    // A withdrawn holder still holds.
    store.withdraw("item:nibu:1").unwrap();
    assert_eq!(
        refusal(store.add(elsewhere("item:nibu:2", "mind the hatch"), &ents(&d))),
        held("item:nibu:2", "item:nibu:1")
    );
    // Superseding a different key does not excuse a copy of another item.
    let mut copy = item("item:nibu:8", "Mind the hatches!");
    copy.supersedes = Some("item:nibu:5".into());
    assert_eq!(
        refusal(store.add(copy, &ents(&d))),
        held("item:nibu:8", "item:nibu:6")
    );
    // A superseder of the holder, in the holder's group, is Added.
    let mut fix = item("item:nibu:2", "mind the hatch");
    fix.supersedes = Some("item:nibu:1".into());
    assert_eq!(store.add(fix, &ents(&d)).unwrap(), AddOutcome::Added);
    // The superseded holder's text stays held against a third key.
    assert_eq!(
        refusal(store.add(elsewhere("item:nibu:7", "Mind the hatch."), &ents(&d))),
        held("item:nibu:7", "item:nibu:1")
    );
}

#[test]
fn corpus_admission_rules() {
    let d = dir();
    let mut store = CorpusStore::open(&d.path().join("c.cc")).unwrap();
    assert_eq!(
        refusal(store.add(item("nibu:1", "x"), &ents(&d))),
        malformed("nibu:1", "item key is not item:<slug>:<n>")
    );
    assert_eq!(
        refusal(store.add(item("item:nibu:x", "x"), &ents(&d))),
        malformed("item:nibu:x", "item key is not item:<slug>:<n>")
    );
    let mut born_withdrawn = item("item:nibu:1", "x");
    born_withdrawn.withdrawn = true;
    assert_eq!(
        refusal(store.add(born_withdrawn, &ents(&d))),
        malformed("item:nibu:1", "an item is admitted live, then withdrawn")
    );
    let mut orphan = item("item:nibu:1", "x");
    orphan.supersedes = Some("item:nibu:9".into());
    assert_eq!(
        refusal(store.add(orphan, &ents(&d))),
        malformed("item:nibu:1", "supersedes an item that does not exist")
    );
    // Nothing was written by the refusals.
    assert!(store.all().unwrap().is_empty());
    // A correction is a new item that supersedes the old one.
    store.add(item("item:nibu:1", "old"), &ents(&d)).unwrap();
    let mut fix = item("item:nibu:2", "new");
    fix.supersedes = Some("item:nibu:1".into());
    assert_eq!(store.add(fix, &ents(&d)).unwrap(), AddOutcome::Added);
    assert_eq!(
        refusal(store.withdraw("item:nibu:7")),
        malformed("item:nibu:7", "no such item")
    );
}

#[test]
fn s5_supersede_keeps_the_group() {
    let d = dir();
    let mut store = CorpusStore::open(&d.path().join("c.cc")).unwrap();
    store.add(item("item:nibu:1", "old"), &ents(&d)).unwrap();
    let mut elsewhere = item("item:nibu:2", "new");
    elsewhere.supersedes = Some("item:nibu:1".into());
    elsewhere.group = "scene-2".into();
    assert_eq!(
        refusal(store.add(elsewhere, &ents(&d))),
        StoreRefusal::S5SupersedeGroup {
            key: "item:nibu:2".into(),
            expected: "scene-1".into(),
            got: "scene-2".into()
        }
    );
    assert!(store.get("item:nibu:2").unwrap().is_none());
    let mut same = item("item:nibu:2", "new");
    same.supersedes = Some("item:nibu:1".into());
    assert_eq!(store.add(same, &ents(&d)).unwrap(), AddOutcome::Added);
}

#[test]
fn withdrawing_keeps_the_text_and_the_number() {
    let d = dir();
    let path = d.path().join("c.cc");
    let mut store = CorpusStore::open(&path).unwrap();
    store
        .add(item("item:nibu:1", "keep me"), &ents(&d))
        .unwrap();
    assert_eq!(store.next_key("nibu").unwrap(), "item:nibu:2");
    assert_eq!(store.next_key("other").unwrap(), "item:other:1");
    store.withdraw("item:nibu:1").unwrap();
    store.withdraw("item:nibu:1").unwrap();
    let store = CorpusStore::open(&path).unwrap();
    let got = store.get("item:nibu:1").unwrap().unwrap();
    assert!(got.withdrawn);
    assert_eq!(got.text, "keep me");
    assert_eq!(store.next_key("nibu").unwrap(), "item:nibu:2");
}

fn entries_for(rev: u32, ids: &[&str]) -> (InventoryRevision, Vec<InventoryEntry>) {
    let entries = ids.iter().map(|id| entry(id, rev)).collect();
    let changes = ids.iter().map(|id| Change::Add(iv(id))).collect();
    (
        rev_with(changes, rev, if rev == 1 { None } else { Some(rev - 1) }),
        entries,
    )
}

fn seeded(path: &std::path::Path) -> InventoryStore {
    let mut store = InventoryStore::open(path).unwrap();
    let (rev, entries) = seed();
    store.admit_revision(rev, entries).unwrap();
    store
}

#[test]
fn s2_revision_chain() {
    let d = dir();
    let mut store = InventoryStore::open(&d.path().join("i.cc")).unwrap();
    // The first revision is 1 with no parent.
    let (rev, entries) = entries_for(2, &["concept:1"]);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        StoreRefusal::S2RevisionChain {
            expected: 1,
            got: 2
        }
    );
    let (mut rev, entries) = entries_for(1, &["concept:1"]);
    rev.parent = Some(0);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        StoreRefusal::S2RevisionChain {
            expected: 1,
            got: 1
        }
    );
    let (rev, entries) = entries_for(1, &["concept:1"]);
    store.admit_revision(rev, entries).unwrap();
    // Replaying n, skipping n, or naming the wrong parent is refused.
    let (rev, entries) = entries_for(1, &["concept:2"]);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        StoreRefusal::S2RevisionChain {
            expected: 2,
            got: 1
        }
    );
    let (rev, entries) = entries_for(3, &["concept:2"]);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        StoreRefusal::S2RevisionChain {
            expected: 2,
            got: 3
        }
    );
    let (mut rev, entries) = entries_for(2, &["concept:2"]);
    rev.parent = None;
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        StoreRefusal::S2RevisionChain {
            expected: 2,
            got: 2
        }
    );
    let (rev, entries) = entries_for(2, &["concept:2"]);
    store.admit_revision(rev, entries).unwrap();
    assert_eq!(store.inventory().unwrap().latest().unwrap().n, 2);
}

#[test]
fn s3_frozen_is_final() {
    let d = dir();
    let mut store = InventoryStore::open(&d.path().join("i.cc")).unwrap();
    let (mut rev, entries) = entries_for(1, &["concept:1"]);
    rev.frozen = true;
    store.admit_revision(rev, entries).unwrap();
    let (rev, entries) = entries_for(2, &["concept:2"]);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        StoreRefusal::S3Frozen { frozen: 1 }
    );
    assert_eq!(store.inventory().unwrap().latest().unwrap().n, 1);
}

#[test]
fn s4_entry_introduced_by_its_revision() {
    let d = dir();
    let mut store = InventoryStore::open(&d.path().join("i.cc")).unwrap();
    let (rev, mut entries) = entries_for(1, &["concept:1"]);
    entries[0].introduced_rev = 2;
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        StoreRefusal::S4IntroducedRev {
            id: iv("concept:1"),
            introduced: 2,
            revision: 1
        }
    );
    assert!(store.inventory().unwrap().latest().is_none());
}

#[test]
fn revision_change_rules() {
    let d = dir();
    let mut store = seeded(&d.path().join("i.cc"));
    // An added id with no entry.
    let rev = rev_with(vec![Change::Add(iv("concept:1"))], 2, Some(1));
    assert_eq!(
        refusal(store.admit_revision(rev, vec![])),
        malformed("concept:1", "added id has no entry")
    );
    // An entry the changes do not name.
    let (rev, mut entries) = entries_for(2, &["concept:1"]);
    entries.push(entry("concept:2", 2));
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        malformed("concept:2", "entry is not in the revision's changes")
    );
    // An id already in the inventory, and one added twice.
    let (rev, entries) = entries_for(2, &["role:1"]);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        malformed("role:1", "id is already in the inventory")
    );
    let (mut rev, entries) = entries_for(2, &["concept:1"]);
    rev.changes.push(Change::Add(iv("concept:1")));
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        malformed("concept:1", "id is already in the inventory")
    );
    // A malformed entry: a feature with no value kind, a non-feature with one,
    // an extension with no value kind or with slots, an entry born retired.
    let shape = "entry shape does not match its family";
    let (rev, mut entries) = entries_for(2, &["feature:50"]);
    entries[0].applies_to = vec![Slot::Discourse];
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        malformed("feature:50", shape)
    );
    let (rev, mut entries) = entries_for(2, &["feature:50"]);
    entries[0].value_kind = Some(ValueKind::Flag);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        malformed("feature:50", shape)
    );
    let (rev, mut entries) = entries_for(2, &["concept:1"]);
    entries[0].value_kind = Some(ValueKind::Flag);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        malformed("concept:1", shape)
    );
    let (rev, entries) = entries_for(2, &["extension:1"]);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        malformed("extension:1", shape)
    );
    let (rev, mut entries) = entries_for(2, &["extension:1"]);
    entries[0].value_kind = Some(ValueKind::Flag);
    entries[0].applies_to = vec![Slot::Discourse];
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        malformed("extension:1", shape)
    );
    let (rev, mut entries) = entries_for(2, &["concept:1"]);
    entries[0].retired_rev = Some(2);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        malformed("concept:1", shape)
    );
    // Retire targets must exist and be live; an id changes once per revision.
    let rev = rev_with(vec![Change::Retire(iv("concept:9"))], 2, Some(1));
    assert_eq!(
        refusal(store.admit_revision(rev, vec![])),
        malformed("concept:9", "retire target is missing or already retired")
    );
    let rev = rev_with(
        vec![Change::Retire(iv("role:1")), Change::Retire(iv("role:1"))],
        2,
        Some(1),
    );
    assert_eq!(
        refusal(store.admit_revision(rev, vec![])),
        malformed("role:1", "id changes twice in one revision")
    );
    // Nothing above was written.
    let inv = store.inventory().unwrap();
    assert_eq!(inv.latest().unwrap().n, 1);
    assert!(inv.entry(&iv("concept:1")).is_none());
    assert!(inv.entry(&iv("role:1")).unwrap().retired_rev.is_none());
}

#[test]
fn a_well_formed_extension_entry_is_admitted() {
    let d = dir();
    let mut store = seeded(&d.path().join("i.cc"));
    let (rev, mut entries) = entries_for(2, &["extension:1"]);
    entries[0].value_kind = Some(ValueKind::Grade);
    store.admit_revision(rev, entries).unwrap();
    assert_eq!(
        store
            .inventory()
            .unwrap()
            .entry(&iv("extension:1"))
            .unwrap()
            .family(),
        Family::Extension
    );
}

#[test]
fn retiring_keeps_the_entry_for_earlier_revisions() {
    let d = dir();
    let path = d.path().join("i.cc");
    let mut store = seeded(&path);
    let (rev, entries) = entries_for(2, &["concept:1"]);
    store.admit_revision(rev, entries).unwrap();
    let retire = rev_with(vec![Change::Retire(iv("concept:1"))], 3, Some(2));
    store.admit_revision(retire, vec![]).unwrap();
    let inv = InventoryStore::open(&path).unwrap().inventory().unwrap();
    let id = iv("concept:1");
    assert_eq!(inv.entry(&id).unwrap().retired_rev, Some(3));
    assert!(inv.resolve(&id, 2).is_some());
    assert!(inv.resolve(&id, 3).is_none());
    let again = rev_with(vec![Change::Retire(id)], 4, Some(3));
    assert!(matches!(
        refusal(store.admit_revision(again, vec![])),
        StoreRefusal::Malformed { .. }
    ));
}

#[test]
fn revision_one_survives_revision_two() {
    let inv = inventory();
    let revisions: Vec<(u32, usize)> = inv.revisions().map(|r| (r.n, r.changes.len())).collect();
    assert_eq!(revisions, vec![(1, 54), (2, 6), (3, 1), (4, 1)]);
    // A packet of revision 1 is judged by the inventory of revision 1: the
    // revision is known (no V10), and it names nothing that came later.
    let mut p = good();
    p.inventory_rev = 1;
    let refusals = weksa::validate(&p, &inv).unwrap_err();
    assert!(
        refusals
            .iter()
            .all(|r| matches!(r, Refusal::V5Unresolved { .. })),
        "{refusals:?}"
    );
}

#[test]
fn the_seed_is_the_standards_enumerations_and_the_nuance_features() {
    let d = dir();
    let store = seeded(&d.path().join("i.cc"));
    let inv = store.inventory().unwrap();
    let count = |f: Family| inv.entries().filter(|e| e.family() == f).count();
    assert_eq!(count(Family::ReferentKind), 11);
    assert_eq!(count(Family::Relation), 16);
    assert_eq!(count(Family::Role), 8);
    assert_eq!(count(Family::SpeechAct), 8);
    assert_eq!(count(Family::Feature), 18);
    assert_eq!(
        count(Family::Concept) + count(Family::Frame) + count(Family::Extension),
        0
    );
    let gloss = |id: &str| inv.entry(&iv(id)).unwrap().gloss.clone();
    assert_eq!(gloss(PERSON), "person");
    assert_eq!(gloss(AGENT), "agent");
    assert_eq!(gloss(WARN), "warn");
    assert_eq!(gloss("relation:8"), "causes");
    assert_eq!(gloss(POLARITY), "polarity");
    assert_eq!(gloss(MODALITY), "modality");
    assert_eq!(gloss(HEDGE), "hedge");
    assert_eq!(gloss(IRONY), "irony");
    assert_eq!(
        inv.entry(&iv(IRONY)).unwrap().value_kind,
        Some(ValueKind::Flag)
    );
    // Felt affect and restraint: Grade features in slot Stance alone.
    for (id, name) in [
        (ANGER, "anger"),
        (AFFECTION, "affection"),
        (RESTRAINT, "restraint"),
    ] {
        let e = inv.entry(&iv(id)).unwrap();
        assert_eq!(e.gloss, name);
        assert_eq!(e.value_kind, Some(ValueKind::Grade));
        assert_eq!(e.applies_to, vec![Slot::Stance]);
    }
    let stance_names: Vec<&str> = inv
        .entries()
        .filter(|e| e.applies_to == [Slot::Stance])
        .map(|e| e.gloss.as_str())
        .collect();
    assert_eq!(
        stance_names,
        [
            "anger",
            "fear",
            "sadness",
            "joy",
            "disgust",
            "surprise",
            "contempt",
            "affection",
            "shame",
            "restraint"
        ]
    );
    // Hedge, certainty and irony stay on content; displayed formality, warmth
    // and intensity are each language's ontology's, not the seed's.
    for e in inv.entries() {
        assert!(
            !["warmth", "formality", "intensity"].contains(&e.gloss.as_str()),
            "{}",
            e.gloss
        );
    }
    assert!(
        inv.entries()
            .all(|e| e.well_formed() && e.introduced_rev == 1 && e.retired_rev.is_none())
    );
    assert!(inv.has_revision(1));
}

fn admit_one(d: &TempDir, entry: InventoryEntry) -> Result<(), StoreError> {
    let mut store = seeded(&d.path().join("i.cc"));
    let rev = rev_with(vec![Change::Add(entry.id)], 2, Some(1));
    store.admit_revision(rev, vec![entry])
}

#[test]
fn gloss_is_bounded_text() {
    let why = "gloss is 1 to 120 bytes with no control character";
    let with = |gloss: &str| {
        let d = dir();
        let mut e = entry("concept:1", 2);
        e.gloss = gloss.into();
        admit_one(&d, e)
    };
    with(&"g".repeat(120)).unwrap();
    with("two words").unwrap();
    for bad in [
        String::new(),
        "g".repeat(121),
        "a\nb".into(),
        "a\u{7}b".into(),
    ] {
        assert_eq!(refusal(with(&bad)), malformed("concept:1", why), "{bad:?}");
    }
}

#[test]
fn packet_admission_goes_through_the_validator() {
    let mut w = world();
    let mut store = PacketStore::open(&w.dir.path().join("p.cc")).unwrap();
    let mut bad = good();
    bad.schema_version = 5;
    match store.admit(bad, &w.inventory, &w.corpus, &w.entities) {
        Err(e @ StoreError::Invalid(_)) => {
            assert!(e.to_string().contains("V1"), "{e}");
            let StoreError::Invalid(r) = e else {
                unreachable!()
            };
            assert_eq!(r, vec![Refusal::V1SchemaVersion { found: 5 }]);
        }
        other => panic!("{other:?}"),
    }
    assert!(store.all().unwrap().is_empty());
    // The door yields the lowering view, stance included.
    let view = store
        .admit(good(), &w.inventory, &w.corpus, &w.entities)
        .unwrap();
    assert_eq!(view.stance(), &good().stance);
    assert!(!view.stance().features.is_empty());
    // The same packet is idempotent and yields the same view; a different one
    // under the same key is refused.
    let replay = store
        .admit(good(), &w.inventory, &w.corpus, &w.entities)
        .unwrap();
    assert_eq!(replay, view);
    let mut other = good();
    other.provenance.confidence = grade(1);
    assert_eq!(
        refusal(store.admit(other, &w.inventory, &w.corpus, &w.entities)),
        StoreRefusal::PacketImmutable {
            key: PACKET_ID.into()
        }
    );
    assert_eq!(store.all().unwrap().len(), 1);
    assert_eq!(store.get(PACKET_ID).unwrap(), Some(good()));
    // A withdrawn item admits no packet, a replay included; the packet admitted
    // before the withdrawal stays stored and readable.
    w.corpus.withdraw("item:nibu:1").unwrap();
    let mut fresh = good();
    fresh.id = PacketId::parse("packet:00000000000000aa").unwrap();
    let withdrawn = StoreRefusal::WithdrawnCorpusItem {
        key: "item:nibu:1".into(),
    };
    assert_eq!(
        refusal(store.admit(fresh, &w.inventory, &w.corpus, &w.entities)),
        withdrawn
    );
    assert_eq!(
        refusal(store.admit(good(), &w.inventory, &w.corpus, &w.entities)),
        withdrawn
    );
    assert_eq!(store.all().unwrap().len(), 1);
    assert_eq!(store.get(PACKET_ID).unwrap(), Some(good()));
}

#[test]
fn admission_reads_the_store_inventory() {
    // The store holds only revision 1: a revision-2 packet has no inventory to
    // stand on, whatever the caller believes about revision 2.
    let d = dir();
    let mut corpus = empty_corpus(&d);
    corpus
        .add(
            item_in("item:nibu:1", "Nibu says mind the hatch.", "g", cast()),
            &ents(&d),
        )
        .unwrap();
    let mut inventory = seeded(&d.path().join("i.cc"));
    let mut packets = PacketStore::open(&d.path().join("p.cc")).unwrap();
    assert_eq!(
        match packets.admit(good(), &inventory, &corpus, &ents(&d)) {
            Err(StoreError::Invalid(r)) => r,
            other => panic!("{other:?}"),
        },
        vec![Refusal::V10UnknownRevision { rev: 2 }]
    );
    assert!(packets.all().unwrap().is_empty());
    // The same packet admits once revision 2 is admitted.
    let mut ext = entry(EXT, 2);
    ext.value_kind = Some(ValueKind::Grade);
    admit(
        &mut inventory,
        2,
        vec![
            entry(CONCEPT_A, 2),
            entry(CONCEPT_B, 2),
            entry(FRAME, 2),
            ext,
        ],
    );
    packets
        .admit(good(), &inventory, &corpus, &ents(&d))
        .unwrap();
}

#[test]
fn corpus_item_must_resolve() {
    let w = world();
    let mut packets = PacketStore::open(&w.dir.path().join("p.cc")).unwrap();
    let mut p = good();
    p.provenance.corpus_item = weksa::CorpusItemId::parse("item:nibu:9").unwrap();
    assert_eq!(
        refusal(packets.admit(p, &w.inventory, &w.corpus, &w.entities)),
        StoreRefusal::UnknownCorpusItem {
            key: "item:nibu:9".into()
        }
    );
    assert!(packets.all().unwrap().is_empty());
    // Free text is not an item id, in Rust or on the wire; a packet with no item
    // cannot be built or read back.
    for bad in ["banana", "", "the reference sentence goes right here"] {
        assert!(weksa::CorpusItemId::parse(bad).is_err(), "{bad:?}");
    }
    let mut wire = serde_json::to_value(good().provenance).unwrap();
    for bad in [serde_json::Value::Null, "banana".into()] {
        wire["corpus_item"] = bad;
        assert!(serde_json::from_value::<weksa::packet::Provenance>(wire.clone()).is_err());
    }
    wire.as_object_mut().unwrap().remove("corpus_item");
    assert!(serde_json::from_value::<weksa::packet::Provenance>(wire).is_err());
}

#[test]
fn store_roundtrip() {
    let w = world();
    let d = dir();
    // A packet, an entry, a revision and an item written to temp stores read back equal.
    let packets = d.path().join("p.cc");
    let p = good();
    PacketStore::open(&packets)
        .unwrap()
        .admit(p.clone(), &w.inventory, &w.corpus, &w.entities)
        .unwrap();
    assert_eq!(
        PacketStore::open(&packets).unwrap().get(PACKET_ID).unwrap(),
        Some(p)
    );

    let inventories = d.path().join("i.cc");
    let declared = feature(
        "feature:1",
        ValueKind::Options(weksa::OptionSet::new(vec![lab("a"), lab("b")]).unwrap()),
        &[Slot::Relation, Slot::Referent],
    );
    let mut concept = entry("concept:1", 1);
    concept.gloss = "a gloss".into();
    let rev = rev_with(
        vec![Change::Add(iv("concept:1")), Change::Add(iv("feature:1"))],
        1,
        None,
    );
    InventoryStore::open(&inventories)
        .unwrap()
        .admit_revision(rev.clone(), vec![concept.clone(), declared.clone()])
        .unwrap();
    let back = InventoryStore::open(&inventories)
        .unwrap()
        .inventory()
        .unwrap();
    assert_eq!(back.entry(&iv("concept:1")), Some(&concept));
    assert_eq!(back.entry(&iv("feature:1")), Some(&declared));
    assert_eq!(back.latest(), Some(&rev));

    let corpus = d.path().join("c.cc");
    let it = item("item:nibu:1", "round trip");
    CorpusStore::open(&corpus)
        .unwrap()
        .add(it.clone(), &ents(&d))
        .unwrap();
    assert_eq!(
        CorpusStore::open(&corpus)
            .unwrap()
            .get("item:nibu:1")
            .unwrap(),
        Some(it)
    );
}

#[test]
fn the_committed_stores_load_and_hold_clean_data() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let inv = InventoryStore::open(&root.join("data/interlingua/inventory.cc"))
        .unwrap()
        .inventory()
        .unwrap();
    assert!(inv.has_revision(1));
    assert!(inv.entry(&iv(PERSON)).is_some());
    let items = CorpusStore::open(&root.join("data/corpus/corpus.cc"))
        .unwrap()
        .all()
        .unwrap();
    assert!(items.len() >= 3, "{} items", items.len());
    for i in &items {
        assert!(
            !i.text.trim().is_empty() && i.language == "en-US" && !i.withdrawn,
            "{i:?}"
        );
        if matches!(i.source, Source::ModelWritten { .. }) {
            assert_ne!(
                SplitScheme::V1.split(i),
                Some(weksa::Split::Test),
                "{}",
                i.id
            );
        }
    }
    let by_id = |id: &str| {
        items
            .iter()
            .find(|i| i.id == id)
            .unwrap_or_else(|| panic!("{id}"))
    };
    // brokkr:1 carries the generator string its fixture wrote, verbatim.
    let fixture = std::fs::read_to_string(
        root.join("examples/persona-lowering/brokkr-edda-voice/lowered-output.yaml"),
    )
    .unwrap();
    let written = fixture
        .lines()
        .find_map(|l| l.strip_prefix("generator: "))
        .expect("the fixture names its generator");
    assert_eq!(written, "codex-local-source-aware-draft");
    assert_eq!(
        by_id("item:brokkr:1").source,
        Source::ModelWritten {
            family: ModelFamily::OpenAi,
            generator: Some(written.into()),
            prompt_digest: None
        }
    );
    // Fixtures that record no author are Unrecorded, never CastAuthored.
    let unrecorded = Source::ModelWritten {
        family: ModelFamily::Unrecorded,
        generator: None,
        prompt_digest: None,
    };
    assert_eq!(by_id("item:nibu:1").source, unrecorded);
    assert_eq!(by_id("item:nibu:2").source, unrecorded);
    assert!(
        !items
            .iter()
            .any(|i| matches!(i.source, Source::CastAuthored { .. })),
        "no item claims a recorded human author"
    );
    assert_eq!(items.len(), 3);
}

#[test]
fn seed_reseeded() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let entities = EntityStore::open(&root.join("data/entity/entity.cc")).unwrap();
    let ids: Vec<String> = entities
        .all()
        .unwrap()
        .iter()
        .map(|e| e.id.to_string())
        .collect();
    assert_eq!(ids, ["entity:brokkr", "entity:nibu"]);
    // The committed corpus replays through the door with its entities: every
    // speaker resolves and every span is a stored form.
    let items = CorpusStore::open(&root.join("data/corpus/corpus.cc"))
        .unwrap()
        .all()
        .unwrap();
    assert_eq!(items.len(), 3);
    // The committed inventory is the seed, reseeded: stance features in, the
    // displayed-affect features out.
    let committed = InventoryStore::open(&root.join("data/interlingua/inventory.cc"))
        .unwrap()
        .inventory()
        .unwrap();
    let (rev, entries) = seed();
    assert!(committed.has_revision(rev.n));
    let mut have: Vec<_> = committed.entries().cloned().collect();
    let mut want = entries;
    have.sort_by_key(|e| e.id);
    want.sort_by_key(|e| e.id);
    assert_eq!(have, want);
    let d = dir();
    let mut replay = CorpusStore::open(&d.path().join("c.cc")).unwrap();
    for i in items {
        assert!(entities.get(&i.speaker).unwrap().is_some(), "{}", i.id);
        replay.add(i, &entities).unwrap();
    }
}

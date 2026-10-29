mod common;
use common::*;
use tempfile::TempDir;
use weksa::corpus::{CorpusItem, Source, Turn};
use weksa::inventory::{Change, seed};
use weksa::{
    AddOutcome, CorpusStore, Family, InventoryEntry, InventoryRevision, InventoryStore,
    PacketStore, Refusal, Slot, SplitScheme, StoreError, StoreRefusal, ValueKind,
};

fn item(key: &str, text: &str) -> CorpusItem {
    CorpusItem {
        prior_turns: vec![Turn {
            speaker: "player".into(),
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
fn s1_item_text_immutable() {
    let d = dir();
    let path = d.path().join("c.cc");
    let mut store = CorpusStore::open(&path).unwrap();
    assert_eq!(
        store.add(item("item:nibu:1", "Mind the hatch.")).unwrap(),
        AddOutcome::Added
    );
    // The same text is idempotent, even from a reopened store.
    let mut store = CorpusStore::open(&path).unwrap();
    assert_eq!(
        store.add(item("item:nibu:1", "Mind the hatch.")).unwrap(),
        AddOutcome::Unchanged
    );
    assert_eq!(
        refusal(store.add(item("item:nibu:1", "Mind the hatch!"))),
        StoreRefusal::S1ItemTextImmutable {
            key: "item:nibu:1".into()
        }
    );
    assert_eq!(
        store.get("item:nibu:1").unwrap().unwrap().text,
        "Mind the hatch."
    );
}

#[test]
fn corpus_admission_rules() {
    let d = dir();
    let mut store = CorpusStore::open(&d.path().join("c.cc")).unwrap();
    assert_eq!(
        refusal(store.add(item("nibu:1", "x"))),
        malformed("nibu:1", "item key is not item:<slug>:<n>")
    );
    assert_eq!(
        refusal(store.add(item("item:nibu:x", "x"))),
        malformed("item:nibu:x", "item key is not item:<slug>:<n>")
    );
    let mut born_withdrawn = item("item:nibu:1", "x");
    born_withdrawn.withdrawn = true;
    assert_eq!(
        refusal(store.add(born_withdrawn)),
        malformed("item:nibu:1", "an item is admitted live, then withdrawn")
    );
    let mut orphan = item("item:nibu:1", "x");
    orphan.supersedes = Some("item:nibu:9".into());
    assert_eq!(
        refusal(store.add(orphan)),
        malformed("item:nibu:1", "supersedes an item that does not exist")
    );
    // Nothing was written by the refusals.
    assert!(store.all().unwrap().is_empty());
    // A correction is a new item that supersedes the old one.
    store.add(item("item:nibu:1", "old")).unwrap();
    let mut fix = item("item:nibu:2", "new");
    fix.supersedes = Some("item:nibu:1".into());
    assert_eq!(store.add(fix).unwrap(), AddOutcome::Added);
    assert_eq!(
        refusal(store.withdraw("item:nibu:7")),
        malformed("item:nibu:7", "no such item")
    );
}

#[test]
fn s5_supersede_keeps_the_group() {
    let d = dir();
    let mut store = CorpusStore::open(&d.path().join("c.cc")).unwrap();
    store.add(item("item:nibu:1", "old")).unwrap();
    let mut elsewhere = item("item:nibu:2", "new");
    elsewhere.supersedes = Some("item:nibu:1".into());
    elsewhere.group = "scene-2".into();
    assert_eq!(
        refusal(store.add(elsewhere)),
        StoreRefusal::S5SupersedeGroup {
            key: "item:nibu:2".into(),
            expected: "scene-1".into(),
            got: "scene-2".into()
        }
    );
    assert!(store.get("item:nibu:2").unwrap().is_none());
    let mut same = item("item:nibu:2", "new");
    same.supersedes = Some("item:nibu:1".into());
    assert_eq!(store.add(same).unwrap(), AddOutcome::Added);
}

#[test]
fn withdrawing_keeps_the_text_and_the_number() {
    let d = dir();
    let path = d.path().join("c.cc");
    let mut store = CorpusStore::open(&path).unwrap();
    store.add(item("item:nibu:1", "keep me")).unwrap();
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
    assert_eq!(count(Family::Feature), 11);
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
    let w = world();
    let mut store = PacketStore::open(&w.dir.path().join("p.cc")).unwrap();
    let mut bad = good();
    bad.schema_version = 5;
    match store.admit(bad, &w.inventory, &w.corpus) {
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
    store.admit(good(), &w.inventory, &w.corpus).unwrap();
    // The same packet is idempotent; a different one under the same key is refused.
    store.admit(good(), &w.inventory, &w.corpus).unwrap();
    let mut other = good();
    other.provenance.confidence = grade(1);
    assert_eq!(
        refusal(store.admit(other, &w.inventory, &w.corpus)),
        StoreRefusal::PacketImmutable {
            key: PACKET_ID.into()
        }
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
        .add(item_in(
            "item:nibu:1",
            "Nibu says mind the hatch.",
            "g",
            cast(),
        ))
        .unwrap();
    let mut inventory = seeded(&d.path().join("i.cc"));
    let mut packets = PacketStore::open(&d.path().join("p.cc")).unwrap();
    assert_eq!(
        match packets.admit(good(), &inventory, &corpus) {
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
    packets.admit(good(), &inventory, &corpus).unwrap();
}

#[test]
fn corpus_item_must_resolve() {
    let w = world();
    let mut packets = PacketStore::open(&w.dir.path().join("p.cc")).unwrap();
    for key in ["item:nibu:9", "banana"] {
        let mut p = good();
        p.provenance.corpus_item = Some(key.into());
        assert_eq!(
            refusal(packets.admit(p, &w.inventory, &w.corpus)),
            StoreRefusal::UnknownCorpusItem { key: key.into() }
        );
    }
    assert!(packets.all().unwrap().is_empty());
    // A packet minted for no item needs none.
    let mut p = good();
    p.provenance.corpus_item = None;
    packets.admit(p, &w.inventory, &w.corpus).unwrap();
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
        .admit(p.clone(), &w.inventory, &w.corpus)
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
    CorpusStore::open(&corpus).unwrap().add(it.clone()).unwrap();
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
    assert!(
        items
            .iter()
            .any(|i| matches!(i.source, Source::ModelWritten { .. })),
        "model-written lines are labelled"
    );
}

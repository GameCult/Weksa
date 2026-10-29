#![allow(dead_code)]

use std::sync::OnceLock;

use tempfile::TempDir;
use weksa::corpus::ModelFamily;
use weksa::inventory::{Change, seed};
use weksa::packet::*;
use weksa::{
    CorpusItem, CorpusItemId, CorpusStore, Entity, EntityOrigin, EntityStore, InvId, Inventory,
    InventoryEntry, InventoryRevision, InventoryStore, LanguageForms, NameSpan, OptionLabel, Slot,
    Source, SplitScheme, ValueKind,
};

pub const PERSON: &str = "referent_kind:1";
pub const AGENT: &str = "role:1";
pub const PATIENT: &str = "role:2";
pub const WARN: &str = "speech_act:5";
pub const POLARITY: &str = "feature:1";
pub const MODALITY: &str = "feature:4";
pub const HEDGE: &str = "feature:6";
pub const IRONY: &str = "feature:8";
/// Stance features, slot Stance only (Grade).
pub const ANGER: &str = "feature:9";
pub const AFFECTION: &str = "feature:16";
pub const RESTRAINT: &str = "feature:18";
/// Introduced at rev 2.
pub const CONCEPT_A: &str = "concept:1";
pub const CONCEPT_B: &str = "concept:2";
/// Introduced at rev 2, retired at rev 3.
pub const CONCEPT_RETIRED: &str = "concept:3";
/// Introduced at rev 4.
pub const CONCEPT_LATE: &str = "concept:4";
pub const FRAME: &str = "frame:1";
/// An extension entry of value kind Grade, introduced at rev 2.
pub const EXT: &str = "extension:1";
/// A Grade feature introduced at rev 2 that sits in every slot.
pub const FEAT_ALL: &str = "feature:50";

pub const PACKET_ID: &str = "packet:0123456789abcdef";
pub const ENCODER_ID: &str = "encoder:fedcba9876543210";

pub fn iv(s: &str) -> InvId {
    s.parse().unwrap()
}

pub fn nd(s: &str) -> NodeId {
    NodeId::parse(s).unwrap()
}

pub fn lab(s: &str) -> OptionLabel {
    OptionLabel::parse(s).unwrap()
}

pub fn grade(v: u16) -> Grade {
    Grade::new(v).unwrap()
}

pub fn group_in_bucket(want: u64) -> String {
    (0..100_000)
        .map(|i| format!("g{i}"))
        .find(|g| SplitScheme::V1.bucket(g) == want)
        .unwrap_or_else(|| panic!("no group lands in bucket {want}"))
}

pub fn cast() -> Source {
    Source::CastAuthored {
        persona: "nibu".into(),
        fixture: "f.yaml".into(),
    }
}

pub fn model() -> Source {
    Source::ModelWritten {
        family: ModelFamily::Unrecorded,
        generator: None,
        prompt_digest: None,
    }
}

/// Every speaker the tests use, as entities with one en-US form.
const SPEAKERS: [&str; 6] = ["nibu", "brokkr", "player", "s", "someone", "hero"];

pub fn eid(slug: &str) -> EntityId {
    EntityId::parse(&format!("entity:{slug}")).unwrap()
}

/// A cast entity with `forms` in `language`, citation form first.
pub fn ent(slug: &str, language: &str, forms: &[&str]) -> Entity {
    Entity {
        id: eid(slug),
        forms: vec![LanguageForms {
            language: language.into(),
            forms: forms.iter().map(|f| f.to_string()).collect(),
        }],
        origin: EntityOrigin::Cast {
            persona: slug.into(),
        },
    }
}

/// The entity store at `entities.cc` under `dir`, holding every test speaker
/// (en-US form: the slug capitalized). Reopening replays identically.
pub fn entities_in(dir: &std::path::Path) -> EntityStore {
    let mut store = EntityStore::open(&dir.join("entities.cc")).unwrap();
    for slug in SPEAKERS {
        let form = slug[..1].to_uppercase() + &slug[1..];
        store.put(ent(slug, "en-US", &[&form])).unwrap();
    }
    store
}

pub fn item_in(key: &str, text: &str, group: &str, source: Source) -> CorpusItem {
    CorpusItem {
        id: key.into(),
        text: text.into(),
        language: "en-US".into(),
        speaker: eid("nibu"),
        names: vec![],
        group: group.into(),
        prior_turns: vec![],
        source,
        admitted_on: "2026-10-29".into(),
        supersedes: None,
        withdrawn: false,
    }
}

pub fn entry(id: &str, introduced: u32) -> InventoryEntry {
    InventoryEntry {
        id: iv(id),
        gloss: id.into(),
        value_kind: None,
        applies_to: vec![],
        introduced_rev: introduced,
        retired_rev: None,
    }
}

pub fn feature(id: &str, kind: ValueKind, slots: &[Slot]) -> InventoryEntry {
    InventoryEntry {
        value_kind: Some(kind),
        applies_to: slots.to_vec(),
        ..entry(id, 1)
    }
}

pub fn rev_with(changes: Vec<Change>, n: u32, parent: Option<u32>) -> InventoryRevision {
    InventoryRevision {
        n,
        parent,
        changes,
        frozen: false,
    }
}

/// Admits `entries` as revision `n` and returns nothing; the stores are the only door.
pub fn admit(store: &mut InventoryStore, n: u32, entries: Vec<InventoryEntry>) {
    let changes = entries.iter().map(|e| Change::Add(e.id)).collect();
    let parent = n.checked_sub(1).filter(|p| *p > 0);
    store
        .admit_revision(rev_with(changes, n, parent), entries)
        .unwrap();
}

/// Stores a test can admit through: a corpus holding `item:nibu:1` in a Train
/// group, and an inventory of revisions 1 (the seed), 2 (concepts A, B,
/// RETIRED, a frame, a Grade extension), 3 (RETIRED retired) and 4 (LATE).
pub struct World {
    pub dir: TempDir,
    pub inventory: InventoryStore,
    pub corpus: CorpusStore,
    pub entities: EntityStore,
}

impl World {
    pub fn inv(&self) -> Inventory {
        self.inventory.inventory().unwrap()
    }
}

pub fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let mut corpus = CorpusStore::open(&dir.path().join("c.cc")).unwrap();
    let entities = entities_in(dir.path());
    let train = group_in_bucket(10);
    let mut first = item_in("item:nibu:1", "Nibu says mind the hatch.", &train, cast());
    first.names = vec![NameSpan {
        entity: eid("nibu"),
        start: 0,
        end: 4,
    }];
    corpus.add(first, &entities).unwrap();
    let mut inventory = InventoryStore::open(&dir.path().join("i.cc")).unwrap();
    let (rev1, entries) = seed();
    inventory.admit_revision(rev1, entries).unwrap();
    let mut ext = entry(EXT, 2);
    ext.value_kind = Some(ValueKind::Grade);
    admit(
        &mut inventory,
        2,
        vec![
            entry(CONCEPT_A, 2),
            entry(CONCEPT_B, 2),
            entry(CONCEPT_RETIRED, 2),
            entry(FRAME, 2),
            ext,
            InventoryEntry {
                introduced_rev: 2,
                ..feature(
                    FEAT_ALL,
                    ValueKind::Grade,
                    &[
                        Slot::Discourse,
                        Slot::Referent,
                        Slot::Predication,
                        Slot::Relation,
                        Slot::Stance,
                    ],
                )
            },
        ],
    );
    inventory
        .admit_revision(
            rev_with(vec![Change::Retire(iv(CONCEPT_RETIRED))], 3, Some(2)),
            vec![],
        )
        .unwrap();
    admit(&mut inventory, 4, vec![entry(CONCEPT_LATE, 4)]);
    World {
        dir,
        inventory,
        corpus,
        entities,
    }
}

/// The world's inventory, built once.
pub fn inventory() -> Inventory {
    static INV: OnceLock<Inventory> = OnceLock::new();
    INV.get_or_init(|| world().inv()).clone()
}

pub fn good() -> Packet {
    Packet {
        id: PacketId::parse(PACKET_ID).unwrap(),
        schema_version: SCHEMA_VERSION,
        inventory_rev: 2,
        kind: PacketKind::DialogueLine,
        provenance: Provenance {
            source: ProvenanceSource::Authored,
            corpus_item: CorpusItemId::parse("item:nibu:1").unwrap(),
            encoder: EncoderId::parse(ENCODER_ID).unwrap(),
            confidence: grade(900),
        },
        context: Context {
            setting: Some(iv(CONCEPT_A)),
            medium: Some(iv(CONCEPT_B)),
            participants: vec![nd("n1")],
            audience: vec![nd("n1")],
        },
        discourse: Discourse {
            speech_act: Some(iv(WARN)),
            addressee: Some(nd("n1")),
            features: vec![FeatureValue {
                feature: iv(HEDGE),
                value: FeatValue::Grade(grade(300)),
            }],
        },
        referents: vec![Referent {
            id: nd("n1"),
            kind: iv(PERSON),
            concept: iv(CONCEPT_A),
            entity: Some(eid("nibu")),
            features: vec![],
        }],
        predications: vec![Predication {
            id: nd("n2"),
            concept: iv(CONCEPT_B),
            frame: iv(FRAME),
            roles: vec![
                RoleFill {
                    role: iv(AGENT),
                    filler: nd("n1"),
                },
                RoleFill {
                    role: iv(PATIENT),
                    filler: nd("n1"),
                },
            ],
            features: vec![FeatureValue {
                feature: iv(MODALITY),
                value: FeatValue::Option(lab("potential")),
            }],
        }],
        relations: vec![Relation {
            relation: iv("relation:8"),
            from: nd("n2"),
            to: nd("n1"),
            features: vec![],
        }],
        constraints: Constraints {
            preserve: vec![nd("n1")],
            deterministic: true,
            max_latency_ms: Some(50),
        },
        extensions: vec![Extension {
            key: iv(EXT),
            value: FeatValue::Grade(grade(3)),
        }],
        trace: Trace {
            notes: vec!["a note".into()],
            rejected_readings: vec![],
        },
        stance: Stance {
            features: vec![
                FeatureValue {
                    feature: iv(ANGER),
                    value: FeatValue::Grade(grade(300)),
                },
                FeatureValue {
                    feature: iv(RESTRAINT),
                    value: FeatValue::Grade(grade(700)),
                },
            ],
        },
    }
}

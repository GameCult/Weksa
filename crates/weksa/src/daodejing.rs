//! The Dao De Jing test bed: inventory revision 3 of the `data/daodejing`
//! stores and the packets for the slice of James Legge's chapters in its corpus.
//! It follows `proof_packet`'s precedent: a hand-authored packet the stores
//! admit through the same doors as any other.
//!
//! The ids carry the leak labels the golf loop needs. A frame minted while
//! reading a Train chapter is generic evidence; one minted for a Validation or
//! Test chapter was written with that chapter's reference in view, so a score
//! on it measures the lowerer table's reach and not the interlingua's.

use std::num::NonZeroU32;

use crate::corpus::CorpusItemId;
use crate::inventory::{Change, Family, InvId, InventoryEntry, InventoryRevision};
use crate::packet::{
    Constraints, Context, Discourse, EncoderId, FeatValue, FeatureValue, Grade, NodeId, Packet,
    PacketId, PacketKind, Predication, Provenance, ProvenanceSource, Referent, RoleFill,
    SCHEMA_VERSION, Stance, Trace,
};

const CONCEPTS: &[&str] = &[
    "sage",
    "mind",
    "belly",
    "will",
    "bone",
    "empty",
    "fill",
    "weaken",
    "strengthen",
    "heaven",
    "earth",
    "is_long_enduring",
    "continues_long",
    "highest_excellence",
    "water",
    "true_words",
    "seem_paradoxical",
    "space_between_heaven_and_earth",
    "bellows",
    "compare",
    "resemble",
];

/// Frame glosses, from `frame:4`. 4 is minted from a Train chapter, 5 and 6
/// from Validation chapters, 7 and 8 from Test chapters.
const FRAMES: &[&str] = &[
    "act",
    "intransitive",
    "likeness",
    "seems_paradoxical",
    "may_be_compared",
];

/// Revision 3 of the Dao De Jing stores, after the seed and the proof revision.
pub fn revision() -> (InventoryRevision, Vec<InventoryEntry>) {
    let mut entries = Vec::new();
    for (family, glosses) in [(Family::Concept, CONCEPTS), (Family::Frame, FRAMES)] {
        let first = if family == Family::Concept { 7 } else { 4 };
        for (i, gloss) in glosses.iter().enumerate() {
            entries.push(InventoryEntry {
                id: family.id(NonZeroU32::new(first + i as u32).expect("nonzero")),
                gloss: (*gloss).to_string(),
                value_kind: None,
                applies_to: vec![],
                introduced_rev: 3,
                retired_rev: None,
            });
        }
    }
    let changes = entries.iter().map(|e| Change::Add(e.id)).collect();
    (
        InventoryRevision {
            n: 3,
            parent: Some(2),
            changes,
            frozen: false,
        },
        entries,
    )
}

fn concept(gloss: &str) -> &'static str {
    let at = CONCEPTS
        .iter()
        .position(|c| *c == gloss)
        .expect("known concept");
    Box::leak(format!("concept:{}", 7 + at).into_boxed_str())
}

fn frame(gloss: &str) -> &'static str {
    let at = FRAMES
        .iter()
        .position(|c| *c == gloss)
        .expect("known frame");
    Box::leak(format!("frame:{}", 4 + at).into_boxed_str())
}

fn hex8(ascii: &str) -> String {
    assert_eq!(ascii.len(), 8);
    ascii.bytes().map(|b| format!("{b:02x}")).collect()
}

struct Spec {
    /// Eight ASCII characters; the packet id is their hex.
    id: &'static str,
    item: &'static str,
    speech_act: &'static str,
    /// `(node, referent kind id, concept gloss)`.
    referents: &'static [(&'static str, &'static str, &'static str)],
    /// `(node, concept gloss, frame gloss, [(role id, filler node)])`.
    predications: &'static [(
        &'static str,
        &'static str,
        &'static str,
        &'static [(&'static str, &'static str)],
    )],
    /// The author's reading of the speaker's felt state: `(feature gloss, permille)`.
    stance: &'static [(&'static str, u16)],
    note: &'static str,
}

const AGENT: &str = "role:1";
const PATIENT: &str = "role:2";
const EXPERIENCER: &str = "role:3";
const GOAL: &str = "role:6";
const PERSON: &str = "referent_kind:1";
const ORGANISM: &str = "referent_kind:3";
const PLACE: &str = "referent_kind:4";
const ABSTRACT: &str = "referent_kind:11";

/// Feature ids of the seed: restraint is the tenth stance feature.
fn stance_feature(gloss: &str) -> InvId {
    let n = [
        "anger",
        "fear",
        "sadness",
        "joy",
        "disgust",
        "surprise",
        "contempt",
        "affection",
        "shame",
        "restraint",
    ]
    .iter()
    .position(|g| *g == gloss)
    .expect("stance feature");
    // Features before the stance block: polarity, tense, aspect, modality,
    // evidentiality, hedge, certainty, irony.
    InvId::parse(&format!("feature:{}", 9 + n)).expect("feature id")
}

const SPECS: &[Spec] = &[
    // Chapter 3 section 2. Train.
    Spec {
        id: "dao-0302",
        item: "item:daodejing:1",
        speech_act: "speech_act:1",
        referents: &[
            ("n1", PERSON, "sage"),
            ("n2", ABSTRACT, "mind"),
            ("n3", ORGANISM, "belly"),
            ("n4", ABSTRACT, "will"),
            ("n5", ORGANISM, "bone"),
        ],
        predications: &[
            ("n6", "empty", "act", &[(AGENT, "n1"), (PATIENT, "n2")]),
            ("n7", "fill", "act", &[(AGENT, "n1"), (PATIENT, "n3")]),
            ("n8", "weaken", "act", &[(AGENT, "n1"), (PATIENT, "n4")]),
            ("n9", "strengthen", "act", &[(AGENT, "n1"), (PATIENT, "n5")]),
        ],
        stance: &[("restraint", 800)],
        note: "Four acts of one agent; 'in the exercise of his government' and 'their' are not carried.",
    },
    // Chapter 7 section 1, first sentence. Validation.
    Spec {
        id: "dao-0701",
        item: "item:daodejing:2",
        speech_act: "speech_act:1",
        referents: &[("n1", PLACE, "heaven"), ("n2", PLACE, "earth")],
        predications: &[
            (
                "n3",
                "is_long_enduring",
                "intransitive",
                &[(EXPERIENCER, "n1")],
            ),
            (
                "n4",
                "continues_long",
                "intransitive",
                &[(EXPERIENCER, "n2")],
            ),
        ],
        stance: &[("restraint", 800)],
        note: "Heaven and earth as places; whether they are entities is a ruling the ontology owes.",
    },
    // Chapter 8 section 1, first sentence. Validation.
    Spec {
        id: "dao-0801",
        item: "item:daodejing:3",
        speech_act: "speech_act:1",
        referents: &[
            ("n1", ABSTRACT, "highest_excellence"),
            ("n2", ABSTRACT, "water"),
        ],
        predications: &[(
            "n3",
            "resemble",
            "likeness",
            &[(EXPERIENCER, "n1"), (GOAL, "n2")],
        )],
        stance: &[("restraint", 800), ("affection", 300)],
        note: "The metaphor is a plain likeness predication; the tenor and vehicle are not marked.",
    },
    // Chapter 78 section 4. Test.
    Spec {
        id: "dao-7804",
        item: "item:daodejing:4",
        speech_act: "speech_act:1",
        referents: &[("n1", ABSTRACT, "true_words")],
        predications: &[(
            "n2",
            "seem_paradoxical",
            "seems_paradoxical",
            &[(EXPERIENCER, "n1")],
        )],
        stance: &[("restraint", 800)],
        note: "The whole paradox is in the referent's concept and the frame; the packet holds no more than the surface does.",
    },
    // Chapter 5 section 2, first sentence. Test.
    Spec {
        id: "dao-0502",
        item: "item:daodejing:5",
        speech_act: "speech_act:2",
        referents: &[
            ("n1", PLACE, "space_between_heaven_and_earth"),
            ("n2", ARTIFACT, "bellows"),
        ],
        predications: &[(
            "n3",
            "compare",
            "may_be_compared",
            &[(EXPERIENCER, "n1"), (GOAL, "n2")],
        )],
        stance: &[("restraint", 800)],
        note: "A rhetorical question; the speech act is ask and the negated 'may not' is dropped.",
    },
];

const ARTIFACT: &str = "referent_kind:5";

/// The packets of the slice, each minted for its corpus item.
pub fn packets() -> Vec<Packet> {
    SPECS.iter().map(build).collect()
}

fn build(spec: &Spec) -> Packet {
    let id = |s: &str| InvId::parse(s).expect("ids are well formed");
    let node = |s: &str| NodeId::parse(s).expect("nodes are well formed");
    Packet {
        id: PacketId::parse(&format!("packet:{}", hex8(spec.id))).expect("packet id"),
        schema_version: SCHEMA_VERSION,
        inventory_rev: 3,
        kind: PacketKind::CorpusExample,
        provenance: Provenance {
            source: ProvenanceSource::Authored,
            corpus_item: CorpusItemId::parse(spec.item).expect("item key"),
            encoder: EncoderId::parse(&format!("encoder:{}", hex8("claude-s"))).expect("encoder"),
            confidence: Grade::new(600).expect("grade"),
        },
        context: Context::default(),
        discourse: Discourse {
            speech_act: Some(id(spec.speech_act)),
            addressee: None,
            addressee_consent: None,
            features: vec![],
        },
        referents: spec
            .referents
            .iter()
            .map(|(n, kind, gloss)| Referent {
                id: node(n),
                kind: id(kind),
                concept: id(concept(gloss)),
                entity: None,
                features: vec![],
            })
            .collect(),
        predications: spec
            .predications
            .iter()
            .map(|(n, c, f, roles)| Predication {
                id: node(n),
                concept: id(concept(c)),
                frame: id(frame(f)),
                roles: roles
                    .iter()
                    .map(|(role, filler)| RoleFill {
                        role: id(role),
                        filler: node(filler),
                    })
                    .collect(),
                features: vec![],
            })
            .collect(),
        relations: vec![],
        constraints: Constraints::default(),
        extensions: vec![],
        trace: Trace {
            notes: vec![spec.note.to_string()],
            rejected_readings: vec![],
        },
        stance: Stance {
            features: spec
                .stance
                .iter()
                .map(|(g, v)| FeatureValue {
                    feature: stance_feature(g),
                    value: FeatValue::Grade(Grade::new(*v).expect("grade")),
                })
                .collect(),
        },
    }
}

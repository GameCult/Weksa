//! Error text is a fixed message naming the rule or variant. The caller's
//! identifiers (entity, node and inventory ids, IPA text, value strings) stay
//! on the typed value and never reach Display.

use std::collections::HashSet;
use weksa::packet::{EntityId, NodeId, ProvenanceSource};
use weksa::{Family, InvId, LowerError, Refusal, Slot};

const SENTINEL: &str = "sentinelzz";

fn inv() -> InvId {
    InvId::parse("concept:4242").unwrap()
}
fn node() -> NodeId {
    NodeId::parse("n4242").unwrap()
}
fn entity() -> EntityId {
    EntityId::parse(&format!("entity:{SENTINEL}")).unwrap()
}

fn assert_clean_and_distinct<T: std::fmt::Display + std::fmt::Debug>(all: Vec<T>) {
    let mut seen = HashSet::new();
    for e in all {
        let text = e.to_string();
        assert!(!text.is_empty(), "{e:?}");
        for leak in [SENTINEL, "4242", "{", "}"] {
            assert!(!text.contains(leak), "{e:?} leaks {leak:?}: {text}");
        }
        assert!(seen.insert(text.clone()), "{e:?} repeats {text:?}");
    }
}

#[test]
fn lower_error_display_names_the_variant_and_no_identifier() {
    assert_clean_and_distinct(vec![
        LowerError::Unrendered { what: SENTINEL },
        LowerError::NoRow { id: inv() },
        LowerError::GlossMismatch {
            id: inv(),
            row: SENTINEL,
        },
        LowerError::UnknownRole {
            frame: inv(),
            role: SENTINEL.into(),
        },
        LowerError::MissingRole {
            predication: node(),
            role: SENTINEL.into(),
        },
        LowerError::UnknownEntity { entity: entity() },
        LowerError::NoForm { entity: entity() },
        LowerError::NoPronunciation { entity: entity() },
        LowerError::NoEntityReferent,
        LowerError::UnknownPhone {
            entity: entity(),
            at: 4242,
            text: SENTINEL.into(),
        },
        LowerError::DanglingStress {
            entity: entity(),
            at: 4242,
        },
        LowerError::ConsentMissing {
            predication: node(),
        },
        LowerError::AskerNotAddressee {
            predication: node(),
        },
        LowerError::EntityRead {
            entity: entity(),
            why: SENTINEL.into(),
        },
    ]);
    assert_eq!(
        LowerError::NoPronunciation { entity: entity() }.to_string(),
        "entity has no ruled pronunciation"
    );
}

#[test]
fn refusal_display_names_the_rule_and_no_identifier() {
    let all = vec![
        Refusal::V1SchemaVersion { found: 4242 },
        Refusal::V2PayloadRequired,
        Refusal::V3DuplicateNode { id: node() },
        Refusal::V4DanglingRef {
            id: node(),
            at: SENTINEL,
        },
        Refusal::V5Unresolved { id: inv() },
        Refusal::V6WrongFamily {
            id: inv(),
            expected: Family::Role,
            at: SENTINEL,
        },
        Refusal::V7ValueKind {
            what: SENTINEL.into(),
        },
        Refusal::V8NotApplicable {
            feature: inv(),
            slot: Slot::Scene,
        },
        Refusal::V10UnknownRevision { rev: 4242 },
        Refusal::V11DuplicateFeature {
            feature: inv(),
            slot: Some(Slot::Scene),
        },
        Refusal::V11DuplicateNodeRef {
            id: node(),
            at: SENTINEL,
        },
        Refusal::V11DuplicateRole {
            role: inv(),
            predication: node(),
        },
        Refusal::V12TooManyNodes {
            count: 4242,
            max: 4242,
        },
        Refusal::V14ConsentWithoutAddressee,
        Refusal::V14ConsentNotAPredication { id: node() },
        Refusal::V15ConsentSource {
            source: ProvenanceSource::ImportedCorpus,
        },
        Refusal::V16SpeakerNotInScene,
        Refusal::V17AddresseeNotAReferent { id: node() },
    ];
    for r in &all {
        assert!(r.to_string().starts_with(r.rule()), "{r:?}");
    }
    assert_clean_and_distinct(all);
    assert_eq!(
        Refusal::V14ConsentNotAPredication { id: node() }.to_string(),
        "V14: consent names no predication"
    );
}

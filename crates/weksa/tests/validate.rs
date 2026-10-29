mod common;
use common::*;
use weksa::packet::*;
use weksa::{Family, Refusal, Slot, validate};

fn refusals(p: &Packet) -> Vec<Refusal> {
    validate(p, &inventory()).err().unwrap_or_default()
}

fn one(p: &Packet) -> Refusal {
    let mut r = refusals(p);
    assert_eq!(r.len(), 1, "{r:?}");
    r.remove(0)
}

fn unresolved(id: &str) -> Refusal {
    Refusal::V5Unresolved { id: iv(id) }
}

fn empty_payload() -> Packet {
    let mut p = good();
    p.referents.clear();
    p.predications.clear();
    p.relations.clear();
    p.discourse.addressee = None;
    p.context.participants.clear();
    p.context.audience.clear();
    p.constraints.preserve.clear();
    p
}

fn feat(id: &str, value: FeatValue) -> FeatureValue {
    FeatureValue {
        feature: iv(id),
        value,
    }
}

#[test]
fn a_good_packet_validates() {
    assert!(validate(&good(), &inventory()).is_ok());
}

#[test]
fn v1_schema_version() {
    let mut p = good();
    p.schema_version = SCHEMA_VERSION + 1;
    assert_eq!(
        refusals(&p),
        vec![Refusal::V1SchemaVersion {
            found: SCHEMA_VERSION + 1
        }]
    );
}

#[test]
fn v2_payload_required() {
    assert_eq!(refusals(&empty_payload()), vec![Refusal::V2PayloadRequired]);
    // Any one family is payload enough.
    let mut only_relations = empty_payload();
    only_relations.relations = good().relations;
    only_relations.relations[0].from = nd("n8");
    only_relations.relations[0].to = nd("n9");
    assert!(!refusals(&only_relations).contains(&Refusal::V2PayloadRequired));
    let mut only_predications = empty_payload();
    only_predications.predications = good().predications;
    only_predications.predications[0].roles.clear();
    assert_eq!(refusals(&only_predications), vec![]);
    let mut only_referents = empty_payload();
    only_referents.referents = good().referents;
    assert_eq!(refusals(&only_referents), vec![]);
}

#[test]
fn v3_unique_node_ids() {
    let mut p = good();
    p.predications[0].id = nd("n1");
    p.relations[0].from = nd("n1");
    assert_eq!(
        refusals(&p),
        vec![Refusal::V3DuplicateNode { id: nd("n1") }]
    );
    let mut twice = good();
    twice.referents.push(twice.referents[0].clone());
    assert_eq!(
        refusals(&twice),
        vec![Refusal::V3DuplicateNode { id: nd("n1") }]
    );
}

#[test]
fn v4_refs_resolve() {
    let dangling = |edit: fn(&mut Packet), at: &'static str| {
        let mut p = good();
        edit(&mut p);
        assert_eq!(
            refusals(&p),
            vec![Refusal::V4DanglingRef { id: nd("n99"), at }],
            "{at}"
        );
    };
    dangling(
        |p| p.predications[0].roles[0].filler = nd("n99"),
        "role filler",
    );
    dangling(|p| p.relations[0].from = nd("n99"), "relation from");
    dangling(|p| p.relations[0].to = nd("n99"), "relation to");
    dangling(|p| p.discourse.addressee = Some(nd("n99")), "addressee");
    dangling(|p| p.context.participants.push(nd("n99")), "participant");
    dangling(|p| p.context.audience.push(nd("n99")), "audience");
    dangling(|p| p.constraints.preserve.push(nd("n99")), "preserve");
}

#[test]
fn v5_inventory_resolves_at_rev() {
    let with_concept = |rev: u32, id: &str| {
        let mut p = good();
        p.inventory_rev = rev;
        p.referents[0].concept = iv(id);
        p
    };
    // Introduced after the packet's rev.
    assert_eq!(
        one(&with_concept(2, CONCEPT_LATE)),
        unresolved(CONCEPT_LATE)
    );
    // Introduced at the packet's rev.
    assert_eq!(refusals(&with_concept(4, CONCEPT_LATE)), vec![]);
    // Retired later than the packet's rev still resolves.
    assert_eq!(refusals(&with_concept(2, CONCEPT_RETIRED)), vec![]);
    // Retired at or before the packet's rev does not.
    assert_eq!(
        one(&with_concept(3, CONCEPT_RETIRED)),
        unresolved(CONCEPT_RETIRED)
    );
    assert_eq!(
        one(&with_concept(4, CONCEPT_RETIRED)),
        unresolved(CONCEPT_RETIRED)
    );
    // An id no revision holds.
    assert_eq!(
        one(&with_concept(2, "concept:99")),
        unresolved("concept:99")
    );
    // An extension is resolved like any inventory id.
    let mut ext = good();
    ext.extensions[0].key = iv("extension:99");
    assert_eq!(one(&ext), unresolved("extension:99"));
}

#[test]
fn v6_family_matches_slot() {
    let wrong = |edit: fn(&mut Packet), id: &str, expected: Family, at: &'static str| {
        let mut p = good();
        edit(&mut p);
        assert_eq!(
            refusals(&p),
            vec![Refusal::V6WrongFamily {
                id: iv(id),
                expected,
                at
            }],
            "{at}"
        );
    };
    wrong(
        |p| p.discourse.features[0].feature = iv(AGENT),
        AGENT,
        Family::Feature,
        "feature",
    );
    wrong(
        |p| p.discourse.speech_act = Some(iv("relation:1")),
        "relation:1",
        Family::SpeechAct,
        "speech act",
    );
    wrong(
        |p| p.referents[0].kind = iv(CONCEPT_A),
        CONCEPT_A,
        Family::ReferentKind,
        "referent kind",
    );
    wrong(
        |p| p.referents[0].concept = iv(PERSON),
        PERSON,
        Family::Concept,
        "referent concept",
    );
    wrong(
        |p| p.predications[0].concept = iv(FRAME),
        FRAME,
        Family::Concept,
        "predication concept",
    );
    wrong(
        |p| p.predications[0].frame = iv(CONCEPT_A),
        CONCEPT_A,
        Family::Frame,
        "frame",
    );
    wrong(
        |p| p.predications[0].roles[0].role = iv(HEDGE),
        HEDGE,
        Family::Role,
        "role",
    );
    wrong(
        |p| p.relations[0].relation = iv(AGENT),
        AGENT,
        Family::Relation,
        "relation",
    );
    wrong(
        |p| p.context.setting = Some(iv(FRAME)),
        FRAME,
        Family::Concept,
        "context concept",
    );
    wrong(
        |p| p.context.medium = Some(iv(FRAME)),
        FRAME,
        Family::Concept,
        "context concept",
    );
    wrong(
        |p| p.extensions[0].key = iv(CONCEPT_A),
        CONCEPT_A,
        Family::Extension,
        "extension",
    );
}

#[test]
fn v7_value_kind() {
    let hedge = |v: FeatValue| {
        let mut p = good();
        p.discourse.features[0].value = v;
        refusals(&p)
    };
    let bad = vec![Refusal::V7ValueKind { what: HEDGE.into() }];
    assert_eq!(hedge(FeatValue::Grade(grade(1000))), vec![]);
    assert_eq!(hedge(FeatValue::Flag(true)), bad);
    assert_eq!(hedge(FeatValue::Option(lab("x"))), bad);
    let modality = |v: FeatValue| {
        let mut p = good();
        p.predications[0].features[0].value = v;
        refusals(&p)
    };
    let bad_modality = vec![Refusal::V7ValueKind {
        what: MODALITY.into(),
    }];
    assert_eq!(modality(FeatValue::Option(lab("necessary"))), vec![]);
    assert_eq!(modality(FeatValue::Option(lab("banana"))), bad_modality);
    assert_eq!(modality(FeatValue::Grade(grade(1))), bad_modality);
    let mut flag = good();
    flag.discourse.features = vec![feat(IRONY, FeatValue::Flag(false))];
    assert_eq!(refusals(&flag), vec![]);
    flag.discourse.features[0].value = FeatValue::Grade(grade(5));
    assert_eq!(
        refusals(&flag),
        vec![Refusal::V7ValueKind { what: IRONY.into() }]
    );
}

#[test]
fn extension_key_is_an_extension_entry() {
    // A concept id is not an extension; an extension entry fits its value kind.
    let mut by_concept = good();
    by_concept.extensions[0].key = iv(CONCEPT_A);
    assert_eq!(
        one(&by_concept),
        Refusal::V6WrongFamily {
            id: iv(CONCEPT_A),
            expected: Family::Extension,
            at: "extension"
        }
    );
    let mut flagged = good();
    flagged.extensions[0].value = FeatValue::Flag(true);
    assert_eq!(one(&flagged), Refusal::V7ValueKind { what: EXT.into() });
    let mut optioned = good();
    optioned.extensions[0].value = FeatValue::Option(lab("x"));
    assert_eq!(one(&optioned), Refusal::V7ValueKind { what: EXT.into() });
    let mut unknown = good();
    unknown.extensions[0].key = iv("extension:2");
    assert_eq!(one(&unknown), unresolved("extension:2"));
    // No extension means none to check.
    let mut none = good();
    none.extensions.clear();
    assert_eq!(refusals(&none), vec![]);
}

#[test]
fn v8_feature_applies_to() {
    let mut p = good();
    // polarity applies to predications only.
    p.discourse
        .features
        .push(feat(POLARITY, FeatValue::Option(lab("negative"))));
    assert_eq!(
        refusals(&p),
        vec![Refusal::V8NotApplicable {
            feature: iv(POLARITY),
            slot: Slot::Discourse
        }]
    );
    let mut referent = good();
    referent.referents[0]
        .features
        .push(feat(HEDGE, FeatValue::Grade(grade(1))));
    assert_eq!(
        refusals(&referent),
        vec![Refusal::V8NotApplicable {
            feature: iv(HEDGE),
            slot: Slot::Referent
        }]
    );
    let mut relation = good();
    relation.relations[0]
        .features
        .push(feat(HEDGE, FeatValue::Grade(grade(1))));
    assert_eq!(
        refusals(&relation),
        vec![Refusal::V8NotApplicable {
            feature: iv(HEDGE),
            slot: Slot::Relation
        }]
    );
    let mut pred = good();
    pred.predications[0]
        .features
        .push(feat(HEDGE, FeatValue::Grade(grade(1))));
    assert_eq!(refusals(&pred), vec![]);
}

#[test]
fn v10_rev_exists() {
    let mut p = good();
    p.inventory_rev = 9;
    assert_eq!(refusals(&p), vec![Refusal::V10UnknownRevision { rev: 9 }]);
    p.inventory_rev = 0;
    assert_eq!(refusals(&p), vec![Refusal::V10UnknownRevision { rev: 0 }]);
}

#[test]
fn every_broken_rule_is_reported() {
    let mut p = good();
    p.schema_version = 7;
    p.predications[0].roles[0].filler = nd("n99");
    p.extensions[0].key = iv(CONCEPT_A);
    let rules: Vec<&str> = refusals(&p).iter().map(Refusal::rule).collect();
    assert_eq!(rules, vec!["V1", "V4", "V6"]);
}

#[test]
fn lowering_input_carries_no_trace_id_or_provenance() {
    let mut p = good();
    p.trace.notes = vec!["SECRET-NOTE".into()];
    p.trace.rejected_readings = vec!["SECRET-READING".into()];
    let input = validate(&p, &inventory()).unwrap();
    let seen = format!("{input:?}");
    for hidden in [PACKET_ID, ENCODER_ID, "item:nibu:1", "SECRET"] {
        assert!(!seen.contains(hidden), "{hidden} reached the input: {seen}");
    }
    // The view is the packet's meaning, readable through getters.
    assert_eq!(input.inventory_rev(), 2);
    assert_eq!(input.kind(), PacketKind::DialogueLine);
    assert_eq!(input.referents(), p.referents.as_slice());
    assert_eq!(input.predications(), p.predications.as_slice());
    assert_eq!(input.relations(), p.relations.as_slice());
    assert_eq!(input.extensions(), p.extensions.as_slice());
    assert_eq!(input.context(), &p.context);
    assert_eq!(input.discourse(), &p.discourse);
    assert_eq!(input.constraints(), &p.constraints);
}

#[test]
fn invalid_packet_has_no_lowering_input() {
    let mut p = good();
    p.schema_version = 99;
    p.referents.clear();
    let refused = validate(&p, &inventory()).unwrap_err();
    let rules: Vec<&str> = refused.iter().map(Refusal::rule).collect();
    assert_eq!(rules[0], "V1");
    assert!(rules.contains(&"V4"), "{rules:?}");
}

fn grade_feature(grade_value: u16) -> FeatureValue {
    feat(FEAT_ALL, FeatValue::Grade(grade(grade_value)))
}

fn referent(id: &str) -> Referent {
    Referent {
        id: nd(id),
        kind: iv(PERSON),
        concept: iv(CONCEPT_A),
        entity: None,
        features: vec![],
    }
}

fn v11(slot: Option<Slot>, feature: &str) -> Refusal {
    Refusal::V11DuplicateFeature {
        feature: iv(feature),
        slot,
    }
}

#[test]
fn v11_duplicate_feature_each_slot() {
    let mut discourse = good();
    discourse.discourse.features = vec![grade_feature(1), grade_feature(2)];
    assert_eq!(
        refusals(&discourse),
        vec![v11(Some(Slot::Discourse), FEAT_ALL)]
    );
    let mut referent = good();
    referent.referents[0].features = vec![grade_feature(1), grade_feature(2)];
    assert_eq!(
        refusals(&referent),
        vec![v11(Some(Slot::Referent), FEAT_ALL)]
    );
    let mut predication = good();
    predication.predications[0]
        .features
        .extend([grade_feature(1), grade_feature(2)]);
    assert_eq!(
        refusals(&predication),
        vec![v11(Some(Slot::Predication), FEAT_ALL)]
    );
    let mut relation = good();
    relation.relations[0].features = vec![grade_feature(1), grade_feature(2)];
    assert_eq!(
        refusals(&relation),
        vec![v11(Some(Slot::Relation), FEAT_ALL)]
    );
    let mut extension = good();
    extension.extensions.push(Extension {
        key: iv(EXT),
        value: FeatValue::Grade(grade(4)),
    });
    assert_eq!(refusals(&extension), vec![v11(None, EXT)]);
}

#[test]
fn v11_counts_per_list_not_per_packet() {
    // The same feature once in each list, and different features in one list, are fine.
    let mut p = good();
    p.discourse.features = vec![grade_feature(1), feat(HEDGE, FeatValue::Grade(grade(2)))];
    p.referents[0].features = vec![grade_feature(1)];
    p.predications[0].features.push(grade_feature(1));
    p.relations[0].features = vec![grade_feature(1)];
    assert_eq!(refusals(&p), vec![]);
}

#[test]
fn v11_grade_bytes_packet_refused() {
    // Soul s2: 113 hedge grades in one list spell a line byte by byte.
    let mut p = good();
    p.discourse.features = (0..113)
        .map(|i| feat(HEDGE, FeatValue::Grade(grade(i))))
        .collect();
    let got = refusals(&p);
    assert_eq!(got.len(), 112, "{got:?}");
    assert!(got.iter().all(|r| *r == v11(Some(Slot::Discourse), HEDGE)));
}

#[test]
fn v12_node_bound() {
    // 62 referents, the predication n2 and the relation make 64 nodes.
    let with_referents = |last: u32| {
        let mut p = good();
        let ids = std::iter::once(1).chain(3..=last);
        p.referents = ids.map(|k| referent(&format!("n{k}"))).collect();
        p
    };
    let at_bound = with_referents(63);
    assert_eq!(
        at_bound.referents.len() + at_bound.predications.len() + at_bound.relations.len(),
        Packet::MAX_NODES
    );
    assert_eq!(refusals(&at_bound), vec![]);
    assert_eq!(
        refusals(&with_referents(64)),
        vec![Refusal::V12TooManyNodes {
            count: 65,
            max: Packet::MAX_NODES
        }]
    );
}

// Soul s3: a repeated node id in a node list, or a repeated role in one
// predication, spelled a line one bit per entry. Each list now holds a node or
// a role at most once, so the channel is bounded.

const LINE: &str = "Take your hand off the hatch, Captain.";

fn bits(s: &str) -> Vec<bool> {
    s.bytes()
        .flat_map(|b| (0..8).rev().map(move |i| (b >> i) & 1 == 1))
        .collect()
}

fn two_referents() -> Packet {
    let mut p = good();
    let mut r = p.referents[0].clone();
    r.id = nd("n3");
    p.referents.push(r);
    p
}

fn bit_ids(s: &str) -> Vec<NodeId> {
    bits(s)
        .into_iter()
        .map(|b| nd(if b { "n3" } else { "n1" }))
        .collect()
}

fn dup_node(id: &str, at: &'static str) -> Refusal {
    Refusal::V11DuplicateNodeRef { id: nd(id), at }
}

#[test]
fn v11_node_lists_refuse_repeats_and_allow_each_node_once() {
    let mut ok = two_referents();
    ok.context.participants = vec![nd("n1"), nd("n3")];
    ok.context.audience = vec![nd("n3"), nd("n1")];
    ok.constraints.preserve = vec![nd("n3"), nd("n1")];
    assert_eq!(refusals(&ok), vec![]);

    let mut p = two_referents();
    p.context.participants = vec![nd("n1"), nd("n1")];
    assert_eq!(refusals(&p), vec![dup_node("n1", "participant")]);
    let mut p = two_referents();
    p.context.audience = vec![nd("n3"), nd("n1"), nd("n3")];
    assert_eq!(refusals(&p), vec![dup_node("n3", "audience")]);
    let mut p = two_referents();
    p.constraints.preserve = vec![nd("n1"), nd("n1")];
    assert_eq!(refusals(&p), vec![dup_node("n1", "preserve")]);
}

#[test]
fn v11_bit_channel_in_each_node_list_refused() {
    let n = bits(LINE).len();
    let mut p = two_referents();
    p.context.participants = bit_ids(LINE);
    let got = refusals(&p);
    assert_eq!(got.len(), n - 2, "{got:?}");
    assert!(got.iter().all(|r| r.rule() == "V11"));
    let mut p = two_referents();
    p.context.audience = bit_ids(LINE);
    assert_eq!(refusals(&p).len(), n - 2);
    let mut p = two_referents();
    p.constraints.preserve = bit_ids(LINE);
    assert_eq!(refusals(&p).len(), n - 2);
}

#[test]
fn v11_unbounded_participants_refused() {
    let mut p = two_referents();
    p.context.participants = (0..100_000)
        .map(|i| nd(if i % 3 == 0 { "n3" } else { "n1" }))
        .collect();
    assert_eq!(refusals(&p).len(), 100_000 - 2);
}

#[test]
fn v11_role_repeat_in_one_predication_refused() {
    let dup = |role: &str| Refusal::V11DuplicateRole {
        role: iv(role),
        predication: nd("n2"),
    };
    let mut p = good();
    p.predications[0].roles = bits(LINE)
        .into_iter()
        .map(|b| RoleFill {
            role: iv(if b { AGENT } else { PATIENT }),
            filler: nd("n1"),
        })
        .collect();
    let got = refusals(&p);
    assert_eq!(got.len(), bits(LINE).len() - 2, "{got:?}");
    assert!(got.contains(&dup(AGENT)) && got.contains(&dup(PATIENT)));

    // The same role in two different predications is not a repeat.
    let mut ok = good();
    let mut second = ok.predications[0].clone();
    second.id = nd("n4");
    ok.predications.push(second);
    assert_eq!(refusals(&ok), vec![]);
}

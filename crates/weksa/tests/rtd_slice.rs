//! The first `rtd-imperial` corpus slice: ten broadcast-commentary utterances as
//! packets plus an expected surface category. Every packet must pass the
//! validator against the placeholder inventory revision 4, and every expected
//! class, and every class a packet's facts select, must exist in the imperial
//! ontology profile. A class the ontology lacks fails here, never defaults.
//! No target-language text exists in the slice and none is derived from English.

mod common;
use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;
use weksa::inventory::{proof_revision, rtd_revision, scene_revision, seed};
use weksa::packet::{
    Constraints, Context, Discourse, FeatValue, FeatureValue, Grade, Packet, PacketId, PacketKind,
    Predication, Provenance, ProvenanceSource, Referent, RoleFill, Stance, Trace,
};
use weksa::{Family, InvId, Inventory, InventoryStore, OptionLabel, validate};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ontology {
    profile_id: String,
    status: String,
    finish_classes: Vec<Finish>,
    commentator_display: Display,
    evaluation_slots: Vec<Tagged>,
    evidential_classes: Vec<String>,
    inference_required_for_readings: Vec<String>,
    standing: Vec<Tagged>,
    fields: Vec<String>,
    audiences: Vec<String>,
    means_order: Vec<String>,
    locus_classes: Vec<String>,
    practice_frames: Vec<String>,
    locus_note: String,
    path_fit_readings: Vec<Tagged>,
    path_fit_rules: Vec<PathRule>,
    path_fit_note: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Tagged {
    id: String,
    provenance: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Finish {
    id: String,
    restraint_min: u16,
    restraint_max: u16,
    provenance: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Display {
    provenance: String,
    strong_affect_min: u16,
    classes: Vec<String>,
    rules: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathRule {
    decoration: String,
    serves_path: Option<bool>,
    staged: Option<bool>,
    reading: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Slice {
    slice: String,
    status: String,
    entries: Vec<Entry>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    key: String,
    situation: String,
    meaning: String,
    referents: Vec<Ref>,
    predications: Vec<Pred>,
    stance: Affects,
    scene: SceneFacts,
    expected: Expected,
    placeholders: Vec<String>,
    packet_context: PacketContext,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PacketContext {
    medium: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ref {
    id: String,
    kind: String,
    concept: String,
    decoration: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pred {
    id: String,
    concept: String,
    frame: String,
    roles: std::collections::BTreeMap<String, String>,
    constraint_volition: Option<String>,
    evidentiality: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Affects {
    anger: u16,
    fear: u16,
    sadness: u16,
    joy: u16,
    disgust: u16,
    surprise: u16,
    contempt: u16,
    affection: u16,
    shame: u16,
    restraint: u16,
}

/// What the packet cannot carry yet: the `Speaker` and `Scene` slots are
/// planned (cut `speaker-scene`), and `serves_path` and `staged` are
/// ontology-side inputs by ruling, never packet fields.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneFacts {
    audience: String,
    performer_field: String,
    performer_standing: String,
    serves_path: Option<bool>,
    staged: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    finish_class: String,
    evaluation: Vec<String>,
    evidential: String,
    path_fit: String,
    overmatch: bool,
    locus: String,
    commentator_display: String,
}

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn ontology() -> Ontology {
    let text =
        std::fs::read_to_string(root().join("data/target-language-ontology/rtd-imperial.json"))
            .unwrap();
    serde_json::from_str(&text).unwrap()
}

fn slice() -> Slice {
    let text =
        std::fs::read_to_string(root().join("data/corpus/rtd-imperial/slice-1.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn inventory() -> (Inventory, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = InventoryStore::open(&dir.path().join("i.cc")).unwrap();
    for (rev, entries) in [seed(), proof_revision(), scene_revision(), rtd_revision()] {
        store.admit_revision(rev, entries).unwrap();
    }
    (store.inventory().unwrap(), dir)
}

fn by_gloss(inv: &Inventory, family: Family, gloss: &str) -> InvId {
    inv.entries()
        .find(|e| e.family() == family && e.gloss == gloss && e.live_at(4))
        .unwrap_or_else(|| panic!("inventory revision 4 has no {family:?} {gloss}"))
        .id
}

fn option(inv: &Inventory, feature: &str, label: &str) -> FeatureValue {
    FeatureValue {
        feature: by_gloss(inv, Family::Feature, feature),
        value: FeatValue::Option(OptionLabel::parse(label).unwrap()),
    }
}

fn node(s: &str) -> weksa::packet::NodeId {
    weksa::packet::NodeId::parse(s).unwrap()
}

/// The slice entry as a packet of inventory revision 4. Fixture words are glosses;
/// ids come from the inventory, so a gloss the inventory lacks is a panic here.
fn packet_of(e: &Entry, inv: &Inventory) -> Packet {
    let referents = e
        .referents
        .iter()
        .map(|r| Referent {
            id: node(&r.id),
            kind: InvId::parse(&r.kind).unwrap(),
            concept: by_gloss(inv, Family::Concept, &r.concept),
            entity: None,
            features: r
                .decoration
                .iter()
                .map(|d| option(inv, "decoration_state", d))
                .collect(),
        })
        .collect();
    let predications = e
        .predications
        .iter()
        .map(|p| Predication {
            id: node(&p.id),
            concept: by_gloss(inv, Family::Concept, &p.concept),
            frame: by_gloss(inv, Family::Frame, &p.frame),
            roles: p
                .roles
                .iter()
                .map(|(role, filler)| RoleFill {
                    role: by_gloss(inv, Family::Role, role),
                    filler: node(filler),
                })
                .collect(),
            features: p
                .constraint_volition
                .iter()
                .map(|v| option(inv, "constraint_volition", v))
                .chain(
                    p.evidentiality
                        .iter()
                        .map(|v| option(inv, "evidentiality", v)),
                )
                .collect(),
        })
        .collect();
    let a = &e.stance;
    let stance = [
        ("anger", a.anger),
        ("fear", a.fear),
        ("sadness", a.sadness),
        ("joy", a.joy),
        ("disgust", a.disgust),
        ("surprise", a.surprise),
        ("contempt", a.contempt),
        ("affection", a.affection),
        ("shame", a.shame),
        ("restraint", a.restraint),
    ]
    .into_iter()
    .filter(|(g, v)| *v > 0 || *g == "restraint")
    .map(|(g, v)| FeatureValue {
        feature: by_gloss(inv, Family::Feature, g),
        value: FeatValue::Grade(Grade::new(v).unwrap()),
    })
    .collect();
    Packet {
        id: PacketId::parse(&format!("packet:{}", hex16(&e.key))).unwrap(),
        schema_version: weksa::packet::SCHEMA_VERSION,
        inventory_rev: 4,
        kind: PacketKind::CommentaryIntent,
        provenance: Provenance {
            source: ProvenanceSource::Authored,
            corpus_item: weksa::CorpusItemId::parse(&format!("item:{}:1", e.key)).unwrap(),
            encoder: weksa::packet::EncoderId::parse("encoder:6f70657261746f72").unwrap(),
            confidence: Grade::new(500).unwrap(),
        },
        context: Context {
            setting: None,
            medium: Some(by_gloss(inv, Family::Concept, &e.packet_context.medium)),
            participants: vec![],
            audience: vec![],
        },
        discourse: Discourse {
            speech_act: Some(by_gloss(inv, Family::SpeechAct, "assert")),
            addressee: None,
            features: vec![],
        },
        referents,
        predications,
        relations: vec![],
        constraints: Constraints {
            preserve: vec![],
            deterministic: true,
            max_latency_ms: None,
        },
        extensions: vec![],
        trace: Trace {
            notes: vec![
                "PLACEHOLDER slice packet; see data/corpus/rtd-imperial/slice-1.json".into(),
            ],
            rejected_readings: vec![],
        },
        stance: Stance { features: stance },
    }
}

fn hex16(key: &str) -> String {
    let hex: String = key.bytes().map(|b| format!("{b:02x}")).collect();
    format!("{hex:0<16}")[..16].to_string()
}

/// The ontology's whole reading of an entry from its facts; the expected block
/// must equal it. Each step is a lookup in the profile or a count on the packet.
fn derive(e: &Entry, o: &Ontology) -> Result<Expected, String> {
    let r = e.stance.restraint;
    let finish = o
        .finish_classes
        .iter()
        .find(|f| (f.restraint_min..=f.restraint_max).contains(&r))
        .ok_or(format!("{}: no finish class holds restraint {r}", e.key))?;
    let performer = e
        .referents
        .iter()
        .find(|x| x.concept == "performer")
        .ok_or(format!("{}: no performer", e.key))?;
    let decoration = performer.decoration.as_deref().unwrap_or("not_applicable");
    let matches: Vec<&PathRule> = o
        .path_fit_rules
        .iter()
        .filter(|p| {
            p.decoration == decoration
                && p.serves_path.is_none_or(|s| Some(s) == e.scene.serves_path)
                && p.staged.is_none_or(|s| s == e.scene.staged)
        })
        .collect();
    let [rule] = matches.as_slice() else {
        return Err(format!(
            "{}: {} path-fit rules match decoration {decoration}",
            e.key,
            matches.len()
        ));
    };
    let rank = |instrument: &str| o.means_order.iter().position(|m| m == instrument);
    let instrument_of = |agent: &str| {
        e.predications
            .iter()
            .find(|p| {
                p.frame == "use_means" && p.roles.get("agent").map(String::as_str) == Some(agent)
            })
            .and_then(|p| {
                let node = p.roles.get("instrument")?;
                let concept = &e.referents.iter().find(|x| &x.id == node)?.concept;
                Some((rank(concept)?, p.constraint_volition.clone()))
            })
    };
    let outcome = e.predications.iter().find(|p| p.frame == "contest_outcome");
    let overmatch = outcome
        .and_then(|p| {
            let winner = instrument_of(p.roles.get("agent")?)?;
            let loser = instrument_of(p.roles.get("patient")?)?;
            Some(winner.0 > loser.0 && winner.1.as_deref() != Some("chosen"))
        })
        .unwrap_or(false);
    let strongest = [
        e.stance.anger,
        e.stance.fear,
        e.stance.sadness,
        e.stance.joy,
        e.stance.disgust,
        e.stance.surprise,
        e.stance.contempt,
        e.stance.affection,
        e.stance.shame,
    ]
    .into_iter()
    .max()
    .unwrap();
    let display = if strongest == 0 {
        "absent_devotion"
    } else if finish.id == "raw" && strongest >= o.commentator_display.strong_affect_min {
        "vulgar"
    } else {
        "form"
    };
    let evidential = if o.inference_required_for_readings.contains(&rule.reading) {
        "inferred"
    } else {
        "witnessed"
    };
    let locus = if e
        .predications
        .iter()
        .any(|p| o.practice_frames.contains(&p.frame))
    {
        "practice"
    } else {
        "spectacle"
    };
    Ok(Expected {
        finish_class: finish.id.clone(),
        evaluation: e.expected.evaluation.clone(),
        evidential: evidential.into(),
        path_fit: rule.reading.clone(),
        overmatch,
        locus: locus.into(),
        commentator_display: display.into(),
    })
}

/// Every class name an entry's expected block and scene use that the profile
/// does not list, as `entry: kind name`.
fn lacking(e: &Entry, o: &Ontology) -> Vec<String> {
    let ids = |v: &[Tagged]| v.iter().map(|t| t.id.clone()).collect::<BTreeSet<_>>();
    let finish: BTreeSet<_> = o.finish_classes.iter().map(|f| f.id.clone()).collect();
    let evaluation = ids(&o.evaluation_slots);
    let standing = ids(&o.standing);
    let readings = ids(&o.path_fit_readings);
    let mut out = Vec::new();
    let mut need = |kind: &str, name: &str, set: &BTreeSet<String>| {
        if !set.contains(name) {
            out.push(format!("{}: {kind} {name}", e.key));
        }
    };
    need("finish class", &e.expected.finish_class, &finish);
    for slot in &e.expected.evaluation {
        need("evaluation slot", slot, &evaluation);
    }
    need(
        "evidential class",
        &e.expected.evidential,
        &o.evidential_classes.iter().cloned().collect(),
    );
    need("path-fit reading", &e.expected.path_fit, &readings);
    need("standing", &e.scene.performer_standing, &standing);
    need(
        "field",
        &e.scene.performer_field,
        &o.fields.iter().cloned().collect(),
    );
    need(
        "audience",
        &e.scene.audience,
        &o.audiences.iter().cloned().collect(),
    );
    need(
        "locus",
        &e.expected.locus,
        &o.locus_classes.iter().cloned().collect(),
    );
    need(
        "commentator display",
        &e.expected.commentator_display,
        &o.commentator_display.classes.iter().cloned().collect(),
    );
    out
}

#[test]
fn the_slice_holds_ten_entries_and_calls_itself_a_draft() {
    let s = slice();
    assert_eq!(s.slice, "rtd-imperial-slice-1");
    assert!(s.status.starts_with("DRAFT placeholder"));
    assert_eq!(s.entries.len(), 10);
    let keys: BTreeSet<_> = s.entries.iter().map(|e| e.key.as_str()).collect();
    assert_eq!(keys.len(), 10, "keys are unique");
    for e in &s.entries {
        assert_eq!(e.situation, "broadcast_commentary", "{}", e.key);
        assert!(!e.meaning.is_empty(), "{}", e.key);
        assert!(!e.placeholders.is_empty(), "{}: invention is listed", e.key);
    }
    let o = ontology();
    assert_eq!(
        o.profile_id,
        "weksa.target_cultural_ontology.rtd-imperial.v0"
    );
    assert!(o.status.starts_with("DRAFT placeholder"));
    assert!(o.path_fit_note.contains("never packet fields"));
    assert!(!o.locus_note.is_empty() && !o.commentator_display.rules.is_empty());
    assert!(
        o.finish_classes
            .iter()
            .map(|f| &f.provenance)
            .chain(o.standing.iter().map(|t| &t.provenance))
            .chain(o.path_fit_readings.iter().map(|t| &t.provenance))
            .chain(o.evaluation_slots.iter().map(|t| &t.provenance))
            .chain([&o.commentator_display.provenance])
            .all(|p| ["E", "I", "placeholder"].contains(&p.as_str())),
        "every class carries a provenance tag"
    );
}

#[test]
fn every_packet_passes_the_validator_against_the_placeholder_revision() {
    let (inv, _dir) = inventory();
    for e in &slice().entries {
        let packet = packet_of(e, &inv);
        if let Err(refusals) = validate(&packet, &inv) {
            panic!(
                "{} refused: {:?}",
                e.key,
                refusals.iter().map(|r| r.rule()).collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn every_class_an_entry_uses_exists_in_the_ontology() {
    let o = ontology();
    let missing: Vec<String> = slice()
        .entries
        .iter()
        .flat_map(|e| lacking(e, &o))
        .collect();
    assert!(
        missing.is_empty(),
        "classes the ontology lacks: {missing:?}"
    );
}

#[test]
fn the_ontology_reads_every_entry_to_its_expected_category() {
    let o = ontology();
    for e in &slice().entries {
        let got = derive(e, &o).unwrap();
        let want = &e.expected;
        assert_eq!(got.finish_class, want.finish_class, "{} finish", e.key);
        assert_eq!(got.path_fit, want.path_fit, "{} path-fit", e.key);
        assert_eq!(got.evidential, want.evidential, "{} evidential", e.key);
        assert_eq!(got.overmatch, want.overmatch, "{} overmatch", e.key);
        assert_eq!(got.locus, want.locus, "{} locus", e.key);
        assert_eq!(
            got.commentator_display, want.commentator_display,
            "{} display",
            e.key
        );
    }
}

#[test]
fn the_finish_bands_cover_restraint_exactly_once() {
    let o = ontology();
    for r in 0..=1000u16 {
        let n = o
            .finish_classes
            .iter()
            .filter(|f| (f.restraint_min..=f.restraint_max).contains(&r))
            .count();
        assert_eq!(n, 1, "restraint {r}");
    }
}

#[test]
fn the_slice_exercises_every_finish_class_and_path_fit_reading() {
    let (o, s) = (ontology(), slice());
    for f in &o.finish_classes {
        assert!(
            s.entries.iter().any(|e| e.expected.finish_class == f.id),
            "no entry for finish class {}",
            f.id
        );
    }
    for t in &o.path_fit_readings {
        assert!(
            s.entries.iter().any(|e| e.expected.path_fit == t.id),
            "no entry for path-fit reading {}",
            t.id
        );
    }
    assert!(s.entries.iter().any(|e| e.expected.overmatch));
    assert!(s.entries.iter().any(|e| e.expected.locus == "practice"));
    assert!(
        s.entries
            .iter()
            .any(|e| e.expected.commentator_display == "vulgar")
    );
}

#[test]
fn a_class_the_ontology_lacks_fails_instead_of_defaulting() {
    let (o, s) = (ontology(), slice());
    let mut e = s.entries[0].clone();
    e.expected.finish_class = "glossy".into();
    e.expected.evaluation.push("gleaming".into());
    e.scene.performer_standing = "viceroy".into();
    e.scene.audience = "alien_tourist".into();
    let missing = lacking(&e, &o);
    assert_eq!(missing.len(), 4, "{missing:?}");
    for what in ["glossy", "gleaming", "viceroy", "alien_tourist"] {
        assert!(missing.iter().any(|m| m.ends_with(what)), "{what}");
    }
}

#[test]
fn a_decoration_state_with_no_path_fit_rule_is_refused_by_the_ontology() {
    let mut o = ontology();
    o.path_fit_rules.retain(|r| r.reading != "gauche");
    let s = slice();
    let e = s
        .entries
        .iter()
        .find(|e| e.expected.path_fit == "gauche")
        .unwrap();
    assert!(derive(e, &o).unwrap_err().contains("0 path-fit rules"));
}

#[test]
fn a_restraint_no_finish_class_holds_is_refused_by_the_ontology() {
    let mut o = ontology();
    o.finish_classes.retain(|f| f.id != "held");
    let s = slice();
    let e = s
        .entries
        .iter()
        .find(|e| e.expected.finish_class == "held")
        .unwrap();
    assert!(derive(e, &o).unwrap_err().contains("no finish class"));
}

#[test]
fn no_entry_carries_target_language_text() {
    // The fixture's only prose is `meaning` (an English description of the
    // neutral facts) and `placeholders`; the typed structs refuse any other
    // field, so a `text` or `surface` cannot be added without failing to parse.
    let raw =
        std::fs::read_to_string(root().join("data/corpus/rtd-imperial/slice-1.json")).unwrap();
    let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
    for e in value["entries"].as_array().unwrap() {
        let keys: BTreeSet<_> = e.as_object().unwrap().keys().map(String::as_str).collect();
        for forbidden in ["text", "surface", "utterance", "gloss", "romanization"] {
            assert!(!keys.contains(forbidden), "{forbidden}");
        }
    }
}

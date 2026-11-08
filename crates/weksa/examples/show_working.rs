//! Build-time generator for the "Show her working" page. Runs the real
//! admission door, lowerer and IPA reader over the committed proof packet and
//! variants of it, and prints one JSON document on stdout: the typed slots, the
//! inventory entries the packet chose, and the outcome, which is lowered text or
//! a named refusal. Scoring is not here: `tools/show-working/build.py` calls the
//! `weksa score` CLI on the text this emits.
//!
//! Run from the repository root: `cargo run --release --example show_working`.
//! Every variant is a mutation of `proof_packet()`, spelled in its `source`.

use std::path::Path;

use serde_json::{Value, json};
use weksa::packet::{
    EntityId, FeatValue, FeatureValue, Grade, NodeId, ProvenanceSource, RoleFill, proof_packet,
};
use weksa::{
    CorpusStore, EntityStore, InvId, Inventory, InventoryStore, LowerError, Packet, PacketStore,
    StoreError, lower, pronounce,
};

struct Fixture {
    id: &'static str,
    title: &'static str,
    note: &'static str,
    source: &'static str,
    packet: Packet,
}

fn id(s: &str) -> InvId {
    s.parse().expect("inventory id")
}

fn node(s: &str) -> NodeId {
    NodeId::parse(s).expect("node id")
}

fn grade(feature: &str, v: u16) -> FeatureValue {
    FeatureValue {
        feature: id(feature),
        value: FeatValue::Grade(Grade::new(v).expect("grade")),
    }
}

fn fixtures() -> Vec<Fixture> {
    let proof = proof_packet();
    let mut out = vec![Fixture {
        id: "proof",
        title: "Nibu warns the player off the control",
        note: "The committed proof packet, unchanged.",
        source: "proof_packet() in crates/weksa/src/packet.rs",
        packet: proof.clone(),
    }];
    let mut add = |id, title, note, source, f: &dyn Fn(&mut Packet)| {
        let mut packet = proof.clone();
        f(&mut packet);
        out.push(Fixture { id, title, note, source, packet });
    };
    add(
        "no-addressee",
        "Same meaning, nobody addressed",
        "With the addressee unset the player is a third party: no \"you\", no reflexive.",
        "proof_packet() with discourse.addressee = None",
        &|p| p.discourse.addressee = None,
    );
    add(
        "stance-held-back",
        "Anger held back, restraint high",
        "Stance is the speaker's felt affect. The lowerer has no row that reads it, so the text is the proof's text.",
        "proof_packet() with stance anger=300, restraint=700",
        &|p| p.stance.features = vec![grade("feature:9", 300), grade("feature:18", 700)],
    );
    add(
        "hedged",
        "Hedged and uncertain warning",
        "Hedge and certainty are meaning the lowerer cannot yet say. It refuses rather than drop them.",
        "proof_packet() with discourse hedge=600, certainty=400",
        &|p| p.discourse.features = vec![grade("feature:6", 600), grade("feature:7", 400)],
    );
    add(
        "negated",
        "Negated intervention (polarity: negative)",
        "Dropping the negation would say the opposite, so the lowerer refuses. This is the shape of a negated Dao line awaiting polarity.",
        "proof_packet() with predication n5 polarity=negative",
        &|p| {
            p.predications[1].features = vec![FeatureValue {
                feature: id("feature:1"),
                value: FeatValue::Option("negative".parse().expect("label")),
            }]
        },
    );
    add(
        "consent-authored",
        "Addressee consent, authored source",
        "Someone who saw the request says the addressee asked for it. No lowering speaks for that request yet.",
        "proof_packet() with discourse.addressee_consent = n4 (source Authored)",
        &|p| p.discourse.addressee_consent = Some(node("n4")),
    );
    add(
        "consent-imported",
        "Addressee consent, imported-corpus source",
        "A reader of wording cannot have seen a request, so the packet is refused at admission (V15), before any lowering.",
        "proof_packet() with addressee_consent = n4 and provenance.source = ImportedCorpus",
        &|p| {
            p.discourse.addressee_consent = Some(node("n4"));
            p.provenance.source = ProvenanceSource::ImportedCorpus;
        },
    );
    add(
        "relation",
        "A causal relation between the predications",
        "Relations are not rendered; the lowerer refuses instead of asserting a different meaning.",
        "proof_packet() with relation:8 (causes) n4 -> n5",
        &|p| {
            p.relations = vec![weksa::packet::Relation {
                relation: id("relation:8"),
                from: node("n4"),
                to: node("n5"),
                features: vec![],
            }]
        },
    );
    add(
        "dangling-filler",
        "A role filled by a node that does not exist",
        "Refused at admission (V4): the packet is not a packet.",
        "proof_packet() with predication n5 agent filler n9",
        &|p| {
            p.predications[1].roles[0] = RoleFill { role: id("role:1"), filler: node("n9") }
        },
    );
    add(
        "unknown-entity",
        "A referent naming an entity nobody holds",
        "The entity store is the only owner of names; a packet cannot invent one.",
        "proof_packet() with referent n3 entity = entity:nobody",
        &|p| p.referents[2].entity = Some(EntityId::parse("entity:nobody").expect("entity")),
    );
    add(
        "common-noun-agent",
        "The ship mind as a common noun",
        "No entity: the referent lowers as \"the ship mind\", and there is no name to pronounce.",
        "proof_packet() with referent n3 entity = None",
        &|p| p.referents[2].entity = None,
    );
    out
}

fn gloss(inv: &Inventory, rev: u32, i: &InvId) -> Value {
    match inv.resolve(i, rev) {
        Some(e) => json!({ "id": i.to_string(), "gloss": e.gloss }),
        None => json!({ "id": i.to_string(), "gloss": null }),
    }
}

fn feature_json(inv: &Inventory, rev: u32, slot: &str, at: &str, f: &FeatureValue) -> Value {
    let mut g = gloss(inv, rev, &f.feature);
    g["slot"] = json!(slot);
    g["at"] = json!(at);
    g["value"] = serde_json::to_value(&f.value).expect("value");
    g
}

/// Every feature the packet carries, with where it sits.
fn features(p: &Packet, inv: &Inventory) -> Vec<Value> {
    let rev = p.inventory_rev;
    let mut v = Vec::new();
    for f in &p.discourse.features {
        v.push(feature_json(inv, rev, "discourse", "discourse", f));
    }
    for f in &p.stance.features {
        v.push(feature_json(inv, rev, "stance", "stance", f));
    }
    for r in &p.referents {
        for f in &r.features {
            v.push(feature_json(inv, rev, "referent", r.id.as_str(), f));
        }
    }
    for pr in &p.predications {
        for f in &pr.features {
            v.push(feature_json(inv, rev, "predication", pr.id.as_str(), f));
        }
    }
    v
}

/// Ids the packet chose, once each, with the inventory's gloss at its revision.
fn chosen(p: &Packet, inv: &Inventory) -> Vec<Value> {
    let rev = p.inventory_rev;
    let mut ids: Vec<InvId> = Vec::new();
    let mut push = |i: &InvId| {
        if !ids.contains(i) {
            ids.push(*i)
        }
    };
    p.context.setting.iter().for_each(&mut push);
    p.context.medium.iter().for_each(&mut push);
    p.discourse.speech_act.iter().for_each(&mut push);
    for r in &p.referents {
        push(&r.kind);
        push(&r.concept);
    }
    for pr in &p.predications {
        push(&pr.concept);
        push(&pr.frame);
        pr.roles.iter().for_each(|f| push(&f.role));
    }
    p.relations.iter().for_each(|r| push(&r.relation));
    features(p, inv)
        .iter()
        .filter_map(|f| f["id"].as_str().and_then(|s| s.parse().ok()))
        .for_each(|i: InvId| push(&i));
    ids.iter().map(|i| gloss(inv, rev, i)).collect()
}

fn slots(p: &Packet) -> Value {
    let j = |v: &dyn erased::Ser| v.to_value();
    json!({
        "meaning": {
            "referents": p.referents.iter().map(|r| j(r)).collect::<Vec<_>>(),
            "predications": p.predications.iter().map(|r| j(r)).collect::<Vec<_>>(),
            "relations": p.relations.iter().map(|r| j(r)).collect::<Vec<_>>(),
        },
        "stance": j(&p.stance),
        "discourse": j(&p.discourse),
        "context": j(&p.context),
        "extensions": p.extensions.iter().map(|r| j(r)).collect::<Vec<_>>(),
        "constraints": j(&p.constraints),
        "provenance": { "source": j(&p.provenance.source), "corpus_item": p.provenance.corpus_item.to_string(), "confidence": p.provenance.confidence.get() },
        "kind": j(&p.kind),
        "inventory_rev": p.inventory_rev,
    })
}

mod erased {
    pub trait Ser {
        fn to_value(&self) -> serde_json::Value;
    }
    impl<T: serde::Serialize> Ser for T {
        fn to_value(&self) -> serde_json::Value {
            serde_json::to_value(self).expect("serialize")
        }
    }
}

fn lower_error(e: &LowerError) -> Value {
    let (variant, reason) = match e {
        LowerError::Unrendered { what } => ("Unrendered", format!("the packet carries {what}, which the lowerer does not render")),
        LowerError::NoRow { id } => ("NoRow", format!("no table row for {id}")),
        LowerError::GlossMismatch { id, row } => ("GlossMismatch", format!("{id} is not the {row} the row speaks for")),
        LowerError::UnknownRole { frame, role } => ("UnknownRole", format!("{frame} names role {role}, which the inventory lacks")),
        LowerError::MissingRole { predication, role } => ("MissingRole", format!("{predication} fills no {role} role")),
        LowerError::UnknownEntity { entity } => ("UnknownEntity", format!("the entity store does not hold {entity}")),
        LowerError::NoForm { entity } => ("NoForm", format!("{entity} has no en-US form")),
        LowerError::NoPronunciation { entity } => ("NoPronunciation", format!("{entity} has no ruled en-US pronunciation")),
        LowerError::NoEntityReferent => ("NoEntityReferent", "the packet cites no entity".to_string()),
        LowerError::EntityRead { entity, why } => ("EntityRead", format!("reading {entity}: {why}")),
    };
    json!({ "variant": variant, "reason": reason, "debug": format!("{e:?}") })
}

struct World {
    inventory: InventoryStore,
    corpus: CorpusStore,
    entities: EntityStore,
    inv: Inventory,
}

/// Admit `packet` through a fresh packet store and lower it. `Ok` carries the
/// text and the IPA outcome; `Err` is the refusal, as JSON.
fn run(w: &World, dir: &Path, packet: &Packet) -> Result<(String, Value), Value> {
    let mut store = PacketStore::open(&dir.join("p.cc")).expect("packet store");
    let view = match store.admit(packet.clone(), &w.inventory, &w.corpus, &w.entities) {
        Ok(v) => v,
        Err(StoreError::Invalid(rs)) => {
            return Err(json!({
                "kind": "admission_refused",
                "refusals": rs.iter().map(|r| json!({ "rule": r.rule(), "detail": format!("{r:?}") })).collect::<Vec<_>>(),
            }));
        }
        Err(e) => {
            return Err(json!({ "kind": "admission_refused", "refusals": [{ "rule": "store", "detail": format!("{e:?}") }] }));
        }
    };
    let text = lower(&view, &w.inv, &w.entities).map_err(|e| {
        json!({ "kind": "lower_error", "error": lower_error(&e) })
    })?;
    let ipa = match pronounce(&view, &w.entities) {
        Ok(v) => json!({ "ok": v }),
        Err(e) => json!({ "error": lower_error(&e) }),
    };
    Ok((text, ipa))
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let inventory = InventoryStore::open(&root.join("data/interlingua/inventory.cc")).expect("inventory");
    let inv = inventory.inventory().expect("inventory view");
    let w = World {
        corpus: CorpusStore::open(&root.join("data/corpus/corpus.cc")).expect("corpus"),
        entities: EntityStore::open(&root.join("data/entity/entity.cc")).expect("entities"),
        inventory,
        inv,
    };
    let dir = std::env::temp_dir().join(format!("show-working-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let mut out = Vec::new();
    for fx in fixtures() {
        let p = &fx.packet;
        let first = run(&w, &dir, p);
        let outcome = match first {
            Err(e) if e["kind"] == "admission_refused" => e,
            first => {
                // Ablation: run again with each feature removed, against the
                // baseline outcome. It is how the page says whether the lowerer
                // read a feature, instead of the page asserting it.
                let base = first.as_ref().ok().map(|(t, _)| t.clone());
                let n_disc = p.discourse.features.len();
                let n_stance = p.stance.features.len();
                let total = features(p, &w.inv).len();
                let mut effects = Vec::new();
                for k in 0..total {
                    let mut q = p.clone();
                    if k < n_disc {
                        q.discourse.features.remove(k);
                    } else if k < n_disc + n_stance {
                        q.stance.features.remove(k - n_disc);
                    } else {
                        let mut at = k - n_disc - n_stance;
                        'o: {
                            for r in &mut q.referents {
                                if at < r.features.len() { r.features.remove(at); break 'o; }
                                at -= r.features.len();
                            }
                            for pr in &mut q.predications {
                                if at < pr.features.len() { pr.features.remove(at); break 'o; }
                                at -= pr.features.len();
                            }
                        }
                    }
                    let _ = std::fs::remove_file(dir.join("p.cc"));
                    effects.push(match (&base, run(&w, &dir, &q)) {
                        (Some(t), Ok((u, _))) if *t == u => "Without it the text is identical: the lowerer did not read it.",
                        (Some(_), Ok(_)) => "Without it the text changes: the lowerer read it.",
                        (Some(_), Err(_)) => "Without it the packet is refused.",
                        (None, Ok(_)) => "Without it the packet lowers: this feature is what the lowerer refuses.",
                        (None, Err(_)) => "Still refused without this feature alone.",
                    });
                }
                match first {
                    Ok((text, ipa)) => json!({ "kind": "lowered", "text": text, "ipa": ipa, "feature_effects": effects }),
                    Err(e) => { let mut e = e; e["feature_effects"] = json!(effects); e }
                }
            }
        };
        let _ = std::fs::remove_file(dir.join("p.cc"));
        let mut slots = slots(p);
        slots["features"] = json!(features(p, &w.inv));
        out.push(json!({
            "id": fx.id,
            "title": fx.title,
            "note": fx.note,
            "source": fx.source,
            "packet_id": p.id.to_string(),
            "corpus_item": p.provenance.corpus_item.to_string(),
            "slots": slots,
            "chosen": chosen(p, &w.inv),
            "outcome": outcome,
        }));
    }
    let _ = std::fs::remove_dir_all(&dir);
    println!("{}", serde_json::to_string_pretty(&json!({ "fixtures": out })).expect("json"));
}

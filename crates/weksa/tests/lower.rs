//! The English lowerer against the committed inventory (revision 2) and the
//! `nibu-proof-001` packet, and the rules that keep a table row from speaking
//! for a meaning it does not carry.

mod common;
use std::path::Path;

use common::{iv, nd};
use weksa::entity::{Entity, EntityOrigin, LanguageForms};
use weksa::inventory::proof_revision;
use weksa::packet::{EntityId, FeatValue, FeatureValue, Packet, proof_packet};
use weksa::{
    CorpusStore, EntityForms, EntityStore, Inventory, InventoryStore, LowerError, LoweringInput,
    PacketStore, lower, validate,
};

const PROOF: &str = "Warning: you may endanger yourself by touching the control. \
                     Nibu will prevent that to protect you.";

fn committed() -> (Inventory, tempfile::TempDir) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let inv = InventoryStore::open(&root.join("data/interlingua/inventory.cc")).unwrap();
    (inv.inventory().unwrap(), tempfile::tempdir().unwrap())
}

/// A `LoweringInput` has one source, `PacketStore::admit`, so every packet
/// under test goes through a throwaway store backed by the committed stores.
fn view(packet: &Packet, _inv: &Inventory) -> LoweringInput {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let inventory = InventoryStore::open(&root.join("data/interlingua/inventory.cc")).unwrap();
    admitted(packet, &inventory)
}

/// The same door against an inventory store the test built.
fn admitted(packet: &Packet, inventory: &InventoryStore) -> LoweringInput {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let corpus = CorpusStore::open(&root.join("data/corpus/corpus.cc")).unwrap();
    let entities = EntityStore::open(&root.join("data/entity/entity.cc")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    PacketStore::open(&dir.path().join("p.cc"))
        .unwrap()
        .admit(packet.clone(), inventory, &corpus, &entities)
        .unwrap()
}

fn committed_entities() -> EntityStore {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    EntityStore::open(&root.join("data/entity/entity.cc")).unwrap()
}

fn lowered(packet: &Packet, inv: &Inventory) -> Result<String, LowerError> {
    lower(&view(packet, inv), inv, &committed_entities())
}

#[test]
fn the_proof_packet_is_admitted_by_the_store_and_lowers_to_its_sentences() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (inv, dir) = committed();
    let inventory = InventoryStore::open(&root.join("data/interlingua/inventory.cc")).unwrap();
    let corpus = CorpusStore::open(&root.join("data/corpus/corpus.cc")).unwrap();
    let entities = EntityStore::open(&root.join("data/entity/entity.cc")).unwrap();
    let mut packets = PacketStore::open(&dir.path().join("p.cc")).unwrap();
    packets
        .admit(proof_packet(), &inventory, &corpus, &entities)
        .unwrap();
    let stored = packets.get("packet:6e6962752d303031").unwrap().unwrap();
    assert_eq!(lowered(&stored, &inv).unwrap(), PROOF);
}

#[test]
fn only_the_addressee_is_you_and_is_reflexive() {
    let (inv, _dir) = committed();
    let mut p = proof_packet();
    p.discourse.addressee = None;
    assert_eq!(
        lowered(&p, &inv).unwrap(),
        "Warning: the player may endanger the player by touching the control. \
         Nibu will prevent that to protect the player."
    );
}

#[test]
fn no_speech_act_means_no_opener_and_a_capital() {
    let (inv, _dir) = committed();
    let mut p = proof_packet();
    p.discourse.speech_act = None;
    assert!(lowered(&p, &inv).unwrap().starts_with("You may endanger"));
    p.discourse.speech_act = Some(iv("speech_act:1"));
    assert!(lowered(&p, &inv).unwrap().starts_with("You may endanger"));
}

#[test]
fn a_packet_carrying_what_the_lowerer_does_not_render_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = InventoryStore::open(&dir.path().join("i.cc")).unwrap();
    let (rev1, entries) = weksa::inventory::seed();
    store.admit_revision(rev1, entries).unwrap();
    let (rev2, entries) = proof_revision();
    store.admit_revision(rev2, entries).unwrap();
    let mut ext = common::entry("extension:1", 3);
    ext.value_kind = Some(weksa::ValueKind::Grade);
    common::admit(&mut store, 3, vec![ext]);
    let inv = store.inventory().unwrap();
    let discourse = |p: &mut Packet| {
        p.discourse.features.push(FeatureValue {
            feature: iv("feature:6"),
            value: FeatValue::Grade(common::grade(300)),
        })
    };
    let predication = |p: &mut Packet| {
        p.predications[0].features.push(FeatureValue {
            feature: iv("feature:1"),
            value: FeatValue::Option(common::lab("negative")),
        })
    };
    let relation = |p: &mut Packet| {
        p.relations.push(weksa::packet::Relation {
            relation: iv("relation:8"),
            from: nd("n4"),
            to: nd("n5"),
            features: vec![],
        })
    };
    let extension = |p: &mut Packet| {
        p.inventory_rev = 3;
        p.extensions.push(weksa::packet::Extension {
            key: iv("extension:1"),
            value: FeatValue::Grade(common::grade(3)),
        })
    };
    let referent_feature = |p: &mut Packet| {
        p.referents[0].features.push(FeatureValue {
            feature: iv("feature:6"),
            value: FeatValue::Grade(common::grade(1)),
        })
    };
    let no_predication = |p: &mut Packet| p.predications.clear();
    let extra_role = |p: &mut Packet| {
        let filler = p.predications[0].roles[0].filler.clone();
        p.predications[0].roles.push(weksa::packet::RoleFill {
            role: iv("role:3"),
            filler,
        })
    };
    let lone_referent = |p: &mut Packet| {
        let mut r = p.referents[0].clone();
        r.id = nd("n99");
        p.referents.push(r)
    };
    for (what, edit) in [
        ("features", &discourse as &dyn Fn(&mut Packet)),
        ("features", &predication),
        ("relations", &relation),
        ("extensions", &extension),
        ("no predication", &no_predication),
        ("a role the frame does not name", &extra_role),
        ("a referent no role mentions", &lone_referent),
    ] {
        let mut p = proof_packet();
        edit(&mut p);
        assert_eq!(
            lower(&admitted(&p, &store), &inv, &committed_entities()),
            Err(LowerError::Unrendered { what }),
            "{what}"
        );
    }
    // The seed lets no feature sit on a referent, so that arm is refused by validate.
    let mut p = proof_packet();
    referent_feature(&mut p);
    assert!(validate(&p, &inv).is_err());
}

#[test]
fn an_unknown_row_and_an_unfilled_role_are_refused_by_name() {
    let (inv, _dir) = committed();
    let mut p = proof_packet();
    p.discourse.speech_act = Some(iv("speech_act:3"));
    assert_eq!(
        lowered(&p, &inv),
        Err(LowerError::NoRow {
            id: iv("speech_act:3")
        })
    );
    let mut p = proof_packet();
    p.predications[0].roles.pop();
    assert_eq!(
        lowered(&p, &inv),
        Err(LowerError::MissingRole {
            predication: nd("n4"),
            role: "patient".into()
        })
    );
}

#[test]
fn a_row_cannot_speak_for_an_id_whose_gloss_differs() {
    // Revision 2 with concept:4 glossed "hatch" in place of "control": the row
    // for concept:4 says "control" and must not render it.
    let dir = tempfile::tempdir().unwrap();
    let mut store = InventoryStore::open(&dir.path().join("i.cc")).unwrap();
    let (rev1, entries) = weksa::inventory::seed();
    store.admit_revision(rev1, entries).unwrap();
    let (rev2, mut entries) = proof_revision();
    for e in &mut entries {
        if e.id == iv("concept:4") {
            e.gloss = "hatch".into();
        }
    }
    store.admit_revision(rev2, entries).unwrap();
    let inv = store.inventory().unwrap();
    assert_eq!(
        lower(
            &admitted(&proof_packet(), &store),
            &inv,
            &committed_entities()
        ),
        Err(LowerError::GlossMismatch {
            id: iv("concept:4"),
            row: "control"
        })
    );
}

/// An entity port a test controls: ids to entities, and an optional read fault.
#[derive(Default)]
struct Entities {
    held: Vec<Entity>,
    fault: bool,
}

impl EntityForms for Entities {
    fn entity(&self, id: &EntityId) -> anyhow::Result<Option<Entity>> {
        if self.fault {
            anyhow::bail!("store unreadable");
        }
        Ok(self.held.iter().find(|e| &e.id == id).cloned())
    }
}

fn nibu(forms: &[(&str, &[&str])]) -> Entity {
    Entity {
        id: EntityId::parse("entity:nibu").unwrap(),
        forms: forms
            .iter()
            .map(|(language, forms)| LanguageForms {
                language: (*language).into(),
                forms: forms.iter().map(|f| (*f).into()).collect(),
            })
            .collect(),
        origin: EntityOrigin::Cast {
            persona: "nibu".into(),
            fixture: "f.yaml".into(),
        },
    }
}

fn through(entities: &Entities) -> Result<String, LowerError> {
    let (inv, _dir) = committed();
    lower(&view(&proof_packet(), &inv), &inv, entities)
}

#[test]
fn a_name_survives_lowering_verbatim_where_the_concept_word_was() {
    // The entity's own form, not "the ship mind", and not re-cased or inflected:
    // the form is lowercase and stays so mid-sentence.
    let mut e = Entities::default();
    let n = nibu(&[("en-US", &["nibu-of-the-deep"])]);
    e.held.push(n);
    // It opens the second sentence, where a template word would be capitalised.
    assert_eq!(
        through(&e).unwrap(),
        "Warning: you may endanger yourself by touching the control. \
         nibu-of-the-deep will prevent that to protect you."
    );
}

#[test]
fn only_the_target_languages_citation_form_is_chosen() {
    // Forms after the first are inflected shapes no English frame asks for; a
    // form under another language tag is never read, whatever order it is in.
    let mut e = Entities::default();
    let n = nibu(&[
        ("sa-Latn", &["Nibuwa"]),
        ("en-US", &["Nibu", "Nibu's", "Nibus"]),
    ]);
    e.held.push(n);
    assert_eq!(through(&e).unwrap(), PROOF);
}

#[test]
fn an_entity_the_port_does_not_hold_is_refused_not_replaced_by_its_concept_word() {
    let id = EntityId::parse("entity:nibu").unwrap();
    assert_eq!(
        through(&Entities::default()),
        Err(LowerError::UnknownEntity { entity: id })
    );
}

#[test]
fn an_entity_with_no_target_language_form_is_refused() {
    let mut e = Entities::default();
    let n = nibu(&[("sa-Latn", &["Nibuwa"])]);
    let id = n.id.clone();
    e.held.push(n);
    assert_eq!(through(&e), Err(LowerError::NoForm { entity: id }));
}

#[test]
fn a_failing_entity_port_is_refused_by_name() {
    let e = Entities {
        fault: true,
        ..Entities::default()
    };
    assert_eq!(
        through(&e),
        Err(LowerError::EntityRead {
            entity: EntityId::parse("entity:nibu").unwrap(),
            why: "store unreadable".into()
        })
    );
}

#[test]
fn an_addressee_entity_still_lowers_to_you_and_is_never_looked_up() {
    // Naming the addressee would break "you"/"yourself": the port is not asked,
    // so a port that faults on every read changes nothing.
    let (inv, _dir) = committed();
    let mut p = proof_packet();
    p.referents[0].entity = Some(EntityId::parse("entity:nibu").unwrap());
    p.referents[2].entity = None;
    let faulting = Entities {
        fault: true,
        ..Entities::default()
    };
    assert_eq!(
        lower(&view(&p, &inv), &inv, &faulting).unwrap(),
        "Warning: you may endanger yourself by touching the control. \
         The ship mind will prevent that to protect you."
    );
}

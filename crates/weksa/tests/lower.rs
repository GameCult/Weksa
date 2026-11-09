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
    PacketStore, Pronunciation, lower, pronounce, validate,
};

const PROOF: &str = "Warning: you may endanger yourself by touching the control. \
                     Nibu will stop that.";

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
         Nibu will stop that."
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
    let consent = |p: &mut Packet| p.discourse.addressee_consent = Some(nd("n5"));
    let lone_referent = |p: &mut Packet| {
        let mut r = p.referents[0].clone();
        r.id = nd("n99");
        p.referents.push(r)
    };
    for (what, edit) in [
        ("features", &discourse as &dyn Fn(&mut Packet)),
        ("features", &predication),
        ("addressee consent", &consent),
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
                pronunciation: None,
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
         nibu-of-the-deep will stop that."
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
         The ship mind will stop that."
    );
}

#[test]
fn a_purpose_the_frame_does_not_say_is_refused_not_dropped() {
    // The intervention row says "will stop"; a packet that also fills the
    // beneficiary role would assert a purpose the text does not carry.
    let (inv, _dir) = committed();
    let mut p = proof_packet();
    p.predications[1].roles.push(weksa::packet::RoleFill {
        role: iv("role:8"),
        filler: nd("n1"),
    });
    assert_eq!(
        lowered(&p, &inv),
        Err(LowerError::Unrendered {
            what: "a role the frame does not name"
        })
    );
}

fn ruled(ipa: &str, rejected: &[&str]) -> Pronunciation {
    Pronunciation {
        ipa: ipa.into(),
        ruled_by: "Nibu".into(),
        date: "2026-11-07".into(),
        source: "test".into(),
        rejected: rejected.iter().map(|r| (*r).into()).collect(),
    }
}

fn say(entities: &dyn EntityForms) -> Result<Vec<String>, LowerError> {
    let (inv, _dir) = committed();
    pronounce(&view(&proof_packet(), &inv), entities)
}

#[test]
fn nibu_round_trips_from_the_committed_seed_to_its_ruled_ipa() {
    let seed = committed_entities();
    let nibu = seed
        .get(&EntityId::parse("entity:nibu").unwrap())
        .unwrap()
        .unwrap();
    let p = nibu.pronunciation_in("en-US").unwrap();
    assert_eq!(
        (p.ipa.as_str(), p.ruled_by.as_str(), p.date.as_str()),
        ("ˈnibu", "Nibu", "2026-11-07")
    );
    assert_eq!(say(&seed).unwrap(), ["ˈnibu"]);
}

#[test]
fn a_rejected_pronunciation_is_never_emitted() {
    let mut e = Entities::default();
    let mut n = nibu(&[("en-US", &["Nibu"])]);
    n.forms[0].pronunciation = Some(ruled("ˈnibu", &["ˈnɪbu", "ˈnaɪbu"]));
    e.held.push(n);
    assert_eq!(say(&e).unwrap(), ["ˈnibu"]);
    // The committed seed carries the same rejection and still says only the ruling.
    let seed = say(&committed_entities()).unwrap().concat();
    assert!(!seed.contains('ɪ'), "{seed}");
}

#[test]
fn an_entity_with_no_ruled_pronunciation_is_refused_not_spelled_out() {
    let id = EntityId::parse("entity:nibu").unwrap();
    // No pronunciation at all.
    let mut e = Entities::default();
    e.held.push(nibu(&[("en-US", &["Nibu"])]));
    assert_eq!(
        say(&e),
        Err(LowerError::NoPronunciation { entity: id.clone() })
    );
    // A pronunciation under another language is not the target language's.
    let mut n = nibu(&[("sa-Latn", &["Nibuwa"]), ("en-US", &["Nibu"])]);
    n.forms[0].pronunciation = Some(ruled("ˈnibuwa", &[]));
    let mut e = Entities::default();
    e.held.push(n);
    assert_eq!(say(&e), Err(LowerError::NoPronunciation { entity: id }));
}

#[test]
fn ipa_needs_an_entity_the_port_holds_and_a_packet_that_cites_one() {
    assert_eq!(
        say(&Entities::default()),
        Err(LowerError::UnknownEntity {
            entity: EntityId::parse("entity:nibu").unwrap()
        })
    );
    let (inv, _dir) = committed();
    let mut p = proof_packet();
    p.referents[2].entity = None;
    assert_eq!(
        pronounce(&view(&p, &inv), &committed_entities()),
        Err(LowerError::NoEntityReferent)
    );
}

/// `nibu-proof-001` re-cut as the request: the addressee asked Nibu to stop,
/// one frame:3 predication (n4), consent naming it, and no other claim.
fn asked() -> Packet {
    let mut p = proof_packet();
    p.referents.remove(1);
    let mut ask = p.predications[1].clone();
    ask.id = nd("n4");
    ask.concept = iv("concept:6");
    ask.frame = iv("frame:3");
    ask.roles = vec![
        weksa::packet::RoleFill {
            role: iv("role:1"),
            filler: nd("n1"),
        },
        weksa::packet::RoleFill {
            role: iv("role:2"),
            filler: nd("n3"),
        },
    ];
    p.predications = vec![ask];
    p.discourse.speech_act = Some(iv("speech_act:1"));
    p.discourse.addressee_consent = Some(nd("n4"));
    p.constraints.preserve = vec![nd("n1")];
    p
}

/// The request plus a frame:2 predication (n5) the consent could wrongly name.
fn asked_and_stopped() -> Packet {
    let mut p = asked();
    let mut stop = proof_packet().predications.remove(1);
    stop.roles[1].filler = nd("n4");
    p.predications.push(stop);
    p
}

#[test]
fn a_consented_request_lowers_to_what_the_addressee_asked() {
    let (inv, _dir) = committed();
    assert_eq!(lowered(&asked(), &inv).unwrap(), "You asked Nibu to stop.");
    let mut p = asked();
    p.provenance.source = weksa::packet::ProvenanceSource::GameTelemetry;
    assert_eq!(lowered(&p, &inv).unwrap(), "You asked Nibu to stop.");
}

#[test]
fn a_request_without_its_consent_is_refused_not_invented() {
    let (inv, _dir) = committed();
    let mut p = asked();
    p.discourse.addressee_consent = None;
    assert_eq!(
        lowered(&p, &inv),
        Err(LowerError::ConsentMissing {
            predication: nd("n4")
        })
    );
    // Consent that names a different predication is no consent to the request.
    let mut p = asked_and_stopped();
    p.discourse.addressee_consent = Some(nd("n5"));
    assert_eq!(
        lowered(&p, &inv),
        Err(LowerError::ConsentMissing {
            predication: nd("n4")
        })
    );
}

#[test]
fn a_request_by_someone_other_than_the_addressee_is_refused() {
    let (inv, _dir) = committed();
    let mut p = asked();
    p.predications[0].roles[0].filler = nd("n3");
    p.predications[0].roles[1].filler = nd("n1");
    assert_eq!(
        lowered(&p, &inv),
        Err(LowerError::AskerNotAddressee {
            predication: nd("n4")
        })
    );
}

#[test]
fn consent_on_a_claim_that_is_no_request_stays_unrendered() {
    let (inv, _dir) = committed();
    let mut p = proof_packet();
    p.discourse.addressee_consent = Some(nd("n5"));
    assert_eq!(
        lowered(&p, &inv),
        Err(LowerError::Unrendered {
            what: "addressee consent"
        })
    );
}

#[test]
fn a_decomposition_or_import_cannot_reach_the_lowerer_with_consent() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let inventory = InventoryStore::open(&root.join("data/interlingua/inventory.cc")).unwrap();
    let corpus = CorpusStore::open(&root.join("data/corpus/corpus.cc")).unwrap();
    let entities = EntityStore::open(&root.join("data/entity/entity.cc")).unwrap();
    for source in [
        weksa::packet::ProvenanceSource::ModelDecomposition,
        weksa::packet::ProvenanceSource::ImportedCorpus,
    ] {
        let mut p = asked();
        p.provenance.source = source;
        let dir = tempfile::tempdir().unwrap();
        let refused = PacketStore::open(&dir.path().join("p.cc"))
            .unwrap()
            .admit(p, &inventory, &corpus, &entities)
            .unwrap_err();
        assert!(format!("{refused:?}").contains("V15"), "{source:?}");
    }
}

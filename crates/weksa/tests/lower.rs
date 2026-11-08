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
    PacketStore, PhoneInventory, PhoneLine, Pronunciation, Stress, lower, pronounce, validate,
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

// ---- the phones line: the phone inventory and the segmenter ----

fn phones_of(ipa: &str) -> Result<Vec<PhoneLine>, LowerError> {
    let mut n = nibu(&[("en-US", &["Nibu"])]);
    n.forms[0].pronunciation = Some(ruled(ipa, &[]));
    let mut e = Entities::default();
    e.held.push(n);
    let (inv, _dir) = committed();
    weksa::phones(&view(&proof_packet(), &inv), &e)
}

fn seg(ipa: &str) -> Vec<(String, Stress)> {
    let inv = PhoneInventory::for_language("en-US").unwrap();
    inv.segment(ipa)
        .unwrap()
        .into_iter()
        .map(|p| (p.ipa, p.stress))
        .collect()
}

fn pairs(v: &[(&str, Stress)]) -> Vec<(String, Stress)> {
    v.iter().map(|(i, s)| ((*i).to_string(), *s)).collect()
}

#[test]
fn nibus_committed_ruling_lowers_to_the_committed_phones_fixture() {
    let (inv, _dir) = committed();
    let lines = weksa::phones(&view(&proof_packet(), &inv), &committed_entities()).unwrap();
    let fixture = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/phonology/fixtures/nibu-en-US.phones.json"),
    )
    .unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(serde_json::to_string(&lines[0]).unwrap(), fixture.trim_end());
}

#[test]
fn tense_i_is_a_phone_and_the_rejected_lax_form_is_a_different_one() {
    use weksa::phones::{Class, Tenseness};
    let inv = PhoneInventory::for_language("en-US").unwrap();
    assert_eq!(inv.class("i"), Some(&Class::Vowel(Tenseness::Tense)));
    assert_eq!(inv.class("ɪ"), Some(&Class::Vowel(Tenseness::Lax)));
    assert_eq!(inv.class("u"), Some(&Class::Vowel(Tenseness::Tense)));
    let ruled_form = seg("ˈnibu");
    let lax = seg("ˈnɪbu");
    assert_ne!(ruled_form, lax);
    assert_eq!(lax[1], ("ɪ".to_string(), Stress::Primary));
    // The committed seed rejects the lax form, and it is not what the seed emits.
    let seed = committed_entities();
    let nibu = seed.get(&EntityId::parse("entity:nibu").unwrap()).unwrap().unwrap();
    let p = nibu.pronunciation_in("en-US").unwrap();
    assert_eq!(p.rejected, ["ˈnɪbu"]);
    assert_eq!(seg(&p.ipa), ruled_form);
    assert!(p.rejected.iter().all(|r| seg(r) != seg(&p.ipa)));
}

#[test]
fn stress_lands_on_the_vowel_after_the_mark_and_nowhere_else() {
    use Stress::{None as N, Primary as P, Secondary as S};
    assert_eq!(seg("ˈnibu"), pairs(&[("n", N), ("i", P), ("b", N), ("u", N)]));
    assert_eq!(seg("nˈibu"), seg("ˈnibu"));
    assert_eq!(seg("bˈunˌi"), pairs(&[("b", N), ("u", P), ("n", N), ("i", S)]));
    assert_eq!(seg("nibu"), pairs(&[("n", N), ("i", N), ("b", N), ("u", N)]));
}

#[test]
fn multi_character_phones_are_one_phone_by_longest_match() {
    use Stress::{None as N, Primary as P};
    assert_eq!(seg("ˈtʃeɪ"), pairs(&[("tʃ", N), ("eɪ", P)]));
    assert_eq!(seg("dʒaʊ"), pairs(&[("dʒ", N), ("aʊ", N)]));
    // /t/ then /ʃ/ is not what a lone "t" and "ʃ" mean when the affricate is listed,
    // but a symbol sequence the inventory cannot join still splits.
    assert_eq!(seg("ɔɪt"), pairs(&[("ɔɪ", N), ("t", N)]));
}

#[test]
fn a_phone_the_inventory_lacks_is_refused_with_where_and_never_guessed() {
    let id = EntityId::parse("entity:nibu").unwrap();
    for (ipa, at, text) in [
        ("ˈnixu", "ˈni".len(), "xu"),
        ("ˈniːbu", "ˈni".len(), "ːbu"),
        ("ˈnibu.", "ˈnibu".len(), "."),
        ("ˈni bu", "ˈni".len(), " bu"),
    ] {
        assert_eq!(
            phones_of(ipa),
            Err(LowerError::UnknownPhone { entity: id.clone(), at, text: text.into() }),
            "{ipa}"
        );
    }
}

#[test]
fn a_stress_mark_with_no_vowel_to_land_on_is_refused() {
    let id = EntityId::parse("entity:nibu").unwrap();
    for (ipa, at) in [("nibuˈ", "nibu".len()), ("ˈˈnibu", 0), ("ˈnˈibu", 0), ("ˈnb", 0)] {
        assert_eq!(
            phones_of(ipa),
            Err(LowerError::DanglingStress { entity: id.clone(), at }),
            "{ipa}"
        );
    }
}

#[test]
fn phones_need_a_ruling_and_an_entity_exactly_as_ipa_does() {
    assert_eq!(
        weksa::phones(&view(&proof_packet(), &committed().0), &Entities::default()),
        Err(LowerError::UnknownEntity { entity: EntityId::parse("entity:nibu").unwrap() })
    );
    let mut e = Entities::default();
    e.held.push(nibu(&[("en-US", &["Nibu"])]));
    assert_eq!(
        weksa::phones(&view(&proof_packet(), &committed().0), &e),
        Err(LowerError::NoPronunciation { entity: EntityId::parse("entity:nibu").unwrap() })
    );
}

#[test]
fn the_inventory_parser_refuses_what_it_cannot_read() {
    for bad in [
        "i\tvowel\ttense",
        "i\tvowel\t-\tx",
        "n\tconsonant\ttense\tx",
        "i\tvowel\tmid\tx",
        "ˈi\tvowel\ttense\tx",
        "\tvowel\ttense\tx",
        "i\tvowel\ttense\t",
        "i\tvowel\ttense\ta\ni\tvowel\tlax\tb",
    ] {
        assert!(PhoneInventory::parse(bad).is_err(), "{bad:?}");
    }
    assert!(PhoneInventory::parse("# c\n\ni\tvowel\ttense\ta\n").is_ok());
    assert!(PhoneInventory::for_language("sa-Latn").is_none());
}

//! The entity port, `pronounce` and `phones` against the committed stores and
//! the `nibu-proof-001` packet.

mod common;
use std::path::Path;

use weksa::entity::{Entity, EntityOrigin, LanguageForms};
use weksa::packet::{EntityId, Packet, proof_packet};
use weksa::{
    CorpusStore, EntityForms, EntityStore, Inventory, InventoryStore, LowerError, LoweringInput,
    PacketStore, PhoneInventory, PhoneLine, Pronunciation, Stress, pronounce,
};

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
fn a_failing_entity_port_is_refused_by_name() {
    let e = Entities {
        fault: true,
        ..Entities::default()
    };
    assert_eq!(
        say(&e),
        Err(LowerError::EntityRead {
            entity: EntityId::parse("entity:nibu").unwrap(),
            why: "store unreadable".into()
        })
    );
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

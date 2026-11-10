mod common;

use common::{cast, ent, entities_in, group_in_bucket, item_in, model, world};
use weksa::context::{EncoderOffer, OfferRefusal, Prompt, TrainItems};
use weksa::split::Checkpoint;
use weksa::{
    CorpusItem, CorpusItemId, CorpusStore, NameSpan, PacketId, PacketStore, StoreError,
    StoreRefusal, Turn,
};

const MINT: &str = "Mint glosses from the lines below.";
const ENCODE: &str = "Encode the line below.";

struct Corpus {
    _dir: tempfile::TempDir,
    store: CorpusStore,
}

/// One live item per split, a withdrawn Train item and a model-written
/// Test-group item, each with a marker word unique to it.
fn corpus() -> Corpus {
    let dir = tempfile::tempdir().unwrap();
    let entities = entities_in(dir.path());
    let mut store = CorpusStore::open(&dir.path().join("c.cc")).unwrap();
    let mut put = |item: CorpusItem| {
        let withdrawn = item.withdrawn;
        let id = item.id.clone();
        store
            .add(
                CorpusItem {
                    withdrawn: false,
                    ..item
                },
                &entities,
            )
            .unwrap();
        if withdrawn {
            store.withdraw(&id).unwrap();
        }
    };
    put(item_in(
        "item:a:1",
        "alphamarker speaks",
        &group_in_bucket(10),
        cast(),
    ));
    put(item_in(
        "item:b:1",
        "betamarker speaks",
        &group_in_bucket(50),
        cast(),
    ));
    put(item_in(
        "item:c:1",
        "gammamarker speaks",
        &group_in_bucket(90),
        cast(),
    ));
    let mut gone = item_in(
        "item:a:2",
        "deltamarker speaks",
        &group_in_bucket(10),
        cast(),
    );
    gone.withdrawn = true;
    put(gone);
    put(item_in(
        "item:c:2",
        "epsilonmarker speaks",
        &group_in_bucket(90),
        model(),
    ));
    Corpus { _dir: dir, store }
}

fn get(c: &Corpus, key: &str) -> CorpusItem {
    c.store.get(key).unwrap().unwrap()
}

#[test]
fn train_items_hold_only_live_train() {
    let c = corpus();
    let t = TrainItems::from_corpus(&c.store).unwrap();
    assert_eq!(t.keys().collect::<Vec<_>>(), ["item:a:1"]);
    assert_eq!((t.len(), t.is_empty()), (1, false));
}

#[test]
fn train_items_are_in_key_order() {
    let dir = tempfile::tempdir().unwrap();
    let entities = entities_in(dir.path());
    let mut store = CorpusStore::open(&dir.path().join("c.cc")).unwrap();
    for (key, text) in [
        ("item:z:1", "zed line"),
        ("item:a:1", "ay line"),
        ("item:m:1", "em line"),
    ] {
        store
            .add(item_in(key, text, &group_in_bucket(10), cast()), &entities)
            .unwrap();
    }
    let t = TrainItems::from_corpus(&store).unwrap();
    assert_eq!(
        t.keys().collect::<Vec<_>>(),
        ["item:a:1", "item:m:1", "item:z:1"]
    );
}

#[test]
fn mint_prompt_holds_train_text_only() {
    let c = corpus();
    let t = TrainItems::from_corpus(&c.store).unwrap();
    let p = Prompt::mint(MINT, &t);
    assert!(p.text().starts_with(MINT));
    assert!(p.text().contains("alphamarker"));
    for other in ["betamarker", "gammamarker", "deltamarker", "epsilonmarker"] {
        assert!(!p.text().contains(other), "{other}");
    }
    assert_eq!(p.item_keys(), ["item:a:1"]);
}

#[test]
fn prompts_render_deterministically() {
    let dir = tempfile::tempdir().unwrap();
    let entities = entities_in(dir.path());
    let mut store = CorpusStore::open(&dir.path().join("c.cc")).unwrap();
    let mut one = item_in("item:a:1", "Hello there.", &group_in_bucket(10), cast());
    one.prior_turns = vec![Turn {
        speaker: common::eid("brokkr"),
        text: "Well?".into(),
    }];
    store.add(one, &entities).unwrap();
    store
        .add(
            item_in("item:a:2", "Second line.", &group_in_bucket(10), cast()),
            &entities,
        )
        .unwrap();
    let t = TrainItems::from_corpus(&store).unwrap();
    assert_eq!(
        Prompt::mint(MINT, &t).text(),
        format!(
            "{MINT}\n\nentity:brokkr: Well?\n[item:a:1] entity:nibu: Hello there.\n\n[item:a:2] entity:nibu: Second line."
        )
    );
    assert_eq!(Prompt::mint(MINT, &t).item_keys(), ["item:a:1", "item:a:2"]);

    let offer = EncoderOffer::for_round(&store.get("item:a:2").unwrap().unwrap()).unwrap();
    assert_eq!(
        Prompt::encode(ENCODE, &offer).text(),
        format!("{ENCODE}\n\n[item:a:2] entity:nibu: Second line.\n\nentities: entity:nibu")
    );
    assert_eq!(Prompt::encode(ENCODE, &offer).item_keys(), ["item:a:2"]);
}

#[test]
fn encoder_offer_split_gate() {
    let c = corpus();
    let cp = Checkpoint::declare(1);
    let train = get(&c, "item:a:1");
    let validation = get(&c, "item:b:1");
    let test = get(&c, "item:c:1");
    let no_split = get(&c, "item:c:2");
    let withdrawn = get(&c, "item:a:2");
    assert!(withdrawn.withdrawn);
    let round = |i: &CorpusItem| EncoderOffer::for_round(i).err();
    let check = |i: &CorpusItem| EncoderOffer::for_checkpoint(i, &cp).err();

    assert_eq!(round(&train), None);
    assert_eq!(round(&validation), None);
    assert_eq!(round(&test), Some(OfferRefusal::TestBeforeCheckpoint));
    assert_eq!(round(&no_split), Some(OfferRefusal::NoSplit));
    assert_eq!(round(&withdrawn), Some(OfferRefusal::Withdrawn));

    assert_eq!(check(&train), None);
    assert_eq!(check(&validation), None);
    assert_eq!(check(&test), None);
    assert_eq!(check(&no_split), Some(OfferRefusal::NoSplit));
    assert_eq!(check(&withdrawn), Some(OfferRefusal::Withdrawn));

    // Withdrawn wins over NoSplit.
    let mut both = no_split.clone();
    both.withdrawn = true;
    assert_eq!(round(&both), Some(OfferRefusal::Withdrawn));
    assert_eq!(check(&both), Some(OfferRefusal::Withdrawn));
    // A withdrawn Test item is Withdrawn, not TestBeforeCheckpoint.
    let mut test_gone = test.clone();
    test_gone.withdrawn = true;
    assert_eq!(round(&test_gone), Some(OfferRefusal::Withdrawn));
}

fn span(slug: &str, start: u32, end: u32) -> NameSpan {
    NameSpan {
        entity: common::eid(slug),
        start,
        end,
    }
}

/// An item naming kay (e1), spoken by hero (e2) after a turn by brokkr (e3),
/// and a sibling of the same group naming someone (e4).
fn cited_world() -> (Corpus, CorpusItem, CorpusItem) {
    let dir = tempfile::tempdir().unwrap();
    let mut entities = entities_in(dir.path());
    entities.put(ent("kay", "en-US", &["Kay"])).unwrap();
    let mut store = CorpusStore::open(&dir.path().join("c.cc")).unwrap();
    let group = group_in_bucket(10);
    let mut mine = item_in("item:a:1", "Kay waits here.", &group, cast());
    mine.speaker = common::eid("hero");
    mine.names = vec![span("kay", 0, 3)];
    mine.prior_turns = vec![
        Turn {
            speaker: common::eid("brokkr"),
            text: "Where?".into(),
        },
        Turn {
            speaker: common::eid("brokkr"),
            text: "Again?".into(),
        },
    ];
    let mut sibling = item_in("item:a:2", "Someone waits there.", &group, cast());
    sibling.names = vec![span("someone", 0, 7)];
    store.add(mine.clone(), &entities).unwrap();
    store.add(sibling.clone(), &entities).unwrap();
    (Corpus { _dir: dir, store }, mine, sibling)
}

#[test]
fn encoder_offer_entities_are_the_items_own() {
    let (_c, mine, sibling) = cited_world();
    assert_eq!(
        mine.cited_entities()
            .map(|e| e.as_str())
            .collect::<Vec<_>>(),
        [
            "entity:kay",
            "entity:hero",
            "entity:brokkr",
            "entity:brokkr"
        ],
        "names, speaker, prior-turn speakers, repeats kept"
    );
    let offer = EncoderOffer::for_round(&mine).unwrap();
    let ids: Vec<&str> = offer.entities().iter().map(|e| e.as_str()).collect();
    assert_eq!(ids, ["entity:brokkr", "entity:hero", "entity:kay"]);
    assert!(
        !ids.contains(&"entity:someone"),
        "a sibling's entity is not the item's"
    );
    assert_eq!(offer.key(), "item:a:1");
    let p = Prompt::encode(ENCODE, &offer);
    assert!(
        p.text()
            .ends_with("entities: entity:brokkr, entity:hero, entity:kay")
    );
    assert!(p.text().contains("[item:a:1]"));
    assert!(!p.text().contains("item:a:2") && !p.text().contains("Someone waits"));
    assert_eq!(p.item_keys(), ["item:a:1"]);
    let sib = EncoderOffer::for_round(&sibling).unwrap();
    assert_eq!(sib.entities().len(), 2);
}

#[test]
fn offer_and_store_agree() {
    let mut w = world();
    w.entities.put(ent("kay", "en-US", &["Kay"])).unwrap();
    let mut item = item_in(
        "item:nibu:2",
        "Kay waits here.",
        &group_in_bucket(10),
        cast(),
    );
    item.speaker = common::eid("hero");
    item.names = vec![span("kay", 0, 3)];
    item.prior_turns = vec![Turn {
        speaker: common::eid("brokkr"),
        text: "Where?".into(),
    }];
    w.corpus.add(item.clone(), &w.entities).unwrap();
    let offer = EncoderOffer::for_round(&item).unwrap();

    let mut packets = PacketStore::open(&w.dir.path().join("p.cc")).unwrap();
    let all = ["nibu", "hero", "kay", "brokkr", "player", "s", "someone"];
    for (n, slug) in all.iter().enumerate() {
        let mut p = common::good();
        p.id = PacketId::parse(&format!("packet:{n:016x}")).unwrap();
        p.provenance.corpus_item = CorpusItemId::parse("item:nibu:2").unwrap();
        p.referents[0].entity = Some(common::eid(slug));
        let offered = offer.entities().contains(&common::eid(slug));
        match packets.admit(p, &w.inventory, &w.corpus, &w.entities) {
            Ok(_) => assert!(offered, "{slug} admitted but not offered"),
            Err(StoreError::Refused(StoreRefusal::EntityNotOnItem { .. })) => {
                assert!(!offered, "{slug} offered but refused")
            }
            Err(other) => panic!("{slug}: {other:?}"),
        }
    }
}

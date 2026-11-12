mod common;

use common::{cast, ent, entities_in, group_in_bucket, item_in, model, run_over, world};
use weksa::context::{
    ENCODE_INSTRUCTIONS as ENCODE, EditFeedback, MINT_INSTRUCTIONS as MINT, OfferRefusal, Prompt,
};
use weksa::run::RunStores;
use weksa::{
    CorpusItem, CorpusItemId, CorpusStore, NameSpan, PacketId, PacketStore, StoreError,
    StoreRefusal, Turn,
};

struct Corpus {
    _dir: tempfile::TempDir,
    store: CorpusStore,
    run: RunStores,
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
    let run = run_over(dir.path());
    Corpus {
        _dir: dir,
        store,
        run,
    }
}

fn get(c: &Corpus, key: &str) -> CorpusItem {
    c.store.get(key).unwrap().unwrap()
}

#[test]
fn train_items_hold_only_live_train() {
    let c = corpus();
    let t = c.run.train().unwrap();
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
    let t = run_over(dir.path()).train().unwrap();
    assert_eq!(
        t.keys().collect::<Vec<_>>(),
        ["item:a:1", "item:m:1", "item:z:1"]
    );
    assert_eq!((t.len(), t.is_empty()), (3, false));
}

#[test]
fn mint_prompt_holds_train_text_only() {
    let c = corpus();
    let t = c.run.train().unwrap();
    let inv = c.run.inventory().inventory().unwrap();
    let p = Prompt::mint(&t, &inv, 4, &EditFeedback::none());
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
    let run = run_over(dir.path());
    let t = run.train().unwrap();
    let inv = run.inventory().inventory().unwrap();
    let fb = EditFeedback::none();
    assert_eq!(
        Prompt::mint(&t, &inv, 0, &fb).text(),
        format!(
            "{MINT}\n\nlast edit: none; validation composite at head: none\n\ninventory rev:0\n\n[item:a:1] entity:nibu: Hello there.\n\n[item:a:2] entity:nibu: Second line."
        )
    );
    assert_eq!(
        Prompt::mint(&t, &inv, 0, &fb).item_keys(),
        ["item:a:1", "item:a:2"]
    );

    let offer = run.offer("item:a:2").unwrap().unwrap();
    assert_eq!(
        Prompt::encode(&offer, &t, &inv, 0).text(),
        format!(
            "{ENCODE}\n\n[item:a:2] entity:nibu: Second line.\n\nentities: entity:nibu\n\ninventory rev:0"
        )
    );
    assert_eq!(
        Prompt::encode(&offer, &t, &inv, 0).item_keys(),
        ["item:a:2"]
    );
}

#[test]
fn encoder_offer_split_gate() {
    let c = corpus();
    let round = |key: &str| c.run.offer(key).unwrap().err();
    assert_eq!(round("item:a:1"), None);
    assert_eq!(round("item:b:1"), None);
    assert_eq!(round("item:c:1"), Some(OfferRefusal::TestBeforeCheckpoint));
    assert_eq!(round("item:c:2"), Some(OfferRefusal::NoSplit));
    assert_eq!(round("item:a:2"), Some(OfferRefusal::Withdrawn));
    assert!(get(&c, "item:a:2").withdrawn);
    let absent = c.run.offer("item:z:9").err().unwrap().to_string();
    assert!(!absent.contains("item:z:9"), "an absent key is not echoed");
}

#[test]
fn offers_are_the_live_train_and_validation_items_in_key_order() {
    let c = corpus();
    let keys: Vec<String> = c
        .run
        .offers()
        .unwrap()
        .iter()
        .map(|o| o.key().to_string())
        .collect();
    assert_eq!(keys, ["item:a:1", "item:b:1"]);
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
    let run = run_over(dir.path());
    (
        Corpus {
            _dir: dir,
            store,
            run,
        },
        mine,
        sibling,
    )
}

#[test]
fn encoder_offer_entities_are_the_items_own() {
    let (c, mine, _sibling) = cited_world();
    let t = c.run.train().unwrap();
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
    let offer = c.run.offer("item:a:1").unwrap().unwrap();
    let ids: Vec<&str> = offer.entities().iter().map(|e| e.as_str()).collect();
    assert_eq!(ids, ["entity:brokkr", "entity:hero", "entity:kay"]);
    assert!(
        !ids.contains(&"entity:someone"),
        "a sibling's entity is not the item's"
    );
    assert_eq!(offer.key(), "item:a:1");
    let inv = c.run.inventory().inventory().unwrap();
    let p = Prompt::encode(&offer, &t, &inv, 0);
    assert!(
        p.text()
            .ends_with("entities: entity:brokkr, entity:hero, entity:kay\n\ninventory rev:0")
    );
    assert!(p.text().contains("[item:a:1]"));
    assert!(!p.text().contains("item:a:2") && !p.text().contains("Someone waits"));
    assert_eq!(p.item_keys(), ["item:a:1"]);
    let sib = c.run.offer("item:a:2").unwrap().unwrap();
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
    let offer = run_over(w.dir.path())
        .offer("item:nibu:2")
        .unwrap()
        .unwrap();

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

/// Items of one Train group and of a Test group, with the lines each prior
/// turn quotes.
#[test]
fn prior_turns_print_only_text_a_live_train_item_holds() {
    let dir = tempfile::tempdir().unwrap();
    let entities = entities_in(dir.path());
    let mut store = CorpusStore::open(&dir.path().join("c.cc")).unwrap();
    let (train_g, other_train_g) = (group_in_bucket(10), group_in_bucket(11));
    let turn = |text: &str| Turn {
        speaker: common::eid("brokkr"),
        text: text.into(),
    };
    let mut add = |key: &str, text: &str, group: &str, turns: Vec<Turn>| {
        let mut item = item_in(key, text, group, cast());
        item.prior_turns = turns;
        store.add(item, &entities).unwrap();
    };
    add("item:a:1", "withdrawnsecret line", &train_g, vec![]);
    add("item:a:4", "livecontext line", &train_g, vec![]);
    add(
        "item:a:2",
        "reply line",
        &train_g,
        vec![turn("withdrawnsecret line")],
    );
    add(
        "item:a:3",
        "second reply",
        &train_g,
        vec![turn("testsecret line")],
    );
    add(
        "item:a:5",
        "third reply",
        &train_g,
        vec![turn("livecontext line")],
    );
    add(
        "item:a:6",
        "other reply",
        &train_g,
        vec![turn("elsewhere line")],
    );
    add("item:o:1", "elsewhere line", &other_train_g, vec![]);
    add(
        "item:b:1",
        "validation reply",
        &group_in_bucket(50),
        vec![turn("testsecret line")],
    );
    add("item:c:1", "testsecret line", &group_in_bucket(90), vec![]);
    store.withdraw("item:a:1").unwrap();

    let run = run_over(dir.path());
    let t = run.train().unwrap();
    let inv = run.inventory().inventory().unwrap();
    let mint = Prompt::mint(&t, &inv, 0, &EditFeedback::none());
    for secret in ["withdrawnsecret", "testsecret", "validation reply"] {
        assert!(!mint.text().contains(secret), "mint printed {secret}");
    }
    assert!(
        !mint.text().contains("brokkr"),
        "mint prints no prior turns"
    );

    let encode = |key: &str| {
        Prompt::encode(&run.offer(key).unwrap().unwrap(), &t, &inv, 0)
            .text()
            .to_string()
    };
    let withdrawn = encode("item:a:2");
    assert!(!withdrawn.contains("withdrawnsecret"));
    assert!(withdrawn.contains("entity:brokkr: [elided]\n[item:a:2]"));
    let test_quote = encode("item:a:3");
    assert!(!test_quote.contains("testsecret"));
    assert!(test_quote.contains("entity:brokkr: [elided]"));
    let validation = encode("item:b:1");
    assert!(!validation.contains("testsecret"));
    assert!(validation.contains("entity:brokkr: [elided]"));
    assert!(encode("item:a:5").contains("entity:brokkr: livecontext line\n[item:a:5]"));
    let cross_group = encode("item:a:6");
    assert!(
        !cross_group.contains("elsewhere"),
        "a held line of another group is not this group's"
    );
}

/// A scratch Train store holding `train_text`, and an encode prompt for a
/// sibling whose one prior turn is `prior_text`.
fn encode_with_prior(train_text: &str, prior_text: &str) -> (String, Result<(), StoreError>) {
    let dir = tempfile::tempdir().unwrap();
    let entities = entities_in(dir.path());
    let mut store = CorpusStore::open(&dir.path().join("c.cc")).unwrap();
    let g = group_in_bucket(10);
    store
        .add(item_in("item:a:1", train_text, &g, cast()), &entities)
        .unwrap();
    let mut reply = item_in("item:a:2", "reply", &g, cast());
    reply.prior_turns = vec![Turn {
        speaker: common::eid("brokkr"),
        text: prior_text.into(),
    }];
    store.add(reply.clone(), &entities).unwrap();
    let run = run_over(dir.path());
    let t = run.train().unwrap();
    let inv = run.inventory().inventory().unwrap();
    let p = Prompt::encode(&run.offer("item:a:2").unwrap().unwrap(), &t, &inv, 0);
    let text = p.text().to_string();
    // The store's own rule, as the oracle for key equality; after the prompt is
    // built, because an admitted probe would itself become a held line.
    let probe = store.add(item_in("item:a:9", prior_text, &g, cast()), &entities);
    (text, probe.map(|_| ()))
}

#[test]
fn prior_turn_prints_the_held_train_text() {
    let (text, _) = encode_with_prior("Context Line", "context-line!! :)");
    assert!(
        text.contains("entity:brokkr: Context Line\n[item:a:2]"),
        "{text}"
    );
    for turn_bytes in ["!!", ":)", "context-line"] {
        assert!(!text.contains(turn_bytes), "{turn_bytes} in {text}");
    }
}

#[test]
fn prior_turn_match_uses_the_stores_text_key() {
    let (text, _) = encode_with_prior("Context Line", "context line");
    assert!(text.contains("entity:brokkr: Context Line\n"));
    assert!(!text.contains("context line"));
}

#[test]
fn prior_turn_fold_matches_the_store() {
    let rows = [
        ("case", "Context Line", "context line"),
        (
            "punctuation and spacing",
            "Context Line",
            "context,   line!",
        ),
        (
            "NFKC fullwidth letters",
            "Context",
            "\u{ff23}\u{ff4f}\u{ff4e}\u{ff54}\u{ff45}\u{ff58}\u{ff54}",
        ),
        ("NFKC fullwidth digits", "room 12", "room \u{ff11}\u{ff12}"),
        ("dotless i", "Ilik", "\u{131}l\u{131}k"),
        ("dotted capital I", "Istanbul", "\u{130}stanbul"),
        (
            "katakana vs hiragana",
            "\u{3072}\u{3089}\u{304c}\u{306a}",
            "\u{30d2}\u{30e9}\u{30ac}\u{30ca}",
        ),
        ("voiced katakana wa", "\u{308f}\u{3099}", "\u{30f7}"),
        ("Arabic-Indic digits", "room 12", "room \u{661}\u{662}"),
    ];
    for (fold, train_text, prior_text) in rows {
        let (text, probe) = encode_with_prior(train_text, prior_text);
        assert!(
            matches!(
                probe,
                Err(StoreError::Refused(StoreRefusal::S6TextHeld { .. }))
            ),
            "{fold}: the store does not hold these as one text: {probe:?}"
        );
        assert!(
            text.contains(&format!("entity:brokkr: {train_text}\n[item:a:2]")),
            "{fold}: {text}"
        );
        assert!(!text.contains(ELIDED_MARK), "{fold}: {text}");
    }
    // A text differing by a letter is a different key: the store admits it and
    // the prior turn is elided.
    let (text, probe) = encode_with_prior("Context Line", "Context Lane");
    assert!(probe.is_ok(), "{probe:?}");
    assert!(
        text.contains("entity:brokkr: [elided]\n[item:a:2]"),
        "{text}"
    );
}

const ELIDED_MARK: &str = "[elided]";

#[test]
fn mint_and_encode_list_the_same_inventory() {
    let w = world();
    let run = run_over(w.dir.path());
    let inv = run.inventory().inventory().unwrap();
    let t = run.train().unwrap();
    let offer = run.offer("item:nibu:1").unwrap().unwrap();
    let fb = EditFeedback::none();
    for rev in [2, 4] {
        let header = format!("inventory rev:{rev}\n");
        let mint = Prompt::mint(&t, &inv, rev, &fb);
        let encode = Prompt::encode(&offer, &t, &inv, rev);
        let from_mint = mint.text().split_once(&header).unwrap().1;
        let from_mint = from_mint.split("\n\n").next().unwrap();
        let from_encode = encode.text().split_once(&header).unwrap().1;
        assert!(!from_mint.is_empty());
        assert_eq!(from_mint, from_encode, "rev {rev}");
    }
    let at = |rev| {
        let text = Prompt::mint(&t, &inv, rev, &fb).text().to_string();
        text.split_once("inventory rev:").unwrap().1.to_string()
    };
    assert_ne!(at(2), at(4), "the revision selects the entries");
}

mod lowering {
    use std::path::{Path, PathBuf};

    use weksa::context::{LOWER_INSTRUCTIONS as LOWER, Prompt};
    use weksa::daodejing::packets;
    use weksa::entity::{Entity, EntityOrigin, LanguageForms};
    use weksa::packet::{EntityId, Packet, proof_packet};
    use weksa::packet::Grade;
    use weksa::{
        Annotator, Condition, CorpusItemId, CorpusStore, EntityForms, EntityStore, Inventory,
        InventoryStore, Layer, LowerError, LoweringInput, OntologyEntry, OntologyStore, PacketStore, Test, select,
    };

    fn data(sub: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").join(sub)
    }

    /// The admitted view, its scene never read.
    fn admitted_unread(
        packet: Packet,
        inv: &InventoryStore,
        corpus: &str,
        entities: &EntityStore,
    ) -> LoweringInput {
        let corpus = CorpusStore::open(&data(corpus)).unwrap();
        let dir = tempfile::tempdir().unwrap();
        PacketStore::open(&dir.path().join("p.cc"))
            .unwrap()
            .admit(packet, inv, &corpus, entities)
            .unwrap()
    }

    /// The admitted view with its scene read over an empty speaker-state store.
    fn admitted(
        packet: Packet,
        inv: &InventoryStore,
        corpus: &str,
        entities: &EntityStore,
    ) -> LoweringInput {
        let view = admitted_unread(packet, inv, corpus, entities);
        let dir = tempfile::tempdir().unwrap();
        let state = weksa::SpeakerStateStore::open(&dir.path().join("s.cc")).unwrap();
        let corpus = CorpusStore::open(&data(corpus)).unwrap();
        view.with_scene(&inv.inventory().unwrap(), &state, &corpus).unwrap()
    }

    #[test]
    fn lowering_prompt_refuses_an_unread_scene() {
        let cast = InventoryStore::open(&data("interlingua/inventory.cc")).unwrap();
        let entities = EntityStore::open(&data("entity/entity.cc")).unwrap();
        let inv = cast.inventory().unwrap();
        let item = CorpusItemId::parse("item:nibu:1").unwrap();
        let held = Held(vec![nibu(&[("en-US", &["Nibu"])])]);
        let unread = admitted_unread(proof_packet(), &cast, "corpus/corpus.cc", &entities);
        let empty = weksa::Ontology::empty();
        assert_eq!(
            Prompt::lower(&unread, &item, "en-US", &inv, &held, &empty).err(),
            Some(LowerError::SceneUnread)
        );
        let read = admitted(proof_packet(), &cast, "corpus/corpus.cc", &entities);
        assert!(Prompt::lower(&read, &item, "en-US", &inv, &held, &empty).is_ok());
    }

    fn dao(index: usize) -> (LoweringInput, Inventory, EntityStore, CorpusItemId) {
        let inv = InventoryStore::open(&data("daodejing/inventory.cc")).unwrap();
        let entities = EntityStore::open(&data("daodejing/entity.cc")).unwrap();
        let packet = packets().remove(index);
        let item = packet.provenance.corpus_item.clone();
        let view = admitted(packet, &inv, "daodejing/corpus.cc", &entities);
        (view, inv.inventory().unwrap(), entities, item)
    }

    /// An entity port a test controls.
    struct Held(Vec<Entity>);

    impl EntityForms for Held {
        fn entity(&self, id: &EntityId) -> anyhow::Result<Option<Entity>> {
            Ok(self.0.iter().find(|e| &e.id == id).cloned())
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

    /// The language line carries the tag the caller passes; no instruction
    /// names a language.
    #[test]
    fn lower_prompt_names_its_language() {
        let (view, inv, entities, item) = dao(2);
        for tag in ["en-US", "ja-JP", "pt-BR"] {
            let prompt = Prompt::lower(&view, &item, tag, &inv, &entities, &weksa::Ontology::empty()).unwrap();
            let want = format!("{LOWER}\n\nlanguage: {tag}\n\npacket: ");
            assert!(prompt.text().starts_with(&want), "{tag}");
        }
        for brief in [LOWER, weksa::context::MINT_INSTRUCTIONS] {
            assert!(!brief.contains("English") && !brief.contains("en-US"));
        }
    }

    #[test]
    fn lower_prompt_text_is_pinned() {
        let (view, inv, entities, item) = dao(2);
        let prompt = Prompt::lower(&view, &item, "en-US", &inv, &entities, &weksa::Ontology::empty()).unwrap();
        let (head, listing) = prompt.text().split_once("inventory rev:3\n").unwrap();
        let expected = format!(
            "{LOWER}\n\nlanguage: en-US\n\npacket: {}\n\nentities: {{}}\n\nontology: none\n\n",
            concat!(
                r#"{"context":{"setting":null,"medium":null,"participants":[],"audience":[]},"#,
                r#""discourse":{"speech_act":"speech_act:1","addressee":null,"addressee_consent":null,"features":[]},"#,
                r#""referents":[{"id":"n1","kind":"referent_kind:11","concept":"concept:20","entity":null,"features":[]},"#,
                r#"{"id":"n2","kind":"referent_kind:11","concept":"concept:21","entity":null,"features":[]}],"#,
                r#""predications":[{"id":"n3","concept":"concept:27","frame":"frame:6","roles":[{"role":"role:3","filler":"n1"},{"role":"role:6","filler":"n2"}],"features":[]}],"#,
                r#""relations":[],"extensions":[],"#,
                r#""stance":{"features":[{"feature":"feature:18","value":{"Grade":800}},{"feature":"feature:16","value":{"Grade":300}}]}}"#
            )
        );
        assert_eq!(head, expected);
        assert!(listing.starts_with("concept:1: endanger\n"));
        assert!(listing.contains("\nfeature:18: "));
        // The brief says what the spec says it must.
        for needed in [
            "the language on the language line",
            "all of its meaning and nothing it does not carry",
            "a grade is permille, 0 to 1000",
            "felt affect and restraint",
            "shapes how the utterance is said, not what it says",
            "forms on the entities line",
            r#"{"text": "..."}"#,
        ] {
            assert!(LOWER.contains(needed), "{needed}");
        }
        // The one call it makes is its item; the key is in no text.
        assert_eq!(prompt.item_keys(), [item.as_str()]);
        assert!(!prompt.text().contains(item.as_str()));
        // The view's speaker and group are private: the item's speaker entity
        // id and its group are in no text.
        let held = CorpusStore::open(&data("daodejing/corpus.cc"))
            .unwrap()
            .get(item.as_str())
            .unwrap()
            .unwrap();
        assert!(!held.group.is_empty());
        for private in [held.speaker.as_str(), held.group.as_str()] {
            assert!(!prompt.text().contains(private), "{private}");
        }
    }

    /// The mint cut added no line to what the lowerer sees: a prompt with a
    /// selected ontology entry has the digest it had at af0a56d.
    #[test]
    fn the_lowering_prompt_of_a_fixed_packet_is_unchanged_by_the_ontology_mint() {
        use weksa::encode::OntologyDraft;
        use weksa::speaker::Annotator;
        let (view, inv, entities, item) = dao(2);
        let store = InventoryStore::open(&data("daodejing/inventory.cc")).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut onto = weksa::OntologyStore::open(&dir.path().join("o.cc")).unwrap();
        let annotator = Annotator::Model {
            family: weksa::corpus::ModelFamily::Anthropic,
            generator: "golden".into(),
        };
        let draft = r#"{"entries":[{"layer":"Grammar","choice":"golden choice","examples":["golden example"]}]}"#;
        let entries = OntologyDraft::parse(draft).unwrap().into_entries("en-US", 3, 1, &annotator);
        onto.admit(entries, &store).unwrap();
        let prompt =
            Prompt::lower(&view, &item, "en-US", &inv, &entities, &onto.ontology().unwrap()).unwrap();
        assert!(prompt.text().contains("golden choice"));
        assert_eq!(prompt.digest(), "4a40eb57282b059d");
    }

    #[test]
    fn lower_prompt_moves_with_the_stance_alone() {
        use weksa::packet::{FeatValue, FeatureValue, Grade, Stance};
        let (_, inv, entities, item) = dao(2);
        let store = InventoryStore::open(&data("daodejing/inventory.cc")).unwrap();
        let own = packets().remove(2).stance;
        let grade = |feature: &str, v: u16| FeatureValue {
            feature: feature.parse().unwrap(),
            value: FeatValue::Grade(Grade::new(v).unwrap()),
        };
        let angry = Stance {
            features: vec![grade("feature:9", 800), grade("feature:18", 900)],
        };
        let prompt_of = |stance: Stance| {
            let mut packet = packets().remove(2);
            packet.stance = stance.clone();
            let view = admitted(packet, &store, "daodejing/corpus.cc", &entities);
            let prompt = Prompt::lower(&view, &item, "en-US", &inv, &entities, &weksa::Ontology::empty()).unwrap();
            (stance, prompt)
        };
        let three = [prompt_of(own), prompt_of(angry), prompt_of(Stance::default())];

        // Three stances, three prompts.
        let digests: std::collections::BTreeSet<String> =
            three.iter().map(|(_, p)| p.digest()).collect();
        assert_eq!(digests.len(), 3);
        // Each packet line ends with exactly that stance's JSON, and the prompt
        // with the packet line taken out is the same text for all three.
        let without_packet_line = |text: &str| -> String {
            text.lines().filter(|l| !l.starts_with("packet: ")).collect::<Vec<_>>().join("\n")
        };
        let bare = without_packet_line(three[0].1.text());
        for (stance, prompt) in &three {
            let line = prompt.text().lines().find(|l| l.starts_with("packet: ")).unwrap();
            let json = serde_json::to_string(stance).unwrap();
            assert!(line.ends_with(&format!(r#","stance":{json}}}"#)), "{line}");
            assert_eq!(without_packet_line(prompt.text()), bare);
        }
        // The stance's feature ids are glossed in the one inventory listing.
        let text = three[1].1.text();
        assert!(text.contains("\nfeature:9: anger [grade; slots Stance]\n"));
        assert!(text.contains("\nfeature:18: restraint [grade; slots Stance]\n"));
    }

    #[test]
    fn an_entity_referent_prints_its_en_us_forms_citation_first() {
        let cast = InventoryStore::open(&data("interlingua/inventory.cc")).unwrap();
        let cast_entities = EntityStore::open(&data("entity/entity.cc")).unwrap();
        let view = admitted(proof_packet(), &cast, "corpus/corpus.cc", &cast_entities);
        let item = CorpusItemId::parse("item:nibu:1").unwrap();
        let held = Held(vec![nibu(&[
            ("sa-Latn", &["Nibuwa"]),
            ("en-US", &["Nibu", "Nibu's"]),
        ])]);
        let prompt = Prompt::lower(&view, &item, "en-US", &cast.inventory().unwrap(), &held, &weksa::Ontology::empty()).unwrap();
        assert!(
            prompt
                .text()
                .contains("\n\nentities: {\"entity:nibu\":[\"Nibu\",\"Nibu's\"]}\n\n"),
            "{}",
            prompt.text()
        );
        assert!(!prompt.text().contains("Nibuwa"));
    }

    #[test]
    fn lower_prompt_refuses_an_entity_without_an_english_form() {
        let cast = InventoryStore::open(&data("interlingua/inventory.cc")).unwrap();
        let cast_entities = EntityStore::open(&data("entity/entity.cc")).unwrap();
        let view = admitted(proof_packet(), &cast, "corpus/corpus.cc", &cast_entities);
        let inv = cast.inventory().unwrap();
        let item = CorpusItemId::parse("item:nibu:1").unwrap();
        let id = EntityId::parse("entity:nibu").unwrap();
        for (held, want) in [
            (
                Held(vec![nibu(&[("sa-Latn", &["Nibuwa"])])]),
                LowerError::NoForm { entity: id.clone() },
            ),
            (
                Held(vec![nibu(&[("en-US", &[])])]),
                LowerError::NoForm { entity: id.clone() },
            ),
            (Held(vec![]), LowerError::UnknownEntity { entity: id }),
        ] {
            assert_eq!(Prompt::lower(&view, &item, "en-US", &inv, &held, &weksa::Ontology::empty()).err(), Some(want));
        }
    }

    fn feature(inv: &Inventory, gloss: &str) -> weksa::InvId {
        inv.entries().find(|e| e.gloss == gloss).unwrap().id
    }

    fn entry(language: &str, n: u32, when: Vec<Condition>, choice: &str) -> OntologyEntry {
        OntologyEntry {
            language: language.into(),
            n,
            inventory_rev: 1,
            layer: Layer::Discourse,
            when,
            choice: choice.into(),
            examples: vec![],
            annotator: Annotator::Operator,
        }
    }

    /// The ontology the store holds after admitting `entries` as one batch.
    fn ontology_of(cast: &InventoryStore, entries: Vec<OntologyEntry>) -> weksa::Ontology {
        let dir = tempfile::tempdir().unwrap();
        let mut store = OntologyStore::open(&dir.path().join("o.cc")).unwrap();
        store.admit(entries, cast).unwrap();
        store.ontology().unwrap()
    }

    fn at_least(inv: &Inventory, gloss: &str, g: u16) -> Condition {
        Condition {
            slot: weksa::Slot::Stance,
            feature: feature(inv, gloss),
            test: Test::AtLeast(Grade::new(g).unwrap()),
        }
    }

    /// The proof packet's view with the given stance restraint and discourse irony.
    fn view_of(
        restraint: Option<u16>,
        irony: Option<bool>,
        cast: &InventoryStore,
        entities: &EntityStore,
    ) -> LoweringInput {
        use weksa::packet::{FeatValue, FeatureValue};
        let inv = cast.inventory().unwrap();
        let mut packet = proof_packet();
        packet.stance.features = restraint
            .map(|v| FeatureValue {
                feature: feature(&inv, "restraint"),
                value: FeatValue::Grade(Grade::new(v).unwrap()),
            })
            .into_iter()
            .collect();
        packet.discourse.features = irony
            .map(|v| FeatureValue {
                feature: feature(&inv, "irony"),
                value: FeatValue::Flag(v),
            })
            .into_iter()
            .collect();
        admitted(packet, cast, "corpus/corpus.cc", entities)
    }

    #[test]
    fn select_reads_stance_and_discourse_of_its_language() {
        use weksa::packet::FeatValue;
        let cast = InventoryStore::open(&data("interlingua/inventory.cc")).unwrap();
        let entities = EntityStore::open(&data("entity/entity.cc")).unwrap();
        let inv = cast.inventory().unwrap();
        let ontology = ontology_of(&cast, vec![
            entry("ja-JP", 1, vec![at_least(&inv, "restraint", 600)], "high"),
            entry(
                "ja-JP",
                2,
                vec![Condition {
                    slot: weksa::Slot::Stance,
                    feature: feature(&inv, "restraint"),
                    test: Test::AtMost(Grade::new(300).unwrap()),
                }],
                "low",
            ),
            entry(
                "ja-JP",
                3,
                vec![Condition {
                    slot: weksa::Slot::Discourse,
                    feature: feature(&inv, "irony"),
                    test: Test::Is(FeatValue::Flag(true)),
                }],
                "ironic",
            ),
            entry("ja-JP", 4, vec![], "always"),
            entry("en-US", 1, vec![], "other language"),
        ]);
        let keys = |view: &LoweringInput, language: &str| -> Vec<String> {
            select(&ontology, language, view).iter().map(|e| e.key()).collect()
        };
        let both = view_of(Some(700), Some(true), &cast, &entities);
        assert_eq!(keys(&both, "ja-JP"), ["onto:ja-JP:1", "onto:ja-JP:3", "onto:ja-JP:4"]);
        assert_eq!(keys(&both, "en-US"), ["onto:en-US:1"]);
        let low = view_of(Some(200), Some(false), &cast, &entities);
        assert_eq!(keys(&low, "ja-JP"), ["onto:ja-JP:2", "onto:ja-JP:4"]);
        // A grade equal to the bound meets AtLeast and AtMost alike.
        let edge = view_of(Some(600), None, &cast, &entities);
        assert_eq!(keys(&edge, "ja-JP"), ["onto:ja-JP:1", "onto:ja-JP:4"]);
        let edge = view_of(Some(300), None, &cast, &entities);
        assert_eq!(keys(&edge, "ja-JP"), ["onto:ja-JP:2", "onto:ja-JP:4"]);
        // An absent feature meets no condition, however lax.
        let bare = view_of(None, None, &cast, &entities);
        assert_eq!(keys(&bare, "ja-JP"), ["onto:ja-JP:4"]);
    }

    #[test]
    fn lowering_prompt_prints_only_selected_entries() {
        let cast = InventoryStore::open(&data("interlingua/inventory.cc")).unwrap();
        let cast_entities = EntityStore::open(&data("entity/entity.cc")).unwrap();
        let inv = cast.inventory().unwrap();
        let mut hit = entry("ja-JP", 1, vec![at_least(&inv, "restraint", 600)], "HITCHOICE");
        hit.examples = vec!["HITEXAMPLE".into()];
        let ontology = ontology_of(&cast, vec![
            hit,
            entry("ja-JP", 2, vec![at_least(&inv, "restraint", 900)], "MISSCHOICE"),
            entry("en-US", 1, vec![], "ENGLISHCHOICE"),
        ]);
        let held = Held(vec![nibu(&[("ja-JP", &["ニブ"]), ("en-US", &["Nibu"])])]);
        let item = CorpusItemId::parse("item:nibu:1").unwrap();
        let view = view_of(Some(700), None, &cast, &cast_entities);
        let lowered = |view: &LoweringInput, o: &weksa::Ontology| {
            Prompt::lower(view, &item, "ja-JP", &inv, &held, o).unwrap()
        };
        let prompt = lowered(&view, &ontology);
        let text = prompt.text();
        assert!(text.contains("\n\nontology:\nonto:ja-JP:1 [discourse] HITCHOICE | e.g. HITEXAMPLE\n\n"), "{text}");
        for absent in ["MISSCHOICE", "ENGLISHCHOICE", "onto:ja-JP:2", "onto:en-US:1"] {
            assert!(!text.contains(absent), "{absent}");
        }
        // The same view with no entries: the section reads none.
        let none = lowered(&view, &weksa::Ontology::empty());
        assert!(none.text().contains("\n\nontology: none\n\n"));
        // Entries move the prompt and nothing else of the view does.
        assert_ne!(prompt.digest(), none.digest());
        // The group is still private; the speaker's entity appears only because
        // the packet cites it on a referent.
        let held_item = CorpusStore::open(&data("corpus/corpus.cc"))
            .unwrap()
            .get(item.as_str())
            .unwrap()
            .unwrap();
        assert!(!text.contains(held_item.group.as_str()));
    }
}

/// First real rounds: five of six encoders wrote bare integers (`"concept": 7`)
/// where the inventory lists `concept:7`, and the draft reader refused them.
/// The prompt says the form outright, and the inventory lines it points to
/// are spelled in that form.
#[test]
fn the_encode_prompt_states_the_id_form_the_inventory_lines_use() {
    assert!(ENCODE.contains("family prefix included: concept:7, never 7"));
    assert!(ENCODE.contains("A referent's concept is never null, even when it carries an entity: give the nearest concept id."));
    assert!(ENCODE.contains("Participants and audience list the node ids of referents, never an entity id: an entity speaker is a referent whose entity field holds it, and participants lists that referent's node id."));
    let w = world();
    let run = run_over(w.dir.path());
    let inv = run.inventory().inventory().unwrap();
    let t = run.train().unwrap();
    let offer = run.offer("item:nibu:1").unwrap().unwrap();
    let text = Prompt::encode(&offer, &t, &inv, 4).text().to_string();
    let listed: Vec<&str> = text
        .split_once("inventory rev:4\n")
        .unwrap()
        .1
        .split("\n\n")
        .next()
        .unwrap()
        .lines()
        .collect();
    assert!(!listed.is_empty(), "the prompt lists entries");
    for line in listed {
        let id = line.split(": ").next().unwrap();
        assert!(id.contains(':'), "an inventory line spells its id in full: {line}");
    }
}

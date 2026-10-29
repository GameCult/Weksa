mod common;
use common::*;
use weksa::corpus::{CorpusItem, NameSpan, Turn};
use weksa::packet::{Packet, Referent};
use weksa::{
    AddOutcome, CorpusItemId, CorpusStore, Entity, EntityId, EntityOrigin, EntityStore, PacketId,
    PacketStore, Refusal, SCHEMA_VERSION, StoreError, StoreRefusal, validate,
};

fn refusal(r: Result<impl std::fmt::Debug, StoreError>) -> StoreRefusal {
    match r {
        Err(StoreError::Refused(r)) => r,
        other => panic!("expected a store refusal, got {other:?}"),
    }
}

fn malformed(r: Result<impl std::fmt::Debug, StoreError>) -> &'static str {
    match refusal(r) {
        StoreRefusal::Malformed { why, .. } => why,
        other => panic!("expected Malformed, got {other:?}"),
    }
}

fn span(slug: &str, start: u32, end: u32) -> NameSpan {
    NameSpan {
        entity: eid(slug),
        start,
        end,
    }
}

/// An item in `language` whose text and spans the test chooses.
fn named(n: u32, language: &str, text: &str, names: Vec<NameSpan>) -> CorpusItem {
    CorpusItem {
        language: language.into(),
        names,
        ..item_in(&format!("item:t:{n}"), text, "g", cast())
    }
}

#[test]
fn entity_id_form() {
    for ok in ["entity:brokkr", "entity:a-1", "entity:7", "entity:a-b-c"] {
        assert_eq!(EntityId::parse(ok).unwrap().as_str(), ok);
    }
    let long_ok = format!("entity:{}", "a".repeat(48));
    assert!(EntityId::parse(&long_ok).is_ok());
    let too_long = format!("entity:{}", "a".repeat(49));
    for bad in [
        "Entity:x",
        "entity:-a",
        "entity:a-",
        "entity:",
        "entity:a_b",
        "entity:A",
        "entity:a b",
        "entity:a:b",
        "brokkr",
        "entity:\u{5c71}",
        &too_long,
    ] {
        assert!(EntityId::parse(bad).is_err(), "{bad:?}");
        assert!(
            serde_json::from_value::<EntityId>(bad.into()).is_err(),
            "{bad:?} must not read back from a store"
        );
    }
    // A referent reads its entity through the same parse.
    let mut referent = serde_json::to_value(good().referents[0].clone()).unwrap();
    assert!(serde_json::from_value::<Referent>(referent.clone()).is_ok());
    referent["entity"] = "Nibu".into();
    assert!(serde_json::from_value::<Referent>(referent).is_err());
}

#[test]
fn entity_forms_only_grow() {
    let d = tempfile::tempdir().unwrap();
    let mut store = EntityStore::open(&d.path().join("e.cc")).unwrap();
    let kay = ent("kay", "en-US", &["Kay"]);
    assert_eq!(store.put(kay.clone()).unwrap(), AddOutcome::Added);
    assert_eq!(store.put(kay.clone()).unwrap(), AddOutcome::Unchanged);

    // A form appends; a language appends.
    let mut grown = kay.clone();
    grown.forms[0].forms.push("Kay's".into());
    assert_eq!(store.put(grown.clone()).unwrap(), AddOutcome::Added);
    let mut more = grown.clone();
    more.forms.push(weksa::LanguageForms {
        language: "tr".into(),
        forms: vec!["Kay".into()],
    });
    assert_eq!(store.put(more.clone()).unwrap(), AddOutcome::Added);
    // The store read back from disk holds the growth.
    let mut store = EntityStore::open(&d.path().join("e.cc")).unwrap();
    assert_eq!(store.get(&eid("kay")).unwrap(), Some(more.clone()));

    // Dropping, reordering, editing and re-originating are refused, each leaving
    // the stored entity as it was.
    let not_append = StoreRefusal::EntityNotAppendOnly { id: eid("kay") };
    let edits: [fn(&mut Entity); 6] = [
        |e| e.forms[0].forms.reverse(),
        |e| e.forms[0].forms[0] = "Kai".into(),
        |e| {
            e.forms.pop();
        },
        |e| e.forms.reverse(),
        |e| e.forms[1].language = "fi".into(),
        |e| {
            e.origin = EntityOrigin::Live {
                persona: "kay".into(),
            }
        },
    ];
    assert_eq!(
        refusal(store.put(ent("kay", "en-US", &["Kay"]))),
        not_append
    );
    for edit in edits {
        let mut v = more.clone();
        edit(&mut v);
        assert_eq!(refusal(store.put(v.clone())), not_append, "{v:?}");
    }
    assert_eq!(store.get(&eid("kay")).unwrap(), Some(more));

    // Shape: no forms, a language twice, an empty language, an empty language
    // entry, an empty, padded, control-bearing, oversize or repeated form.
    let shape = |forms: Vec<weksa::LanguageForms>| weksa::Entity {
        forms,
        ..ent("shape", "en-US", &["X"])
    };
    let lf = |language: &str, forms: &[&str]| weksa::LanguageForms {
        language: language.into(),
        forms: forms.iter().map(|f| f.to_string()).collect(),
    };
    for bad in [
        vec![],
        vec![lf("en-US", &["A"]), lf("en-US", &["B"])],
        vec![lf("en-US", &[])],
        vec![lf("en-US", &[""])],
        vec![lf("en-US", &[" A"])],
        vec![lf("en-US", &["A\u{a0}"])],
        vec![lf("en-US", &["A\nB"])],
        vec![lf("en-US", &["A", "A"])],
    ] {
        malformed(store.put(shape(bad.clone())));
        assert!(
            store.get(&eid("shape")).unwrap().is_none(),
            "{bad:?} was stored"
        );
    }
    // An empty language is not a tag.
    let e = shape(vec![lf("", &["A"])]);
    assert!(matches!(
        refusal(store.put(e)),
        StoreRefusal::EntityBounds {
            bound: weksa::store::EntityBound::LanguageTag,
            ..
        }
    ));
    // 64 bytes is the edge; a form may repeat across languages.
    let edge = "a".repeat(64);
    store
        .put(shape(vec![lf("en-US", &[&edge]), lf("tr", &[&edge])]))
        .unwrap();
}

#[test]
fn corpus_spans_checked() {
    let d = tempfile::tempdir().unwrap();
    let mut ents = entities_in(d.path());
    ents.put(ent("kay", "en-US", &["Kay"])).unwrap();
    ents.put(ent("ab", "en-US", &["abab", "bab"])).unwrap();
    let mut corpus = CorpusStore::open(&d.path().join("c.cc")).unwrap();

    assert_eq!(
        corpus
            .add(
                named(1, "en-US", "Kay left.", vec![span("kay", 0, 3)]),
                &ents
            )
            .unwrap(),
        AddOutcome::Added
    );

    let range = |n: u32, start: u32, end: u32| StoreRefusal::NameSpanRange {
        key: format!("item:t:{n}"),
        start,
        end,
    };
    let not_a_form = |n: u32, slug: &str, start: u32, end: u32| StoreRefusal::NameSpanNotAForm {
        key: format!("item:t:{n}"),
        entity: eid(slug),
        start,
        end,
    };
    let mut refused = |item: CorpusItem| refusal(corpus.add(item, &ents));
    // Empty, backwards, past the end: a range refusal.
    for (start, end) in [(3, 3), (4, 3), (0, 99)] {
        let item = named(2, "en-US", "Kay left.", vec![span("kay", start, end)]);
        assert_eq!(refused(item), range(2, start, end));
    }
    // Inside a multibyte character: start and end off the boundary.
    let text = "\u{e9}Kay";
    assert_eq!(text.len(), 5);
    let item = named(2, "en-US", text, vec![span("kay", 1, 4)]);
    assert_eq!(refused(item), range(2, 1, 4));
    let item = named(2, "en-US", "Kay\u{e9}", vec![span("kay", 0, 4)]);
    assert_eq!(refused(item), range(2, 0, 4));
    // An unknown entity.
    let item = named(2, "en-US", "Kay left.", vec![span("ghost", 0, 3)]);
    assert_eq!(
        refused(item),
        StoreRefusal::UnknownEntity {
            entity: eid("ghost")
        }
    );
    // Not byte-equal: other text, other case, a missing language.
    let item = named(2, "en-US", "Kai left.", vec![span("kay", 0, 3)]);
    assert_eq!(refused(item), not_a_form(2, "kay", 0, 3));
    let item = named(2, "en-US", "kay left.", vec![span("kay", 0, 3)]);
    assert_eq!(refused(item), not_a_form(2, "kay", 0, 3));
    let item = named(2, "tr", "Kay left.", vec![span("kay", 0, 3)]);
    assert_eq!(refused(item), not_a_form(2, "kay", 0, 3));
    // Overlap: both spans are forms, they share bytes.
    let item = named(2, "en-US", "abab", vec![span("ab", 0, 4), span("ab", 1, 4)]);
    assert_eq!(
        refused(item),
        StoreRefusal::NameSpanOverlap {
            key: "item:t:2".into()
        }
    );
    // Adjacent spans do not overlap, whatever order they are listed in.
    let adjacent = named(
        90,
        "en-US",
        "KayKay",
        vec![span("kay", 3, 6), span("kay", 0, 3)],
    );
    assert_eq!(corpus.add(adjacent, &ents).unwrap(), AddOutcome::Added);

    // Speakers: unknown, and unknown in a prior turn.
    let ghost = EntityId::parse("entity:ghost").unwrap();
    let unknown = StoreRefusal::UnknownEntity {
        entity: ghost.clone(),
    };
    let mut item = named(91, "en-US", "Hello there.", vec![]);
    item.speaker = ghost.clone();
    assert_eq!(refusal(corpus.add(item, &ents)), unknown);
    let mut item = named(92, "en-US", "Hello again.", vec![]);
    item.prior_turns = vec![
        Turn {
            speaker: eid("nibu"),
            text: "hi".into(),
        },
        Turn {
            speaker: ghost,
            text: "hi".into(),
        },
    ];
    assert_eq!(refusal(corpus.add(item, &ents)), unknown);
    let mut item = named(93, "en-US", "Hello at last.", vec![]);
    item.prior_turns = vec![Turn {
        speaker: eid("nibu"),
        text: "hi".into(),
    }];
    assert_eq!(corpus.add(item, &ents).unwrap(), AddOutcome::Added);
    // Nothing refused was stored.
    let keys: Vec<String> = corpus.all().unwrap().into_iter().map(|i| i.id).collect();
    assert_eq!(keys, ["item:t:1", "item:t:90", "item:t:93"]);
}

#[test]
fn corpus_spans_every_script() {
    let d = tempfile::tempdir().unwrap();
    let mut ents = entities_in(d.path());
    ents.put(ent(
        "minsu",
        "ko",
        &["\u{BBFC}\u{C218}", "\u{BBFC}\u{C218}\u{AC00}"],
    ))
    .unwrap();
    ents.put(ent("brokkr-tr", "tr", &["Brokkr", "Brokkr'\u{131}n"]))
        .unwrap();
    ents.put(ent(
        "somchai",
        "th",
        &["\u{E2A}\u{E21}\u{E0A}\u{E32}\u{E22}"],
    ))
    .unwrap();
    let mut corpus = CorpusStore::open(&d.path().join("c.cc")).unwrap();

    // The particle-bound Korean form: byte equality with the stored form.
    let ko = "\u{BBFC}\u{C218}\u{AC00} \u{C654}\u{B2E4}";
    let with_particle = "\u{BBFC}\u{C218}\u{AC00}".len();
    assert_eq!(with_particle, 9);
    let item = named(1, "ko", ko, vec![span("minsu", 0, with_particle as u32)]);
    assert_eq!(corpus.add(item, &ents).unwrap(), AddOutcome::Added);
    // The bare stem is a stored form too; a span cutting a syllable is not.
    let item = named(
        2,
        "ko",
        "\u{BBFC}\u{C218} \u{C654}\u{B2E4}",
        vec![span("minsu", 0, 6)],
    );
    assert_eq!(corpus.add(item, &ents).unwrap(), AddOutcome::Added);
    let item = named(3, "ko", ko, vec![span("minsu", 0, 7)]);
    assert!(matches!(
        refusal(corpus.add(item, &ents)),
        StoreRefusal::NameSpanRange { .. }
    ));
    // Turkish: the apostrophe suffix.
    let tr = "Brokkr'\u{131}n geldi.";
    let len = "Brokkr'\u{131}n".len() as u32;
    let item = named(4, "tr", tr, vec![span("brokkr-tr", 0, len)]);
    assert_eq!(corpus.add(item, &ents).unwrap(), AddOutcome::Added);
    // Thai: no spaces, no segmentation, a span is a byte range.
    let th = "\u{E2A}\u{E21}\u{E0A}\u{E32}\u{E22}\u{E21}\u{E32}\u{E41}\u{E25}\u{E49}\u{E27}";
    let len = "\u{E2A}\u{E21}\u{E0A}\u{E32}\u{E22}".len() as u32;
    let item = named(5, "th", th, vec![span("somchai", 0, len)]);
    assert_eq!(corpus.add(item, &ents).unwrap(), AddOutcome::Added);
    // The same bytes under another language tag are refused.
    let item = named(6, "ko", th, vec![span("somchai", 0, len)]);
    assert!(matches!(
        refusal(corpus.add(item, &ents)),
        StoreRefusal::NameSpanNotAForm { .. }
    ));
}

#[test]
fn packet_entities_resolve() {
    let mut w = world();
    w.entities.put(ent("kay", "en-US", &["Kay"])).unwrap();
    let mut talk = named(2, "en-US", "Kay and Hero talk.", vec![span("kay", 0, 3)]);
    talk.id = "item:nibu:2".into();
    talk.prior_turns = vec![Turn {
        speaker: eid("hero"),
        text: "hi".into(),
    }];
    w.corpus.add(talk, &w.entities).unwrap();

    let mut packets = PacketStore::open(&w.dir.path().join("p.cc")).unwrap();
    let packet = |n: u8, item: &str, entity: &str| {
        let mut p = good();
        p.id = PacketId::parse(&format!("packet:{n:016x}")).unwrap();
        p.provenance.corpus_item = CorpusItemId::parse(item).unwrap();
        p.referents[0].entity = Some(eid(entity));
        p
    };
    let mut admit = |p: Packet| packets.admit(p, &w.inventory, &w.corpus, &w.entities);

    // An entity the store lacks.
    let unknown = StoreRefusal::UnknownEntity {
        entity: eid("ghost"),
    };
    assert_eq!(refusal(admit(packet(1, "item:nibu:1", "ghost"))), unknown);
    assert_eq!(refusal(admit(packet(2, "item:nibu:2", "ghost"))), unknown);
    // A stored entity that is not on the item.
    let off_item = |item: &str, entity: &str| StoreRefusal::EntityNotOnItem {
        entity: eid(entity),
        item: item.into(),
    };
    assert_eq!(
        refusal(admit(packet(3, "item:nibu:2", "brokkr"))),
        off_item("item:nibu:2", "brokkr")
    );
    // nibu:1 names nibu (and is spoken by nibu); kay is only on nibu:2.
    assert_eq!(
        refusal(admit(packet(4, "item:nibu:1", "kay"))),
        off_item("item:nibu:1", "kay")
    );
    // On the item: a name entity, the speaker, a prior-turn speaker.
    admit(packet(5, "item:nibu:2", "kay")).unwrap();
    admit(packet(6, "item:nibu:2", "nibu")).unwrap();
    admit(packet(7, "item:nibu:2", "hero")).unwrap();
    // No packet escapes the item check: a stored entity that no item names is
    // refused on every item, and a packet cannot omit its item.
    assert_eq!(
        refusal(admit(packet(8, "item:nibu:1", "brokkr"))),
        off_item("item:nibu:1", "brokkr")
    );
    // A referent with no entity needs none; only a packet that cites checks.
    let mut anonymous = packet(9, "item:nibu:2", "kay");
    anonymous.referents[0].entity = None;
    admit(anonymous).unwrap();
    // Refused packets were not stored.
    let stored: Vec<String> = PacketStore::open(&w.dir.path().join("p.cc"))
        .unwrap()
        .all()
        .unwrap()
        .iter()
        .map(|p| p.id.to_string())
        .collect();
    assert_eq!(stored.len(), 4, "{stored:?}");
}

#[test]
fn packet_schema_v2() {
    assert_eq!(SCHEMA_VERSION, 3);
    let mut p = good();
    p.schema_version = 2;
    let Err(refused) = validate(&p, &inventory()) else {
        panic!("a v1 packet must not validate")
    };
    assert_eq!(refused, vec![Refusal::V1SchemaVersion { found: 2 }]);

    let published: std::collections::BTreeMap<_, _> =
        weksa::schema::published().into_iter().collect();
    let packet: serde_json::Value =
        serde_json::from_str(&published["weksa.interlingua_packet.schema.json"]).unwrap();
    let referent = &packet["$defs"]["Referent"]["properties"];
    assert!(referent.get("entity").is_some(), "{referent}");
    assert!(referent.get("name").is_none(), "{referent}");
    let defs = packet["$defs"].as_object().unwrap();
    assert!(!defs.keys().any(|k| k.ends_with("Name")), "{defs:?}");
    assert_eq!(
        packet["$defs"]["EntityId"]["pattern"],
        "^entity:[a-z0-9]([a-z0-9-]{0,46}[a-z0-9])?$"
    );
    let item: serde_json::Value =
        serde_json::from_str(&published["weksa.corpus_item.schema.json"]).unwrap();
    assert!(item["properties"].get("names").is_some());
    assert!(published.contains_key("weksa.entity.schema.json"));
}

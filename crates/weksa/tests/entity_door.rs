//! Behavioural pins from Soul's entity-store s1 probes: the entity door does not
//! trust the writer (ruling entity-door-bounds).
mod common;
use common::*;
use unicode_normalization::UnicodeNormalization;
use weksa::corpus::{CorpusItem, NameSpan};
use weksa::entity::{Entity, EntityOrigin, LanguageForms};
use weksa::store::EntityBound;
use weksa::{EntityStore, StoreError, StoreRefusal};

fn refusal(r: Result<impl std::fmt::Debug, StoreError>) -> StoreRefusal {
    match r {
        Err(StoreError::Refused(r)) => r,
        other => panic!("expected a store refusal, got {other:?}"),
    }
}

fn item(
    n: impl std::fmt::Display,
    lang: &str,
    text: &str,
    names: Vec<NameSpan>,
    group: &str,
) -> CorpusItem {
    CorpusItem {
        language: lang.into(),
        names,
        ..item_in(&format!("item:probe:{n}"), text, group, cast())
    }
}

fn span(slug: &str, start: usize, end: usize) -> NameSpan {
    NameSpan {
        entity: eid(slug),
        start: start as u32,
        end: end as u32,
    }
}

fn bounds(entity: &Entity, bound: EntityBound) -> StoreRefusal {
    StoreRefusal::EntityBounds {
        id: entity.id.clone(),
        bound,
    }
}

#[test]
fn span_never_splits_a_grapheme_cluster() {
    let mut w = world();
    let g = group_in_bucket(10);
    let splits = |n: u32, start: usize, end: usize| StoreRefusal::NameSpanSplitsCluster {
        key: format!("item:probe:{n}"),
        start: start as u32,
        end: end as u32,
    };

    // Devanagari: the word is a base, a spacing vowel sign and a base; the form
    // "र" ends inside the cluster of the first two.
    w.entities.put(ent("ra", "hi", &["र"])).unwrap();
    let text = "राम आया";
    assert_eq!(
        refusal(
            w.corpus
                .add(item(1, "hi", text, vec![span("ra", 0, 3)], &g), &w.entities)
        ),
        splits(1, 0, 3)
    );
    // The whole words are legal names, the second starting after a space.
    w.entities.put(ent("ram", "hi", &["राम", "आया"])).unwrap();
    let both = vec![span("ram", 0, 9), span("ram", 10, text.len())];
    w.corpus
        .add(item(2, "hi", text, both, &g), &w.entities)
        .unwrap();

    // A start inside a cluster is refused too.
    w.entities.put(ent("aa", "hi", &["ा"])).unwrap();
    assert_eq!(
        refusal(w.corpus.add(
            item(3, "hi", "राम x", vec![span("aa", 3, 6)], &g),
            &w.entities
        )),
        splits(3, 3, 6)
    );

    // Latin base + combining mark with no precomposed form, so NFC keeps two
    // code points: the form "q" leaves the accent outside.
    let q = "q\u{301} arrive.";
    w.entities.put(ent("q", "fr", &["q"])).unwrap();
    assert_eq!(
        refusal(
            w.corpus
                .add(item(4, "fr", q, vec![span("q", 0, 1)], &g), &w.entities)
        ),
        splits(4, 0, 1)
    );
    w.entities.put(ent("qa", "fr", &["q\u{301}"])).unwrap();
    w.corpus
        .add(item(5, "fr", q, vec![span("qa", 0, 3)], &g), &w.entities)
        .unwrap();
}

#[test]
fn text_and_forms_are_stored_nfc() {
    let mut w = world();
    let g = group_in_bucket(10);
    let not_nfc = |key: &str| StoreRefusal::NotNfc { key: key.into() };
    // Decomposed Hangul (conjoining jamo) text is refused, not segmented.
    let nfd = "\u{1106}\u{1175}\u{11AB}\u{1109}\u{116E} \u{110B}\u{116A}\u{11BB}\u{1103}\u{1161}";
    assert!(!unicode_normalization::is_nfc(nfd));
    assert_eq!(
        refusal(w.corpus.add(item(1, "ko", nfd, vec![], &g), &w.entities)),
        not_nfc("item:probe:1")
    );
    // The same words composed are admitted, and a span on a syllable pair is exact.
    let nfc: String = nfd.nfc().collect();
    let name: String = "\u{1106}\u{1175}\u{11AB}\u{1109}\u{116E}".nfc().collect();
    w.entities.put(ent("minsu", "ko", &[&name])).unwrap();
    w.corpus
        .add(
            item(2, "ko", &nfc, vec![span("minsu", 0, name.len())], &g),
            &w.entities,
        )
        .unwrap();
    // Latin NFD text, NFD forms and NFD prior turns are refused.
    assert_eq!(
        refusal(w.corpus.add(
            item(3, "fr", "Rene\u{301} arrive.", vec![], &g),
            &w.entities
        )),
        not_nfc("item:probe:3")
    );
    assert_eq!(
        refusal(w.entities.put(ent("rene", "fr", &["Rene\u{301}"]))),
        not_nfc("entity:rene")
    );
    let mut turns = item(4, "fr", "Salut.", vec![], &g);
    turns.prior_turns = vec![weksa::Turn {
        speaker: eid("nibu"),
        text: "Rene\u{301}?".into(),
    }];
    assert_eq!(
        refusal(w.corpus.add(turns, &w.entities)),
        not_nfc("item:probe:4")
    );
    // NFC text and an NFC form of the same name agree byte for byte.
    w.entities.put(ent("rene", "fr", &["Ren\u{e9}"])).unwrap();
    w.corpus
        .add(
            item(5, "fr", "Ren\u{e9} part.", vec![span("rene", 0, 5)], &g),
            &w.entities,
        )
        .unwrap();
}

#[test]
fn entity_capacity_is_bounded() {
    let mut w = world();
    let lf = |language: &str, forms: Vec<String>| LanguageForms {
        language: language.into(),
        forms,
    };
    let one = |f: &str| vec![f.to_string()];
    let make = |forms: Vec<LanguageForms>| Entity {
        id: eid("big"),
        forms,
        origin: EntityOrigin::Live,
    };

    // A whole line is not a name: 7 words.
    let line = "I will never leave you, not ever.";
    let e = make(vec![lf("en-US", one(line))]);
    assert_eq!(
        refusal(w.entities.put(e.clone())),
        bounds(&e, EntityBound::FormWords)
    );
    // Four words is the edge; five is over. 64 bytes is the edge; 65 is over.
    w.entities
        .put(Entity {
            id: eid("four"),
            ..make(vec![lf("en-US", one("Mary of the Isles"))])
        })
        .unwrap();
    let five = make(vec![lf("en-US", one("Mary of the Isles too"))]);
    assert_eq!(
        refusal(w.entities.put(five.clone())),
        bounds(&five, EntityBound::FormWords)
    );
    let wide = make(vec![lf("en-US", one(&"a".repeat(65)))]);
    assert_eq!(
        refusal(w.entities.put(wide.clone())),
        bounds(&wide, EntityBound::FormBytes)
    );
    w.entities
        .put(Entity {
            id: eid("edge"),
            ..make(vec![lf("en-US", one(&"a".repeat(64)))])
        })
        .unwrap();

    // 8 forms per language is the edge.
    let forms = |n: usize| (0..n).map(|i| format!("form{i}")).collect::<Vec<_>>();
    w.entities
        .put(Entity {
            id: eid("eight"),
            ..make(vec![lf("en-US", forms(8))])
        })
        .unwrap();
    let nine = make(vec![lf("en-US", forms(9))]);
    assert_eq!(
        refusal(w.entities.put(nine.clone())),
        bounds(&nine, EntityBound::Forms)
    );

    // 8 languages is the edge.
    let langs = |n: usize| {
        (0..n)
            .map(|i| lf(&format!("x-l{i}"), one("Form")))
            .collect::<Vec<_>>()
    };
    w.entities
        .put(Entity {
            id: eid("langs"),
            ..make(langs(8))
        })
        .unwrap();
    let many = make(langs(9));
    assert_eq!(
        refusal(w.entities.put(many.clone())),
        bounds(&many, EntityBound::Languages)
    );

    // Soul's probe: 200 junk languages of 5000 long forms each.
    let long: Vec<String> = (0..5000)
        .map(|i| format!("form number {i} of a long free text channel"))
        .collect();
    let probe = make(
        (0..200)
            .map(|l| lf(&format!("x-{l}\n\u{7}"), long.clone()))
            .collect(),
    );
    assert_eq!(
        refusal(w.entities.put(probe.clone())),
        bounds(&probe, EntityBound::Languages)
    );
    assert!(w.entities.get(&eid("big")).unwrap().is_none());

    // Language tags are BCP 47 in form.
    for bad in [
        "",
        "e",
        "en_US",
        "x-0\n\u{7}",
        "-en",
        "en-",
        "en--US",
        "en-US-abcdefghi",
        "1n",
        "\u{65e5}\u{672c}",
        "en-US-aaaaaaaa-bbbbbbbb-cccccccc-dddddddd-eeeeeeee",
        "en-aaaaaaaa-aaaaaaaa-aaaaaaaa-bbbbbb",
    ] {
        let e = make(vec![lf(bad, one("Form"))]);
        assert_eq!(
            refusal(w.entities.put(e.clone())),
            bounds(&e, EntityBound::LanguageTag),
            "{bad:?}"
        );
    }
    for (i, ok) in [
        "en",
        "en-US",
        "zh-Hant-TW",
        "x-klingon",
        "de-CH-1996",
        "en-aaaaaaaa-aaaaaaaa-aaaaaaaa-bbbbb",
    ]
    .into_iter()
    .enumerate()
    {
        w.entities
            .put(Entity {
                id: eid(&format!("tag-{i}")),
                ..make(vec![lf(ok, one("Form"))])
            })
            .unwrap();
    }
    assert!(w.entities.get(&eid("big")).unwrap().is_none());
}

#[test]
fn item_language_is_a_tag() {
    let mut w = world();
    let g = group_in_bucket(10);
    for (n, bad) in ["", "en_US", "x-0\n\u{7}"].into_iter().enumerate() {
        assert!(matches!(
            refusal(
                w.corpus
                    .add(item(n + 1, bad, "Nibu left.", vec![], &g), &w.entities)
            ),
            StoreRefusal::Malformed { .. }
        ));
    }
}

#[test]
fn entities_are_append_only_and_replay_needs_them() {
    let mut w = world();
    let base = ent("z", "en-US", &["Zed"]);
    w.entities.put(base.clone()).unwrap();
    let not_append = StoreRefusal::EntityNotAppendOnly { id: eid("z") };

    let mut prepend = base.clone();
    prepend.forms.insert(
        0,
        LanguageForms {
            language: "fr".into(),
            forms: vec!["Z\u{e8}de".into()],
        },
    );
    assert_eq!(refusal(w.entities.put(prepend)), not_append);
    let mut persona = base.clone();
    persona.origin = EntityOrigin::Cast {
        persona: "someone-else".into(),
    };
    assert_eq!(refusal(w.entities.put(persona)), not_append);
    let mut duplicate = base.clone();
    duplicate.forms.push(LanguageForms {
        language: "en-US".into(),
        forms: vec!["Zeddy".into()],
    });
    assert!(matches!(
        refusal(w.entities.put(duplicate)),
        StoreRefusal::Malformed { .. }
    ));

    // A corpus replay against an entity store that lacks the speaker is refused.
    let empty = EntityStore::open(&w.dir.path().join("empty.cc")).unwrap();
    let again = CorpusItem {
        id: "item:nibu:2".into(),
        ..item(1, "en-US", "Nibu says hello.", vec![], &group_in_bucket(10))
    };
    assert_eq!(
        refusal(w.corpus.add(again, &empty)),
        StoreRefusal::UnknownEntity {
            entity: eid("nibu")
        }
    );
}

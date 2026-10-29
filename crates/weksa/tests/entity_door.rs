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
        origin: EntityOrigin::Live {
            persona: "p".into(),
        },
    };

    // A form is bounded by bytes, not words: a spaced name of any word count and
    // names in scripts that write no spaces are admitted (the Thai name counts
    // five UAX #29 words, a Han name one per character).
    for (i, name) in [
        "I will never leave you, not ever.",
        "Mary of the Isles too",
        "de la Cruz y Garcia",
        "\u{e2a}\u{e21}\u{e0a}\u{e32}\u{e22} \u{e43}\u{e08}\u{e14}\u{e35}",
        "\u{5c71}\u{7530}\u{592a}\u{90ce}",
    ]
    .into_iter()
    .enumerate()
    {
        w.entities
            .put(Entity {
                id: eid(&format!("name-{i}")),
                ..make(vec![lf("en-US", one(name))])
            })
            .unwrap();
    }
    // 64 bytes is the edge; 65 is over, counted in bytes (21 Han characters are
    // 63 bytes, 22 are 66).
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
    w.entities
        .put(Entity {
            id: eid("han-edge"),
            ..make(vec![lf("ja", one(&"\u{5c71}".repeat(21)))])
        })
        .unwrap();
    let han = make(vec![lf("ja", one(&"\u{5c71}".repeat(22)))]);
    assert_eq!(
        refusal(w.entities.put(han.clone())),
        bounds(&han, EntityBound::FormBytes)
    );

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
        fixture: "f.yaml".into(),
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

/// One entity holding `form` as its only en-US form.
fn with_form(slug: &str, form: &str) -> Entity {
    Entity {
        id: eid(slug),
        forms: vec![LanguageForms {
            language: "en-US".into(),
            forms: vec![form.into()],
        }],
        origin: EntityOrigin::Live {
            persona: "p".into(),
        },
    }
}

#[test]
fn invisible_format_characters_are_refused_in_forms() {
    let mut w = world();
    let hidden = |slug: &str| StoreRefusal::Malformed {
        key: format!("entity:{slug}"),
        why: "a form holds an invisible format character",
    };
    let at = |code: u32| char::from_u32(code).unwrap();
    // Every Cf code point except the joiners, plus U+2028 and U+2029, listed
    // from Python's unicodedata. The code-point sweep in form.rs pins the edges
    // of the rule; this checks the door applies it, in each position.
    let listed: Vec<u32> = include_str!("format_refused.txt")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(|l| u32::from_str_radix(l, 16).unwrap())
        .collect();
    assert_eq!(listed.len(), 170);
    for (n, code) in listed.iter().enumerate() {
        let c = at(*code);
        let mut forms = vec![format!("Ka{c}y")];
        if !c.is_whitespace() {
            forms.extend([format!("{c}Kay"), format!("Kay{c}")]);
        }
        for form in forms {
            assert_eq!(
                refusal(w.entities.put(with_form("fmt", &form))),
                hidden("fmt"),
                "U+{code:X} (#{n}) in {form:?}"
            );
        }
    }
    // Default_Ignorable fillers that are not Cf: Hangul and Khmer.
    for code in [0x115f, 0x1160, 0x17b4, 0x17b5, 0x3164, 0xffa0] {
        assert_eq!(
            refusal(
                w.entities
                    .put(with_form("fmt", &format!("Ka{}y", at(code))))
            ),
            hidden("fmt"),
            "U+{code:X}"
        );
    }
    assert!(w.entities.get(&eid("fmt")).unwrap().is_none());

    // The neighbours of the refused ranges, ZWNJ, ZWJ, CGJ, variation
    // selectors and the Mongolian selectors (all Mn) are admitted in a form.
    let admitted: [u32; 33] = [
        0xac,
        0xae,
        0x34f,
        0x5ff,
        0x606,
        0x61b,
        0x61d,
        0x6dc,
        0x6de,
        0x70e,
        0x710,
        0x88f,
        0x892,
        0x8e1,
        0x8e3,
        0x180b,
        0x180c,
        0x180d,
        0x180f,
        0x200a,
        0x200c,
        0x200d,
        0x2010,
        0x202f,
        0x2065,
        0x2070,
        0xfe00,
        0xfe0f,
        0xfeff - 1,
        0xfffc,
        0x110be,
        0x1d172,
        0xe0100,
    ];
    for (n, code) in admitted.into_iter().enumerate() {
        let form = format!("Ka{}y", at(code));
        w.entities
            .put(with_form(&format!("ok-{n}"), &form))
            .unwrap_or_else(|e| panic!("U+{code:X}: {e}"));
    }
    // ZWNJ in a Persian form, ZWJ in an emoji sequence, VS16 and a Mongolian
    // free variation selector beside a letter.
    for (n, form) in [
        "\u{645}\u{6cc}\u{200c}\u{62e}\u{648}\u{627}\u{647}\u{645}",
        "\u{1f469}\u{200d}\u{1f4bb}",
        "\u{2764}\u{fe0f}",
        "\u{1820}\u{180b}",
        "\u{1f600}\u{e0100}",
    ]
    .into_iter()
    .enumerate()
    {
        w.entities
            .put(with_form(&format!("script-{n}"), form))
            .unwrap();
    }
    // A form of only joiners, variation selectors, CGJ or Mongolian selectors
    // shows nothing and is refused.
    let empty_looking = |slug: &str| StoreRefusal::Malformed {
        key: format!("entity:{slug}"),
        why: "a form is more than joiners and selectors",
    };
    for form in [
        "\u{200d}",
        "\u{200c}",
        "\u{200c}\u{200d}",
        "\u{fe0f}",
        "\u{fe00}\u{fe0f}",
        "\u{34f}",
        "\u{180b}\u{180c}\u{180d}\u{180f}",
        "\u{e0100}",
        "\u{e01ef}\u{34f}\u{200d}",
    ] {
        assert_eq!(
            refusal(w.entities.put(with_form("blank", form))),
            empty_looking("blank"),
            "{form:?}"
        );
    }
    assert!(w.entities.get(&eid("blank")).unwrap().is_none());
}

#[test]
fn an_origin_attests_where_the_entity_came_from() {
    let mut w = world();
    let bad = |slug: &str, why: &'static str| StoreRefusal::Malformed {
        key: format!("entity:{slug}"),
        why,
    };
    let with = |slug: &str, origin: EntityOrigin| Entity {
        origin,
        ..with_form(slug, "Kay")
    };
    let cast = |persona: &str, fixture: &str| EntityOrigin::Cast {
        persona: persona.into(),
        fixture: fixture.into(),
    };
    assert_eq!(
        refusal(w.entities.put(with("a", cast("", "f.yaml")))),
        bad("a", "an origin names its persona")
    );
    assert_eq!(
        refusal(w.entities.put(with("b", cast("kay", "")))),
        bad("b", "a cast origin names its fixture")
    );
    assert_eq!(
        refusal(w.entities.put(with(
            "c",
            EntityOrigin::Live {
                persona: String::new()
            }
        ))),
        bad("c", "an origin names its persona")
    );
    assert_eq!(
        refusal(w.entities.put(with(
            "d",
            EntityOrigin::PublicDomain {
                work: String::new()
            }
        ))),
        bad("d", "a public-domain origin names its work")
    );
    for slug in ["a", "b", "c", "d"] {
        assert!(w.entities.get(&eid(slug)).unwrap().is_none());
    }
    w.entities.put(with("e", cast("kay", "f.yaml"))).unwrap();
    w.entities
        .put(with(
            "f",
            EntityOrigin::PublicDomain {
                work: "edda".into(),
            },
        ))
        .unwrap();
}

#[test]
fn an_origin_attestation_holds_something_readable() {
    let mut w = world();
    let with = |slug: &str, origin: EntityOrigin| Entity {
        origin,
        ..with_form(slug, "Kay")
    };
    type Build = fn(String) -> EntityOrigin;
    let origins: [(Build, &str); 4] = [
        (
            |v| EntityOrigin::Cast {
                persona: v,
                fixture: "f.yaml".into(),
            },
            "an origin names its persona",
        ),
        (
            |v| EntityOrigin::Cast {
                persona: "kay".into(),
                fixture: v,
            },
            "a cast origin names its fixture",
        ),
        (
            |v| EntityOrigin::Live { persona: v },
            "an origin names its persona",
        ),
        (
            |v| EntityOrigin::PublicDomain { work: v },
            "a public-domain origin names its work",
        ),
    ];
    let unreadable = [
        "  ".to_string(),
        "\t".into(),
        "\n".into(),
        "\u{200b}".into(),
        "\u{200d}".into(),
        "\0".into(),
        "--".into(),
        "ok\u{200b}ok".into(),
        "ok\u{2028}ok".into(),
        "ok\0ok".into(),
        "x".repeat(201),
        "x".repeat(2 * 1024 * 1024),
    ];
    let mut n = 0;
    for (build, why) in &origins {
        for bad in &unreadable {
            n += 1;
            let slug = format!("o-{n}");
            assert_eq!(
                refusal(w.entities.put(with(&slug, build(bad.clone())))),
                StoreRefusal::Malformed {
                    key: format!("entity:{slug}"),
                    why
                },
                "{why}: {:?}",
                bad.chars().take(12).collect::<String>()
            );
            assert!(w.entities.get(&eid(&slug)).unwrap().is_none());
        }
    }
    for (m, (build, _)) in origins.iter().enumerate() {
        for (k, good) in ["x".repeat(200), "two words, 2 more".to_string()]
            .into_iter()
            .enumerate()
        {
            w.entities
                .put(with(&format!("g-{}", 2 * m + k), build(good)))
                .unwrap();
        }
    }
}

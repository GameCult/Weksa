mod common;
use common::*;
use tempfile::TempDir;
use weksa::corpus::{CorpusItem, ModelFamily, Source, Turn};
use weksa::inventory::{Change, seed};
use weksa::{
    AddOutcome, CorpusStore, EntityStore, Family, InventoryEntry, InventoryRevision,
    InventoryStore, PacketStore, Refusal, Slot, SplitScheme, StoreError, StoreRefusal, ValueKind,
};

fn item(key: &str, text: &str) -> CorpusItem {
    CorpusItem {
        prior_turns: vec![Turn {
            speaker: eid("player"),
            text: "hello".into(),
        }],
        ..item_in(key, text, "scene-1", cast())
    }
}

fn refusal(r: Result<impl std::fmt::Debug, StoreError>) -> StoreRefusal {
    match r {
        Err(StoreError::Refused(r)) => r,
        other => panic!("expected a store refusal, got {other:?}"),
    }
}

fn ents(d: &TempDir) -> EntityStore {
    entities_in(d.path())
}

fn dir() -> TempDir {
    tempfile::tempdir().unwrap()
}

fn empty_corpus(d: &TempDir) -> CorpusStore {
    CorpusStore::open(&d.path().join("empty-corpus.cc")).unwrap()
}

fn malformed(key: &str, why: &'static str) -> StoreRefusal {
    StoreRefusal::Malformed {
        key: key.into(),
        why,
    }
}

#[test]
fn s1_item_immutable() {
    let d = dir();
    let path = d.path().join("c.cc");
    let mut store = CorpusStore::open(&path).unwrap();
    assert_eq!(
        store
            .add(item("item:nibu:1", "Mind the hatch."), &ents(&d))
            .unwrap(),
        AddOutcome::Added
    );
    // An equal item is idempotent, even from a reopened store.
    let mut store = CorpusStore::open(&path).unwrap();
    assert_eq!(
        store
            .add(item("item:nibu:1", "Mind the hatch."), &ents(&d))
            .unwrap(),
        AddOutcome::Unchanged
    );
    let refused = || StoreRefusal::S1ItemImmutable {
        key: "item:nibu:1".into(),
    };
    // Changed text, and every other field, is refused; the stored item stays.
    let base = item("item:nibu:1", "Mind the hatch.");
    let mut variants = vec![item("item:nibu:1", "Mind the hatch!")];
    let mut v = base.clone();
    v.source = model();
    variants.push(v);
    let mut v = base.clone();
    v.source = Source::OperatorWritten;
    variants.push(v);
    let mut v = base.clone();
    v.language = "en-GB".into();
    variants.push(v);
    let mut v = base.clone();
    v.speaker = eid("brokkr");
    variants.push(v);
    let mut v = base.clone();
    v.group = "scene-2".into();
    variants.push(v);
    let mut v = base.clone();
    v.prior_turns.clear();
    variants.push(v);
    let mut v = base.clone();
    v.admitted_on = "2026-10-30".into();
    variants.push(v);
    let mut v = base.clone();
    v.supersedes = Some("item:nibu:9".into());
    variants.push(v);
    for v in variants {
        assert_eq!(refusal(store.add(v.clone(), &ents(&d))), refused(), "{v:?}");
    }
    assert_eq!(store.get("item:nibu:1").unwrap(), Some(base.clone()));
    // A withdrawn item is a different item: re-adding it live is refused too.
    store.withdraw("item:nibu:1").unwrap();
    assert_eq!(refusal(store.add(base, &ents(&d))), refused());
}

#[test]
fn corpus_keys_are_canonical() {
    let d = dir();
    let mut store = CorpusStore::open(&d.path().join("c.cc")).unwrap();
    let long_slug = format!("item:a{}:1", "b".repeat(48));
    let not_canonical = [
        "item:a:01",
        "item:a:+1",
        "item:a:0",
        "item:A:1",
        "item:a-:1",
        "item:-a:1",
        "item:a:b:1",
        "item::1",
        "item:a:",
        "item:a:\u{661}",
        "item:a:4294967296",
        &long_slug,
    ];
    for (i, key) in not_canonical.into_iter().enumerate() {
        assert_eq!(
            refusal(store.add(item(key, &format!("line {i}")), &ents(&d))),
            malformed(key, "item key is not item:<slug>:<n>"),
            "{key}"
        );
    }
    assert!(store.all().unwrap().is_empty());
    // The edges of the canonical form are admitted.
    let longest = format!("item:{}:4294967295", "a".repeat(48));
    let canonical = ["item:a:1", "item:a-b:10", "item:7:2", "item:ab:9", &longest];
    for (i, key) in canonical.into_iter().enumerate() {
        assert_eq!(
            store
                .add(item(key, &format!("fine {i}")), &ents(&d))
                .unwrap(),
            AddOutcome::Added,
            "{key}"
        );
    }
    // An alias of a stored key is not a corpus item id at all.
    assert!(weksa::CorpusItemId::parse("item:nibu:01").is_err());
    // The last free number is handed out; past u32::MAX the slug has no free
    // key, and asking neither wraps nor panics.
    store
        .add(item("item:b:4294967294", "one before the last"), &ents(&d))
        .unwrap();
    assert_eq!(store.next_key("b").unwrap(), "item:b:4294967295");
    let full = "a".repeat(48);
    assert_eq!(
        refusal(store.next_key(&full)),
        malformed(&full, "slug has no free key")
    );
}

#[test]
fn s6_text_held_by_another_key() {
    let d = dir();
    let mut store = CorpusStore::open(&d.path().join("c.cc")).unwrap();
    store
        .add(item("item:nibu:1", "Mind the hatch."), &ents(&d))
        .unwrap();
    let elsewhere = |key: &str, text: &str| CorpusItem {
        group: "scene-2".into(),
        ..item(key, text)
    };
    let held = |key: &str, by: &str| StoreRefusal::S6TextHeld {
        key: key.into(),
        held_by: by.into(),
    };
    // The same text in another group, and in the same group, is refused.
    assert_eq!(
        refusal(store.add(elsewhere("item:nibu:2", "Mind the hatch."), &ents(&d))),
        held("item:nibu:2", "item:nibu:1")
    );
    assert_eq!(
        refusal(store.add(item("item:nibu:2", "Mind the hatch."), &ents(&d))),
        held("item:nibu:2", "item:nibu:1")
    );
    // Case, spacing and punctuation variants are the same text.
    for text in [
        "MIND THE HATCH",
        "mind   the\thatch...",
        "  Mind-the-hatch!  ",
        "Mind the hatch.\n",
    ] {
        assert_eq!(
            refusal(store.add(elsewhere("item:nibu:2", text), &ents(&d))),
            held("item:nibu:2", "item:nibu:1"),
            "{text:?}"
        );
    }
    store
        .add(
            item("item:nibu:3", "\u{c4}rger \u{fc}ber \u{d6}l"),
            &ents(&d),
        )
        .unwrap();
    assert_eq!(
        refusal(store.add(
            elsewhere("item:nibu:4", "\u{e4}rger \u{dc}BER \u{f6}l"),
            &ents(&d)
        )),
        held("item:nibu:4", "item:nibu:3")
    );
    // Text that differs in a letter or digit is a different text.
    store
        .add(item("item:nibu:5", "Mind the hatch 2"), &ents(&d))
        .unwrap();
    store
        .add(item("item:nibu:6", "Mind the hatches."), &ents(&d))
        .unwrap();
    // A withdrawn holder still holds.
    store.withdraw("item:nibu:1").unwrap();
    assert_eq!(
        refusal(store.add(elsewhere("item:nibu:2", "mind the hatch"), &ents(&d))),
        held("item:nibu:2", "item:nibu:1")
    );
    // Superseding a different key does not excuse a copy of another item.
    let mut copy = item("item:nibu:8", "Mind the hatches!");
    copy.supersedes = Some("item:nibu:5".into());
    assert_eq!(
        refusal(store.add(copy, &ents(&d))),
        held("item:nibu:8", "item:nibu:6")
    );
    // A superseder of the holder, in the holder's group, is Added.
    let mut fix = item("item:nibu:2", "mind the hatch");
    fix.supersedes = Some("item:nibu:1".into());
    assert_eq!(store.add(fix, &ents(&d)).unwrap(), AddOutcome::Added);
    // The superseded holder's text stays held against a third key.
    assert_eq!(
        refusal(store.add(elsewhere("item:nibu:7", "Mind the hatch."), &ents(&d))),
        held("item:nibu:7", "item:nibu:1")
    );
}

#[test]
fn corpus_admission_rules() {
    let d = dir();
    let mut store = CorpusStore::open(&d.path().join("c.cc")).unwrap();
    assert_eq!(
        refusal(store.add(item("nibu:1", "x"), &ents(&d))),
        malformed("nibu:1", "item key is not item:<slug>:<n>")
    );
    assert_eq!(
        refusal(store.add(item("item:nibu:x", "x"), &ents(&d))),
        malformed("item:nibu:x", "item key is not item:<slug>:<n>")
    );
    let mut born_withdrawn = item("item:nibu:1", "x");
    born_withdrawn.withdrawn = true;
    assert_eq!(
        refusal(store.add(born_withdrawn, &ents(&d))),
        malformed("item:nibu:1", "an item is admitted live, then withdrawn")
    );
    let mut orphan = item("item:nibu:1", "x");
    orphan.supersedes = Some("item:nibu:9".into());
    assert_eq!(
        refusal(store.add(orphan, &ents(&d))),
        malformed("item:nibu:1", "supersedes an item that does not exist")
    );
    // Nothing was written by the refusals.
    assert!(store.all().unwrap().is_empty());
    // A correction is a new item that supersedes the old one.
    store.add(item("item:nibu:1", "old"), &ents(&d)).unwrap();
    let mut fix = item("item:nibu:2", "new");
    fix.supersedes = Some("item:nibu:1".into());
    assert_eq!(store.add(fix, &ents(&d)).unwrap(), AddOutcome::Added);
    assert_eq!(
        refusal(store.withdraw("item:nibu:7")),
        malformed("item:nibu:7", "no such item")
    );
}

#[test]
fn s6_reads_text_under_compatibility_case_folding() {
    let d = dir();
    let mut store = CorpusStore::open(&d.path().join("c.cc")).unwrap();
    // Each pair is one text to the key: fullwidth, ligature, superscript and
    // circled-digit compatibility forms and full case folds (straße, STRASSE).
    let pairs = [
        ("Brokkr", "\u{ff22}\u{ff52}\u{ff4f}\u{ff4b}\u{ff4b}\u{ff52}"),
        ("fine", "\u{fb01}ne"),
        ("room 2", "room \u{b2}"),
        ("gate 1", "gate \u{2460}"),
        ("stra\u{df}e", "STRASSE"),
        (
            "\u{3a3}\u{3af}\u{3c3}\u{3c5}\u{3c6}\u{3bf}\u{3c2}",
            "\u{3c3}\u{3af}\u{3c3}\u{3c5}\u{3c6}\u{3bf}\u{3c3}",
        ),
        // The first NFKC step: a compatibility letter folds only after it is
        // mapped (math-bold S has no case fold of its own; black-letter H is
        // not a case variant of h until NFKC reads it as H).
        (
            "sigrun",
            "\u{1d412}\u{1d422}\u{1d420}\u{1d42b}\u{1d42e}\u{1d427}",
        ),
        ("h", "\u{210c}"),
        // The second NFKC step: a full case fold can leave a decomposed or
        // reordered sequence that only a second NFKC composes to the same key
        // (dialytika tonos, caron under line, macron below).
        ("\u{390}", "\u{3aa}\u{301}"),
        ("\u{3b0}", "\u{3ab}\u{301}"),
        ("\u{1e96}\u{30c}", "\u{21e}\u{331}"),
    ];
    for (n, (a, b)) in pairs.into_iter().enumerate() {
        let (first, second) = (
            format!("item:s6:{}", 2 * n + 1),
            format!("item:s6:{}", 2 * n + 2),
        );
        store.add(item(&first, a), &ents(&d)).unwrap();
        assert_eq!(
            refusal(store.add(item(&second, b), &ents(&d))),
            StoreRefusal::S6TextHeld {
                key: second.clone(),
                held_by: first.clone()
            },
            "{a:?} vs {b:?}"
        );
        assert!(store.get(&second).unwrap().is_none());
    }
    // A decomposed text never reaches the key: the door refuses it as not NFC.
    assert_eq!(
        refusal(store.add(item("item:s6:90", "Bro\u{308}kkr"), &ents(&d))),
        StoreRefusal::NotNfc {
            key: "item:s6:90".into()
        }
    );
    // Cyrillic a is not Latin a: homoglyphs across scripts stay apart.
    store.add(item("item:s6:91", "cat"), &ents(&d)).unwrap();
    store
        .add(item("item:s6:92", "c\u{430}t"), &ents(&d))
        .unwrap();
}

#[test]
fn s6_keeps_tone_marks_and_viramas_apart() {
    let d = dir();
    let mut store = CorpusStore::open(&d.path().join("c.cc")).unwrap();
    // Minimal pairs that differ only by a combining mark: Thai mai tho and mai
    // ek on one word, Thai tone on a short vowel, a Devanagari virama.
    let pairs = [
        ("\u{e19}\u{e49}\u{e33}", "\u{e19}\u{e33}"),
        ("\u{e44}\u{e21}\u{e49}", "\u{e44}\u{e21}\u{e48}"),
        ("\u{915}\u{94d}\u{92f}", "\u{915}\u{92f}"),
    ];
    for (n, (a, b)) in pairs.into_iter().enumerate() {
        store
            .add(item(&format!("item:mk:{}", 2 * n + 1), a), &ents(&d))
            .unwrap_or_else(|e| panic!("{a:?}: {e}"));
        store
            .add(item(&format!("item:mk:{}", 2 * n + 2), b), &ents(&d))
            .unwrap_or_else(|e| panic!("{b:?} held by {a:?}: {e}"));
    }
    // A mark alone is still no text.
    for (n, text) in ["\u{e49}", "\u{94d}"].into_iter().enumerate() {
        let key = format!("item:mk:{}", 90 + n);
        assert_eq!(
            refusal(store.add(item(&key, text), &ents(&d))),
            malformed(&key, "text has no alphanumeric character")
        );
    }
}

#[test]
fn an_attestation_holds_something_readable() {
    let d = dir();
    let mut store = CorpusStore::open(&d.path().join("c.cc")).unwrap();
    let s = |x: &str| x.to_string();
    type Build = fn(String) -> Source;
    let builders: [(Build, &'static str); 9] = [
        (
            |v| Source::CastAuthored {
                persona: v,
                fixture: "f".into(),
                author: "a".into(),
            },
            "a cast-authored source names its persona",
        ),
        (
            |v| Source::CastAuthored {
                persona: "p".into(),
                fixture: v,
                author: "a".into(),
            },
            "a cast-authored source names its fixture",
        ),
        (
            |v| Source::CastAuthored {
                persona: "p".into(),
                fixture: "f".into(),
                author: v,
            },
            "a cast-authored source names its author",
        ),
        (
            |v| Source::OperatorApproved { note: v },
            "an operator-approved source carries its note",
        ),
        (
            |v| Source::PublicDomain {
                work: v,
                author: "a".into(),
                edition_url: "u".into(),
            },
            "a public-domain source names its work",
        ),
        (
            |v| Source::PublicDomain {
                work: "w".into(),
                author: v,
                edition_url: "u".into(),
            },
            "a public-domain source names its author",
        ),
        (
            |v| Source::PublicDomain {
                work: "w".into(),
                author: "a".into(),
                edition_url: v,
            },
            "a public-domain source names its edition url",
        ),
        (
            |v| Source::ModelWritten {
                family: ModelFamily::Unrecorded,
                generator: Some(v),
                prompt_digest: None,
            },
            "a model-written source names a non-empty generator",
        ),
        (
            |v| Source::ModelWritten {
                family: ModelFamily::Unrecorded,
                generator: None,
                prompt_digest: Some(v),
            },
            "a model-written source carries a non-empty prompt digest",
        ),
    ];
    let unreadable = [
        s("  "),
        s("\t"),
        s("\n"),
        s("\u{200b}"),
        s("\u{200d}"),
        s("\0"),
        s("--"),
        s("\u{e49}"),
        s("ok\u{200b}ok"),
        s("ok\u{200d}ok"),
        s("ok\u{2028}ok"),
        s("ok\nok"),
        s("ok\0ok"),
        "x".repeat(201),
        "x".repeat(2 * 1024 * 1024),
    ];
    let mut n = 0;
    for (build, why) in &builders {
        for bad in &unreadable {
            n += 1;
            let key = format!("item:att:{n}");
            let refused = item_in(&key, &format!("line {n}"), "scene-1", build(bad.clone()));
            assert_eq!(
                refusal(store.add(refused, &ents(&d))),
                malformed(&key, why),
                "{why}: {:?}",
                bad.chars().take(12).collect::<String>()
            );
        }
    }
    assert!(store.all().unwrap().is_empty());
    // 200 bytes and plain words with spaces attest; 201 does not.
    for (m, (build, _)) in builders.iter().enumerate() {
        for (k, good) in ["x".repeat(200), s("two words, 2 more")]
            .into_iter()
            .enumerate()
        {
            let key = format!("item:att-ok:{}", 2 * m + k + 1);
            store
                .add(
                    item_in(&key, &format!("fine {m} {k}"), "scene-1", build(good)),
                    &ents(&d),
                )
                .unwrap_or_else(|e| panic!("{key}: {e}"));
        }
    }
}

#[test]
fn a_text_with_no_alphanumeric_character_is_refused() {
    let d = dir();
    let mut store = CorpusStore::open(&d.path().join("c.cc")).unwrap();
    for (n, text) in ["...", "?!", "   ", "\u{2014}\u{2014}", "\u{ff01}"]
        .into_iter()
        .enumerate()
    {
        let key = format!("item:p:{}", n + 1);
        assert_eq!(
            refusal(store.add(item(&key, text), &ents(&d))),
            malformed(&key, "text has no alphanumeric character"),
            "{text:?}"
        );
    }
    assert!(store.all().unwrap().is_empty());
    store.add(item("item:p:1", "a..."), &ents(&d)).unwrap();
}

#[test]
fn an_item_source_attests_with_non_empty_strings() {
    let d = dir();
    let mut store = CorpusStore::open(&d.path().join("c.cc")).unwrap();
    let s = |x: &str| x.to_string();
    let cast = |persona: &str, fixture: &str, author: &str| Source::CastAuthored {
        persona: s(persona),
        fixture: s(fixture),
        author: s(author),
    };
    let model = |generator: Option<&str>, digest: Option<&str>| Source::ModelWritten {
        family: ModelFamily::Unrecorded,
        generator: generator.map(s),
        prompt_digest: digest.map(s),
    };
    let public = |work: &str, author: &str, url: &str| Source::PublicDomain {
        work: s(work),
        author: s(author),
        edition_url: s(url),
    };
    let cases: [(Source, &'static str); 9] = [
        (
            cast("", "f", "a"),
            "a cast-authored source names its persona",
        ),
        (
            cast("p", "", "a"),
            "a cast-authored source names its fixture",
        ),
        (
            cast("p", "f", ""),
            "a cast-authored source names its author",
        ),
        (
            Source::OperatorApproved { note: s("") },
            "an operator-approved source carries its note",
        ),
        (
            public("", "a", "u"),
            "a public-domain source names its work",
        ),
        (
            public("w", "", "u"),
            "a public-domain source names its author",
        ),
        (
            public("w", "a", ""),
            "a public-domain source names its edition url",
        ),
        (
            model(Some(""), None),
            "a model-written source names a non-empty generator",
        ),
        (
            model(None, Some("")),
            "a model-written source carries a non-empty prompt digest",
        ),
    ];
    for (n, (source, why)) in cases.into_iter().enumerate() {
        let key = format!("item:prov:{}", n + 1);
        let refused = item_in(&key, &format!("line {n}"), "scene-1", source);
        assert_eq!(refusal(store.add(refused, &ents(&d))), malformed(&key, why));
    }
    assert!(store.all().unwrap().is_empty());
    // The same sources with every string present are admitted.
    for (n, source) in [
        cast("p", "f", "a"),
        Source::OperatorApproved { note: s("n") },
        Source::OperatorWritten,
        public("w", "a", "u"),
        model(Some("g"), Some("d")),
        model(None, None),
    ]
    .into_iter()
    .enumerate()
    {
        let key = format!("item:prov:{}", n + 1);
        let item = item_in(&key, &format!("line {n}"), "scene-1", source);
        assert_eq!(store.add(item, &ents(&d)).unwrap(), AddOutcome::Added);
    }
}

#[test]
fn s5_supersede_keeps_the_group() {
    let d = dir();
    let mut store = CorpusStore::open(&d.path().join("c.cc")).unwrap();
    store.add(item("item:nibu:1", "old"), &ents(&d)).unwrap();
    let mut elsewhere = item("item:nibu:2", "new");
    elsewhere.supersedes = Some("item:nibu:1".into());
    elsewhere.group = "scene-2".into();
    assert_eq!(
        refusal(store.add(elsewhere, &ents(&d))),
        StoreRefusal::S5SupersedeGroup {
            key: "item:nibu:2".into(),
            expected: "scene-1".into(),
            got: "scene-2".into()
        }
    );
    assert!(store.get("item:nibu:2").unwrap().is_none());
    let mut same = item("item:nibu:2", "new");
    same.supersedes = Some("item:nibu:1".into());
    assert_eq!(store.add(same, &ents(&d)).unwrap(), AddOutcome::Added);
}

#[test]
fn withdrawing_keeps_the_text_and_the_number() {
    let d = dir();
    let path = d.path().join("c.cc");
    let mut store = CorpusStore::open(&path).unwrap();
    store
        .add(item("item:nibu:1", "keep me"), &ents(&d))
        .unwrap();
    assert_eq!(store.next_key("nibu").unwrap(), "item:nibu:2");
    assert_eq!(store.next_key("other").unwrap(), "item:other:1");
    store.withdraw("item:nibu:1").unwrap();
    store.withdraw("item:nibu:1").unwrap();
    let store = CorpusStore::open(&path).unwrap();
    let got = store.get("item:nibu:1").unwrap().unwrap();
    assert!(got.withdrawn);
    assert_eq!(got.text, "keep me");
    assert_eq!(store.next_key("nibu").unwrap(), "item:nibu:2");
}

fn entries_for(rev: u32, ids: &[&str]) -> (InventoryRevision, Vec<InventoryEntry>) {
    let entries = ids.iter().map(|id| entry(id, rev)).collect();
    let changes = ids.iter().map(|id| Change::Add(iv(id))).collect();
    (
        rev_with(changes, rev, if rev == 1 { None } else { Some(rev - 1) }),
        entries,
    )
}

fn seeded(path: &std::path::Path) -> InventoryStore {
    let mut store = InventoryStore::open(path).unwrap();
    let (rev, entries) = seed();
    store.admit_revision(rev, entries).unwrap();
    store
}

#[test]
fn s2_revision_chain() {
    let d = dir();
    let mut store = InventoryStore::open(&d.path().join("i.cc")).unwrap();
    // The first revision is 1 with no parent.
    let (rev, entries) = entries_for(2, &["concept:1"]);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        StoreRefusal::S2RevisionChain {
            expected: 1,
            got: 2
        }
    );
    let (mut rev, entries) = entries_for(1, &["concept:1"]);
    rev.parent = Some(0);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        StoreRefusal::S2RevisionChain {
            expected: 1,
            got: 1
        }
    );
    let (rev, entries) = entries_for(1, &["concept:1"]);
    store.admit_revision(rev, entries).unwrap();
    // Replaying n, skipping n, or naming the wrong parent is refused.
    let (rev, entries) = entries_for(1, &["concept:2"]);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        StoreRefusal::S2RevisionChain {
            expected: 2,
            got: 1
        }
    );
    let (rev, entries) = entries_for(3, &["concept:2"]);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        StoreRefusal::S2RevisionChain {
            expected: 2,
            got: 3
        }
    );
    let (mut rev, entries) = entries_for(2, &["concept:2"]);
    rev.parent = None;
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        StoreRefusal::S2RevisionChain {
            expected: 2,
            got: 2
        }
    );
    let (rev, entries) = entries_for(2, &["concept:2"]);
    store.admit_revision(rev, entries).unwrap();
    assert_eq!(store.inventory().unwrap().latest().unwrap().n, 2);
}

#[test]
fn s3_frozen_is_final() {
    let d = dir();
    let mut store = InventoryStore::open(&d.path().join("i.cc")).unwrap();
    let (mut rev, entries) = entries_for(1, &["concept:1"]);
    rev.frozen = true;
    store.admit_revision(rev, entries).unwrap();
    let (rev, entries) = entries_for(2, &["concept:2"]);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        StoreRefusal::S3Frozen { frozen: 1 }
    );
    assert_eq!(store.inventory().unwrap().latest().unwrap().n, 1);
}

#[test]
fn s4_entry_introduced_by_its_revision() {
    let d = dir();
    let mut store = InventoryStore::open(&d.path().join("i.cc")).unwrap();
    let (rev, mut entries) = entries_for(1, &["concept:1"]);
    entries[0].introduced_rev = 2;
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        StoreRefusal::S4IntroducedRev {
            id: iv("concept:1"),
            introduced: 2,
            revision: 1
        }
    );
    assert!(store.inventory().unwrap().latest().is_none());
}

#[test]
fn revision_change_rules() {
    let d = dir();
    let mut store = seeded(&d.path().join("i.cc"));
    // An added id with no entry.
    let rev = rev_with(vec![Change::Add(iv("concept:1"))], 2, Some(1));
    assert_eq!(
        refusal(store.admit_revision(rev, vec![])),
        malformed("concept:1", "added id has no entry")
    );
    // An entry the changes do not name.
    let (rev, mut entries) = entries_for(2, &["concept:1"]);
    entries.push(entry("concept:2", 2));
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        malformed("concept:2", "entry is not in the revision's changes")
    );
    // An id already in the inventory, and one added twice.
    let (rev, entries) = entries_for(2, &["role:1"]);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        malformed("role:1", "id is already in the inventory")
    );
    let (mut rev, entries) = entries_for(2, &["concept:1"]);
    rev.changes.push(Change::Add(iv("concept:1")));
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        malformed("concept:1", "id is already in the inventory")
    );
    // A malformed entry: a feature with no value kind, a non-feature with one,
    // an extension with no value kind or with slots, an entry born retired.
    let shape = "entry shape does not match its family";
    let (rev, mut entries) = entries_for(2, &["feature:50"]);
    entries[0].applies_to = vec![Slot::Discourse];
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        malformed("feature:50", shape)
    );
    let (rev, mut entries) = entries_for(2, &["feature:50"]);
    entries[0].value_kind = Some(ValueKind::Flag);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        malformed("feature:50", shape)
    );
    let (rev, mut entries) = entries_for(2, &["concept:1"]);
    entries[0].value_kind = Some(ValueKind::Flag);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        malformed("concept:1", shape)
    );
    let (rev, entries) = entries_for(2, &["extension:1"]);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        malformed("extension:1", shape)
    );
    let (rev, mut entries) = entries_for(2, &["extension:1"]);
    entries[0].value_kind = Some(ValueKind::Flag);
    entries[0].applies_to = vec![Slot::Discourse];
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        malformed("extension:1", shape)
    );
    let (rev, mut entries) = entries_for(2, &["concept:1"]);
    entries[0].retired_rev = Some(2);
    assert_eq!(
        refusal(store.admit_revision(rev, entries)),
        malformed("concept:1", shape)
    );
    // Retire targets must exist and be live; an id changes once per revision.
    let rev = rev_with(vec![Change::Retire(iv("concept:9"))], 2, Some(1));
    assert_eq!(
        refusal(store.admit_revision(rev, vec![])),
        malformed("concept:9", "retire target is missing or already retired")
    );
    let rev = rev_with(
        vec![Change::Retire(iv("role:1")), Change::Retire(iv("role:1"))],
        2,
        Some(1),
    );
    assert_eq!(
        refusal(store.admit_revision(rev, vec![])),
        malformed("role:1", "id changes twice in one revision")
    );
    // Nothing above was written.
    let inv = store.inventory().unwrap();
    assert_eq!(inv.latest().unwrap().n, 1);
    assert!(inv.entry(&iv("concept:1")).is_none());
    assert!(inv.entry(&iv("role:1")).unwrap().retired_rev.is_none());
}

#[test]
fn a_well_formed_extension_entry_is_admitted() {
    let d = dir();
    let mut store = seeded(&d.path().join("i.cc"));
    let (rev, mut entries) = entries_for(2, &["extension:1"]);
    entries[0].value_kind = Some(ValueKind::Grade);
    store.admit_revision(rev, entries).unwrap();
    assert_eq!(
        store
            .inventory()
            .unwrap()
            .entry(&iv("extension:1"))
            .unwrap()
            .family(),
        Family::Extension
    );
}

#[test]
fn retiring_keeps_the_entry_for_earlier_revisions() {
    let d = dir();
    let path = d.path().join("i.cc");
    let mut store = seeded(&path);
    let (rev, entries) = entries_for(2, &["concept:1"]);
    store.admit_revision(rev, entries).unwrap();
    let retire = rev_with(vec![Change::Retire(iv("concept:1"))], 3, Some(2));
    store.admit_revision(retire, vec![]).unwrap();
    let inv = InventoryStore::open(&path).unwrap().inventory().unwrap();
    let id = iv("concept:1");
    assert_eq!(inv.entry(&id).unwrap().retired_rev, Some(3));
    assert!(inv.resolve(&id, 2).is_some());
    assert!(inv.resolve(&id, 3).is_none());
    let again = rev_with(vec![Change::Retire(id)], 4, Some(3));
    assert!(matches!(
        refusal(store.admit_revision(again, vec![])),
        StoreRefusal::Malformed { .. }
    ));
}

#[test]
fn revision_one_survives_revision_two() {
    let inv = inventory();
    let revisions: Vec<(u32, usize)> = inv.revisions().map(|r| (r.n, r.changes.len())).collect();
    assert_eq!(revisions, vec![(1, 54), (2, 6), (3, 1), (4, 1)]);
    // A packet of revision 1 is judged by the inventory of revision 1: the
    // revision is known (no V10), and it names nothing that came later.
    let mut p = good();
    p.inventory_rev = 1;
    let refusals = weksa::validate(&p, &inv).unwrap_err();
    assert!(
        refusals
            .iter()
            .all(|r| matches!(r, Refusal::V5Unresolved { .. })),
        "{refusals:?}"
    );
}

#[test]
fn the_seed_is_the_standards_enumerations_and_the_nuance_features() {
    let d = dir();
    let store = seeded(&d.path().join("i.cc"));
    let inv = store.inventory().unwrap();
    let count = |f: Family| inv.entries().filter(|e| e.family() == f).count();
    assert_eq!(count(Family::ReferentKind), 11);
    assert_eq!(count(Family::Relation), 16);
    assert_eq!(count(Family::Role), 8);
    assert_eq!(count(Family::SpeechAct), 8);
    assert_eq!(count(Family::Feature), 11);
    assert_eq!(
        count(Family::Concept) + count(Family::Frame) + count(Family::Extension),
        0
    );
    let gloss = |id: &str| inv.entry(&iv(id)).unwrap().gloss.clone();
    assert_eq!(gloss(PERSON), "person");
    assert_eq!(gloss(AGENT), "agent");
    assert_eq!(gloss(WARN), "warn");
    assert_eq!(gloss("relation:8"), "causes");
    assert_eq!(gloss(POLARITY), "polarity");
    assert_eq!(gloss(MODALITY), "modality");
    assert_eq!(gloss(HEDGE), "hedge");
    assert_eq!(gloss(IRONY), "irony");
    assert_eq!(
        inv.entry(&iv(IRONY)).unwrap().value_kind,
        Some(ValueKind::Flag)
    );
    assert!(
        inv.entries()
            .all(|e| e.well_formed() && e.introduced_rev == 1 && e.retired_rev.is_none())
    );
    assert!(inv.has_revision(1));
}

fn admit_one(d: &TempDir, entry: InventoryEntry) -> Result<(), StoreError> {
    let mut store = seeded(&d.path().join("i.cc"));
    let rev = rev_with(vec![Change::Add(entry.id)], 2, Some(1));
    store.admit_revision(rev, vec![entry])
}

#[test]
fn gloss_is_bounded_text() {
    let why = "gloss is 1 to 120 bytes with no control character";
    let with = |gloss: &str| {
        let d = dir();
        let mut e = entry("concept:1", 2);
        e.gloss = gloss.into();
        admit_one(&d, e)
    };
    with(&"g".repeat(120)).unwrap();
    with("two words").unwrap();
    for bad in [
        String::new(),
        "g".repeat(121),
        "a\nb".into(),
        "a\u{7}b".into(),
    ] {
        assert_eq!(refusal(with(&bad)), malformed("concept:1", why), "{bad:?}");
    }
}

#[test]
fn packet_admission_goes_through_the_validator() {
    let w = world();
    let mut store = PacketStore::open(&w.dir.path().join("p.cc")).unwrap();
    let mut bad = good();
    bad.schema_version = 5;
    match store.admit(bad, &w.inventory, &w.corpus, &w.entities) {
        Err(e @ StoreError::Invalid(_)) => {
            assert!(e.to_string().contains("V1"), "{e}");
            let StoreError::Invalid(r) = e else {
                unreachable!()
            };
            assert_eq!(r, vec![Refusal::V1SchemaVersion { found: 5 }]);
        }
        other => panic!("{other:?}"),
    }
    assert!(store.all().unwrap().is_empty());
    store
        .admit(good(), &w.inventory, &w.corpus, &w.entities)
        .unwrap();
    // The same packet is idempotent; a different one under the same key is refused.
    store
        .admit(good(), &w.inventory, &w.corpus, &w.entities)
        .unwrap();
    let mut other = good();
    other.provenance.confidence = grade(1);
    assert_eq!(
        refusal(store.admit(other, &w.inventory, &w.corpus, &w.entities)),
        StoreRefusal::PacketImmutable {
            key: PACKET_ID.into()
        }
    );
    assert_eq!(store.all().unwrap().len(), 1);
    assert_eq!(store.get(PACKET_ID).unwrap(), Some(good()));
}

#[test]
fn admission_reads_the_store_inventory() {
    // The store holds only revision 1: a revision-2 packet has no inventory to
    // stand on, whatever the caller believes about revision 2.
    let d = dir();
    let mut corpus = empty_corpus(&d);
    corpus
        .add(
            item_in("item:nibu:1", "Nibu says mind the hatch.", "g", cast()),
            &ents(&d),
        )
        .unwrap();
    let mut inventory = seeded(&d.path().join("i.cc"));
    let mut packets = PacketStore::open(&d.path().join("p.cc")).unwrap();
    assert_eq!(
        match packets.admit(good(), &inventory, &corpus, &ents(&d)) {
            Err(StoreError::Invalid(r)) => r,
            other => panic!("{other:?}"),
        },
        vec![Refusal::V10UnknownRevision { rev: 2 }]
    );
    assert!(packets.all().unwrap().is_empty());
    // The same packet admits once revision 2 is admitted.
    let mut ext = entry(EXT, 2);
    ext.value_kind = Some(ValueKind::Grade);
    admit(
        &mut inventory,
        2,
        vec![
            entry(CONCEPT_A, 2),
            entry(CONCEPT_B, 2),
            entry(FRAME, 2),
            ext,
        ],
    );
    packets
        .admit(good(), &inventory, &corpus, &ents(&d))
        .unwrap();
}

#[test]
fn corpus_item_must_resolve() {
    let w = world();
    let mut packets = PacketStore::open(&w.dir.path().join("p.cc")).unwrap();
    let mut p = good();
    p.provenance.corpus_item = weksa::CorpusItemId::parse("item:nibu:9").unwrap();
    assert_eq!(
        refusal(packets.admit(p, &w.inventory, &w.corpus, &w.entities)),
        StoreRefusal::UnknownCorpusItem {
            key: "item:nibu:9".into()
        }
    );
    assert!(packets.all().unwrap().is_empty());
    // Free text is not an item id, in Rust or on the wire; a packet with no item
    // cannot be built or read back.
    for bad in ["banana", "", "the reference sentence goes right here"] {
        assert!(weksa::CorpusItemId::parse(bad).is_err(), "{bad:?}");
    }
    let mut wire = serde_json::to_value(good().provenance).unwrap();
    for bad in [serde_json::Value::Null, "banana".into()] {
        wire["corpus_item"] = bad;
        assert!(serde_json::from_value::<weksa::packet::Provenance>(wire.clone()).is_err());
    }
    wire.as_object_mut().unwrap().remove("corpus_item");
    assert!(serde_json::from_value::<weksa::packet::Provenance>(wire).is_err());
}

#[test]
fn store_roundtrip() {
    let w = world();
    let d = dir();
    // A packet, an entry, a revision and an item written to temp stores read back equal.
    let packets = d.path().join("p.cc");
    let p = good();
    PacketStore::open(&packets)
        .unwrap()
        .admit(p.clone(), &w.inventory, &w.corpus, &w.entities)
        .unwrap();
    assert_eq!(
        PacketStore::open(&packets).unwrap().get(PACKET_ID).unwrap(),
        Some(p)
    );

    let inventories = d.path().join("i.cc");
    let declared = feature(
        "feature:1",
        ValueKind::Options(weksa::OptionSet::new(vec![lab("a"), lab("b")]).unwrap()),
        &[Slot::Relation, Slot::Referent],
    );
    let mut concept = entry("concept:1", 1);
    concept.gloss = "a gloss".into();
    let rev = rev_with(
        vec![Change::Add(iv("concept:1")), Change::Add(iv("feature:1"))],
        1,
        None,
    );
    InventoryStore::open(&inventories)
        .unwrap()
        .admit_revision(rev.clone(), vec![concept.clone(), declared.clone()])
        .unwrap();
    let back = InventoryStore::open(&inventories)
        .unwrap()
        .inventory()
        .unwrap();
    assert_eq!(back.entry(&iv("concept:1")), Some(&concept));
    assert_eq!(back.entry(&iv("feature:1")), Some(&declared));
    assert_eq!(back.latest(), Some(&rev));

    let corpus = d.path().join("c.cc");
    let it = item("item:nibu:1", "round trip");
    CorpusStore::open(&corpus)
        .unwrap()
        .add(it.clone(), &ents(&d))
        .unwrap();
    assert_eq!(
        CorpusStore::open(&corpus)
            .unwrap()
            .get("item:nibu:1")
            .unwrap(),
        Some(it)
    );
}

#[test]
fn the_committed_stores_load_and_hold_clean_data() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let inv = InventoryStore::open(&root.join("data/interlingua/inventory.cc"))
        .unwrap()
        .inventory()
        .unwrap();
    assert!(inv.has_revision(1));
    assert!(inv.entry(&iv(PERSON)).is_some());
    let items = CorpusStore::open(&root.join("data/corpus/corpus.cc"))
        .unwrap()
        .all()
        .unwrap();
    assert!(items.len() >= 3, "{} items", items.len());
    for i in &items {
        assert!(
            !i.text.trim().is_empty() && i.language == "en-US" && !i.withdrawn,
            "{i:?}"
        );
        if matches!(i.source, Source::ModelWritten { .. }) {
            assert_ne!(
                SplitScheme::V1.split(i),
                Some(weksa::Split::Test),
                "{}",
                i.id
            );
        }
    }
    let by_id = |id: &str| {
        items
            .iter()
            .find(|i| i.id == id)
            .unwrap_or_else(|| panic!("{id}"))
    };
    // brokkr:1 carries the generator string its fixture wrote, verbatim.
    let fixture = std::fs::read_to_string(
        root.join("examples/persona-lowering/brokkr-edda-voice/lowered-output.yaml"),
    )
    .unwrap();
    let written = fixture
        .lines()
        .find_map(|l| l.strip_prefix("generator: "))
        .expect("the fixture names its generator");
    assert_eq!(written, "codex-local-source-aware-draft");
    assert_eq!(
        by_id("item:brokkr:1").source,
        Source::ModelWritten {
            family: ModelFamily::OpenAi,
            generator: Some(written.into()),
            prompt_digest: None
        }
    );
    // Fixtures that record no author are Unrecorded, never CastAuthored.
    let unrecorded = Source::ModelWritten {
        family: ModelFamily::Unrecorded,
        generator: None,
        prompt_digest: None,
    };
    assert_eq!(by_id("item:nibu:1").source, unrecorded);
    assert_eq!(by_id("item:nibu:2").source, unrecorded);
    assert!(
        !items
            .iter()
            .any(|i| matches!(i.source, Source::CastAuthored { .. })),
        "no item claims a recorded human author"
    );
    assert_eq!(items.len(), 3);
    // Every committed entity attests, with non-empty strings, where it came
    // from, and a cast entity's fixture is a file of this repository.
    let entities = EntityStore::open(&root.join("data/entity/entity.cc"))
        .unwrap()
        .all()
        .unwrap();
    assert!(!entities.is_empty());
    for e in &entities {
        match &e.origin {
            weksa::entity::EntityOrigin::Cast { persona, fixture } => {
                assert!(!persona.is_empty(), "{}", e.id);
                assert!(root.join(fixture).is_file(), "{}: {fixture}", e.id);
            }
            weksa::entity::EntityOrigin::Live { persona } => assert!(!persona.is_empty()),
            weksa::entity::EntityOrigin::PublicDomain { work } => assert!(!work.is_empty()),
        }
    }
}

#[test]
fn seed_reseeded() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let entities = EntityStore::open(&root.join("data/entity/entity.cc")).unwrap();
    let ids: Vec<String> = entities
        .all()
        .unwrap()
        .iter()
        .map(|e| e.id.to_string())
        .collect();
    assert_eq!(ids, ["entity:brokkr", "entity:nibu"]);
    // The committed corpus replays through the door with its entities: every
    // speaker resolves and every span is a stored form.
    let items = CorpusStore::open(&root.join("data/corpus/corpus.cc"))
        .unwrap()
        .all()
        .unwrap();
    assert_eq!(items.len(), 3);
    let d = dir();
    let mut replay = CorpusStore::open(&d.path().join("c.cc")).unwrap();
    for i in items {
        assert!(entities.get(&i.speaker).unwrap().is_some(), "{}", i.id);
        replay.add(i, &entities).unwrap();
    }
}

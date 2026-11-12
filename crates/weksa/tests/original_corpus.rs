//! The original corpus (`data/original`), in en-US and ja-JP: per language, 28
//! passages written for the non-memorised golf-loop target (Train and
//! Validation, all model-written) and 8 public-domain Test sentences, each with
//! its full provenance. Test is only read at a checkpoint: no round or recall
//! prompt over this store holds it. The ja-JP passages are written in Japanese,
//! not translated; the ja-JP Test sentences are Aozora Bunko excerpts.

const PANEL: [&str; 2] = ["en-US", "ja-JP"];

mod common;

use std::path::Path;

use weksa::corpus::{ModelFamily, Source};
use weksa::split::{Split, SplitScheme};
use weksa::{CorpusStore, EntityStore, InventoryStore};

fn stores() -> (CorpusStore, EntityStore) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/original");
    let tmp = tempfile::tempdir().unwrap();
    for f in ["corpus.cc", "entity.cc", "inventory.cc"] {
        std::fs::copy(dir.join(f), tmp.path().join(f)).unwrap();
    }
    let keep = tmp.keep();
    (
        CorpusStore::open(&keep.join("corpus.cc")).unwrap(),
        EntityStore::open(&keep.join("entity.cc")).unwrap(),
    )
}

#[test]
fn every_original_item_records_its_author_and_the_public_domain_ones_their_provenance() {
    let (corpus, entities) = stores();
    let items = corpus.all().unwrap();
    assert_eq!(items.len(), 72);
    let (mut model, mut public) = ([0; 2], [0; 2]);
    for item in &items {
        let lang = PANEL.iter().position(|l| *l == item.language).unwrap();
        match &item.source {
            Source::ModelWritten {
                family, generator, ..
            } => {
                model[lang] += 1;
                assert!(
                    item.id
                        .starts_with(["item:original:", "item:original-ja:"][lang]),
                    "{}",
                    item.id
                );
                assert_eq!(*family, ModelFamily::Anthropic, "{}", item.id);
                let generator = generator.as_deref().unwrap_or_default();
                assert!(generator.contains("claude-sonnet-5-5"), "{}", item.id);
            }
            Source::PublicDomain {
                work,
                author,
                edition_url,
                licence,
                jurisdiction,
                translator,
                edition_year,
            } => {
                public[lang] += 1;
                assert!(
                    item.id
                        .starts_with(["item:holdout:", "item:holdout-ja:"][lang]),
                    "{}",
                    item.id
                );
                assert!(licence.contains("Public domain"), "{}", item.id);
                assert!(jurisdiction.starts_with("United States"), "{}", item.id);
                if lang == 0 {
                    // The edition is the page the text was taken from.
                    assert!(
                        edition_url.starts_with("https://www.gutenberg.org/cache/epub/")
                            && edition_url.ends_with(".txt"),
                        "{}",
                        item.id
                    );
                    assert!(
                        licence.contains("trademark or licence matter"),
                        "{}",
                        item.id
                    );
                } else {
                    // The card file the text was taken from, its sources kept attached.
                    assert!(
                        edition_url.starts_with("https://www.aozora.gr.jp/cards/")
                            && edition_url.ends_with(".html"),
                        "{}",
                        item.id
                    );
                    for kept in ["Aozora Bunko", "底本", "入力", "校正"] {
                        assert!(licence.contains(kept), "{} lacks {kept}", item.id);
                    }
                    assert!(jurisdiction.contains("Japan"), "{}", item.id);
                    assert!(translator.starts_with("none"), "{}", item.id);
                }
                assert!(
                    [work, author, translator]
                        .iter()
                        .all(|s| !s.trim().is_empty()),
                    "{}",
                    item.id
                );
                // Every edition and every author is out of copyright by date;
                // a ja-JP edition year is the work's first appearance.
                let years = [1600..=1900, 1868..=1930][lang].clone();
                assert!(years.contains(edition_year), "{}", item.id);
            }
            other => panic!("{}: not model-written or public-domain: {other:?}", item.id),
        }
        assert!(
            entities.get(&item.speaker).unwrap().is_some(),
            "{}",
            item.id
        );
    }
    assert_eq!((model, public), ([28, 28], [8, 8]));
}

#[test]
fn the_original_items_spread_over_train_and_validation_and_test_is_public_domain_only() {
    // A model-written item of a Test group is in no split
    // (`SplitScheme::split`), so Test holds the public-domain items alone.
    let (corpus, _) = stores();
    let items = corpus.all().unwrap();
    let in_split = |s| {
        items
            .iter()
            .filter(|i| SplitScheme::V1.live(i) == Some(s))
            .collect::<Vec<_>>()
    };
    for lang in PANEL {
        let of = |s| {
            in_split(s)
                .into_iter()
                .filter(|i| i.language == lang)
                .count()
        };
        assert_eq!(
            (of(Split::Train), of(Split::Validation), of(Split::Test)),
            (14, 14, 8),
            "{lang}"
        );
    }
    let test = in_split(Split::Test);
    assert_eq!(test.len(), 16);
    assert!(
        test.iter()
            .all(|i| matches!(i.source, Source::PublicDomain { .. }))
    );
    // No model-written passage was admitted into a Test group at all.
    assert!(items.iter().all(|i| {
        !matches!(i.source, Source::ModelWritten { .. })
            || SplitScheme::V1.of_bucket(SplitScheme::V1.bucket(&i.group)) != Split::Test
    }));
    // Train and Validation hold no public-domain item.
    for s in [Split::Train, Split::Validation] {
        assert!(
            in_split(s)
                .iter()
                .all(|i| matches!(i.source, Source::ModelWritten { .. }))
        );
    }
}

/// The 28 model-written items are byte-for-byte what they were before Test
/// was added: id, text, group and source, folded in key order.
#[test]
fn the_28_model_written_items_are_unchanged() {
    let (corpus, _) = stores();
    let mut items: Vec<_> = corpus
        .all()
        .unwrap()
        .into_iter()
        .filter(|i| i.id.starts_with("item:original:"))
        .collect();
    items.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(items.len(), 28);
    let lines: Vec<String> = items
        .iter()
        .map(|i| format!("{}\n{}\n{}\n{:?}", i.id, i.text, i.group, i.source))
        .collect();
    let parts: Vec<&[u8]> = lines.iter().map(|l| l.as_bytes()).collect();
    assert_eq!(
        weksa::split::fnv1a64_of(&parts),
        DIGEST_OF_THE_28,
        "an original item changed"
    );
}

/// Taken over the store at base da6bc7a, before any Test item existed.
const DIGEST_OF_THE_28: u64 = 3803723767212018190;

/// The first six items are the original golf-loop set; widening the store
/// adds items after them and never edits them.
#[test]
fn the_first_six_original_items_are_unchanged() {
    let (corpus, _) = stores();
    let want = [
        (
            "item:original:1",
            "A quiet mind is like still water in a deep bowl.",
            "original-bowl",
        ),
        (
            "item:original:2",
            "The river weakens the bank in spring and strengthens the roots in autumn.",
            "original-river",
        ),
        (
            "item:original:3",
            "The sage waits at the door until the water has settled.",
            "original-door",
        ),
        (
            "item:original:4",
            "The old gardener empties the jar and fills it again with rainwater.",
            "original-jar",
        ),
        (
            "item:original:5",
            "Under the same sky the earth keeps its patience, and the baker keeps hers.",
            "original-sky",
        ),
        (
            "item:original:6",
            "A hungry belly argues louder than any will.",
            "original-belly",
        ),
    ];
    for (id, text, group) in want {
        let item = corpus.get(id).unwrap().unwrap();
        assert_eq!(
            (item.text.as_str(), item.group.as_str()),
            (text, group),
            "{id}"
        );
    }
}

#[test]
fn the_original_inventory_is_the_dao_revision_three() {
    let dao = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/daodejing/inventory.cc");
    let own = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/original/inventory.cc");
    assert_eq!(std::fs::read(dao).unwrap(), std::fs::read(&own).unwrap());
    let tmp = tempfile::tempdir().unwrap();
    let copy = tmp.path().join("inventory.cc");
    std::fs::copy(&own, &copy).unwrap();
    assert!(InventoryStore::open(&copy).is_ok());
}

/// The probe over the real original corpus: every item is offered, none names a
/// work, and a model that answers with unrelated words reads at the noise floor.
#[test]
fn the_recall_probe_covers_every_live_original_item_and_names_no_work() {
    use weksa::model::{ModelPort, PortError};
    use weksa::run::{RecallOutcome, RunPaths, RunStores};
    struct Unrelated(Vec<String>);
    impl ModelPort for Unrelated {
        fn model(&self) -> &str {
            "unrelated"
        }
        fn answer(&mut self, p: &weksa::context::Prompt) -> Result<String, PortError> {
            self.0.push(p.text().to_string());
            Ok(r#"{"text":"stone harbours remember distant lanterns"}"#.to_string())
        }
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/original");
    let tmp = tempfile::tempdir().unwrap();
    let path = |f: &str| {
        std::fs::copy(src.join(f), tmp.path().join(f)).unwrap();
        tmp.path().join(f)
    };
    let (c, i, e) = (path("corpus.cc"), path("inventory.cc"), path("entity.cc"));
    let run = RunStores::create(
        &tmp.path().join("run"),
        "orig",
        RunPaths {
            corpus: &c,
            inventory: &i,
            entities: &e,
        },
        "m",
        "bag",
        weksa::run::LowererReach::BriefedSubagent,
        ModelFamily::Unrecorded,
        &PANEL.map(String::from),
    )
    .unwrap();
    let mut model = Unrelated(vec![]);
    let RecallOutcome::Read(reading) = run.recall(&mut model).unwrap() else {
        panic!("answered");
    };
    assert_eq!(reading.languages.len(), 2);
    for r in &reading.languages {
        assert_eq!((r.n, r.answered), (28, 28), "{}", r.language);
        assert!(r.chrf < 25.0, "{} {}", r.language, r.chrf);
    }
    assert_eq!(model.0.len(), 56);
    // No item outside Train and Validation is shown to the model.
    let all = CorpusStore::open(&c).unwrap().all().unwrap();
    for item in &all {
        let live = SplitScheme::V1.live(item);
        // A recall prompt shows the first half of the item, cut at a word boundary.
        let quarter = item.text.chars().count() / 4;
        let opening: String = item.text.chars().take(quarter).collect();
        let opening = opening.trim_end().to_string();
        let shown = model.0.iter().any(|t| t.contains(&opening));
        assert_eq!(
            shown,
            matches!(live, Some(Split::Train | Split::Validation)),
            "{}",
            item.id
        );
    }
    assert!(model.0.iter().all(|t| t.contains("work: not published")));
    assert!(model.0.iter().all(|t| !t.contains("translator:")));
}

/// Answers every prompt with text that is not a draft: nothing is admitted, so
/// the round measures only what is counted before admission.
struct Nope;

impl weksa::model::ModelPort for Nope {
    fn model(&self) -> &str {
        "nope"
    }
    fn answer(&mut self, _: &weksa::context::Prompt) -> Result<String, weksa::model::PortError> {
        Ok("not a draft".to_string())
    }
}

struct Bag;

impl weksa::score::Embedder for Bag {
    fn model(&self) -> &str {
        "bag"
    }
    fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|_| vec![1.0]).collect())
    }
}

fn original_round(family: ModelFamily) -> (weksa::run::RunStores, weksa::run::RoundRecord) {
    use weksa::run::{LowererReach, RoundOutcome, RunPaths, RunStores, round};
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/original");
    let tmp = tempfile::tempdir().unwrap().keep();
    for f in ["corpus.cc", "entity.cc", "inventory.cc"] {
        std::fs::copy(src.join(f), tmp.join(f)).unwrap();
    }
    let mut run = RunStores::create(
        &tmp.join("run"),
        "orig",
        RunPaths {
            corpus: &tmp.join("corpus.cc"),
            inventory: &tmp.join("inventory.cc"),
            entities: &tmp.join("entity.cc"),
        },
        "nope",
        "bag",
        LowererReach::ToolLessRelay,
        family,
        &PANEL.map(String::from),
    )
    .unwrap();
    let scorer = weksa::score::Scorer::new(Bag);
    match round(&mut run, &mut Nope, &scorer).unwrap() {
        RoundOutcome::Recorded(r) => (run, r),
        RoundOutcome::Unanswered { .. } => panic!("answered"),
    }
}

#[test]
fn own_family_counts_and_labels() {
    // The original corpus is Anthropic-written items, all offered.
    let (run, r) = original_round(ModelFamily::Anthropic);
    assert_eq!(r.own_family(), 56);
    assert_eq!(
        r.parse_reasons().iter().map(|c| c.count).sum::<u32>(),
        56,
        "every item was refused at Parse and says why"
    );
    let line = run.round_line(&r);
    assert!(line.contains(" own-family 56"), "{line}");

    // A different answerer family, or none recorded, labels nothing.
    for family in [ModelFamily::OpenAi, ModelFamily::Unrecorded] {
        let (run, r) = original_round(family);
        assert_eq!(r.own_family(), 0);
        assert!(!run.round_line(&r).contains("own-family"));
    }
}

/// Over a fixture store whose items are not all live: recall probes the live
/// Train and Validation items and no others (a withdrawn item, a model-written
/// item of a test group, a public-domain Test item).
#[test]
fn recall_probes_only_live_train_and_validation_items() {
    use common::{cast, entities_in, group_in_bucket, item_in, run_over};
    use weksa::model::{ModelPort, PortError};
    use weksa::run::RecallOutcome;
    struct Record(Vec<String>);
    impl ModelPort for Record {
        fn model(&self) -> &str {
            "record"
        }
        fn answer(&mut self, p: &weksa::context::Prompt) -> Result<String, PortError> {
            self.0.push(p.text().to_string());
            Ok(r#"{"text":"nothing"}"#.to_string())
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let entities = entities_in(dir.path());
    let mut store = CorpusStore::open(&dir.path().join("c.cc")).unwrap();
    let public = Source::PublicDomain {
        work: "A Work".into(),
        author: "An Author".into(),
        edition_url: "https://example.org/e".into(),
        licence: "CC0".into(),
        jurisdiction: "US".into(),
        translator: "A Translator".into(),
        edition_year: 1900,
    };
    let rows = [
        (
            "item:f:1",
            "trainmark opens here and goes on",
            10,
            cast(),
            false,
        ),
        (
            "item:f:2",
            "validmark opens here and goes on",
            50,
            cast(),
            false,
        ),
        (
            "item:f:3",
            "gonemark opens here and goes on",
            10,
            cast(),
            true,
        ),
        (
            "item:f:4",
            "modelmark opens here and goes on",
            90,
            common::model(),
            false,
        ),
        (
            "item:f:5",
            "publicmark opens here and goes on",
            90,
            public,
            false,
        ),
    ];
    for (id, text, bucket, source, withdrawn) in rows {
        let item = item_in(id, text, &group_in_bucket(bucket), source);
        store.add(item, &entities).unwrap();
        if withdrawn {
            store.withdraw(id).unwrap();
        }
    }
    drop(store);
    let run = run_over(dir.path());
    let mut m = Record(vec![]);
    let RecallOutcome::Read(reading) = run.recall(&mut m).unwrap() else {
        panic!("answered");
    };
    let r = reading.languages[0].clone();
    assert_eq!(r.n, 2);
    let shown = |mark: &str| m.0.iter().any(|t| t.contains(mark));
    assert!(shown("trainmark") && shown("validmark"));
    for hidden in ["gonemark", "modelmark", "publicmark"] {
        assert!(!shown(hidden), "recall showed {hidden}");
    }
}

/// Collects every prompt a round or recall shows the model; its answer is no
/// draft and no recall.
struct Record(Vec<String>);

impl weksa::model::ModelPort for Record {
    fn model(&self) -> &str {
        "nope"
    }
    fn answer(&mut self, p: &weksa::context::Prompt) -> Result<String, weksa::model::PortError> {
        self.0.push(p.text().to_string());
        Ok(r#"{"text":"nothing"}"#.to_string())
    }
}

/// The public-domain Test items are held out: no prompt of round 1 over
/// `data/original`, no recall prompt, and no round record names one.
#[test]
fn no_test_item_is_offered_to_a_round_or_recall_or_recorded() {
    use weksa::run::{LowererReach, RoundOutcome, RunPaths, RunStores, round};
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/original");
    let tmp = tempfile::tempdir().unwrap();
    for f in ["corpus.cc", "entity.cc", "inventory.cc"] {
        std::fs::copy(src.join(f), tmp.path().join(f)).unwrap();
    }
    let p = |f: &str| tmp.path().join(f);
    let mut run = RunStores::create(
        &p("run"),
        "orig",
        RunPaths {
            corpus: &p("corpus.cc"),
            inventory: &p("inventory.cc"),
            entities: &p("entity.cc"),
        },
        "nope",
        "bag",
        LowererReach::ToolLessRelay,
        ModelFamily::Unrecorded,
        &PANEL.map(String::from),
    )
    .unwrap();
    let items = CorpusStore::open(&p("corpus.cc")).unwrap().all().unwrap();
    let held: Vec<_> = items
        .iter()
        .filter(|i| SplitScheme::V1.live(i) == Some(Split::Test))
        .collect();
    assert_eq!(held.len(), 16);
    // A Test item's text, or any run of five of its words (twelve characters
    // of a text written without spaces), in a prompt.
    let leaks = |prompts: &[String], item: &weksa::corpus::CorpusItem| {
        let w: Vec<&str> = item.text.split_whitespace().collect();
        let c: Vec<char> = item.text.chars().collect();
        prompts.iter().any(|t| {
            t.contains(&item.text)
                || w.windows(5).any(|k| t.contains(&k.join(" ")))
                || (w.len() == 1
                    && c.windows(12)
                        .any(|k| t.contains(&k.iter().collect::<String>())))
        })
    };
    let scorer = weksa::score::Scorer::new(Bag);
    let mut model = Record(vec![]);
    let RoundOutcome::Recorded(r) = round(&mut run, &mut model, &scorer).unwrap() else {
        panic!("answered");
    };
    // The check bites: the round did show live items, 56 of them, and the
    // probe sees a shown Train item of each language.
    assert!(model.0.len() >= 56, "{}", model.0.len());
    for lang in PANEL {
        assert!(
            items
                .iter()
                .filter(|i| i.language == lang)
                .filter(|i| matches!(SplitScheme::V1.live(i), Some(Split::Train)))
                .any(|i| leaks(&model.0, i)),
            "the probe cannot see a shown {lang} Train item"
        );
    }
    for item in &held {
        assert!(!leaks(&model.0, item), "round prompt held {}", item.id);
        assert!(r.calls().iter().all(|c| !c.items.contains(&item.id)));
        assert!(r.train().iter().all(|row| row.item != item.id));
    }
    let mut recall = Record(vec![]);
    let weksa::run::RecallOutcome::Read(reading) = run.recall(&mut recall).unwrap() else {
        panic!("answered");
    };
    for rr in &reading.languages {
        assert_eq!((rr.n, rr.answered), (28, 28), "{}", rr.language);
    }
    for item in &held {
        assert!(!leaks(&recall.0, item), "recall prompt held {}", item.id);
    }
}

/// The exclusion stands: a model-written item in a Test group is read in no
/// split, and is not moved to Train or Validation.
#[test]
fn a_model_written_item_in_a_test_group_is_read_in_no_split() {
    use common::{group_in_bucket, item_in, model};
    for bucket in [80, 99] {
        let item = item_in("item:f:1", "x opens", &group_in_bucket(bucket), model());
        assert_eq!(SplitScheme::V1.split(&item), None, "bucket {bucket}");
        assert_eq!(SplitScheme::V1.live(&item), None);
    }
    // The same group holds a public-domain item in Test.
    let public = Source::PublicDomain {
        work: "w".into(),
        author: "a".into(),
        edition_url: "https://example.org/e".into(),
        licence: "l".into(),
        jurisdiction: "US".into(),
        translator: "none".into(),
        edition_year: 1900,
    };
    let item = item_in("item:f:2", "x opens", &group_in_bucket(80), public);
    assert_eq!(SplitScheme::V1.live(&item), Some(Split::Test));
}

/// An Aozora Bunko file carries ruby, 《》 and ｜, and ［＃…］ notes; the items
/// hold the base text only, in NFC.
#[test]
fn ja_test_text_carries_no_aozora_markup() {
    use unicode_normalization::UnicodeNormalization;
    let (corpus, _) = stores();
    let ja: Vec<_> = corpus
        .all()
        .unwrap()
        .into_iter()
        .filter(|i| i.language == "ja-JP")
        .collect();
    assert_eq!(ja.len(), 36);
    for item in &ja {
        for bad in ["《", "》", "｜", "［＃", "※"] {
            assert!(!item.text.contains(bad), "{} holds {bad}", item.id);
        }
        assert!(!item.text.starts_with('\u{3000}'), "{}", item.id);
        assert_eq!(
            item.text.nfc().collect::<String>(),
            item.text,
            "{}",
            item.id
        );
    }
}

/// A packet that cites an item's speaker lowers only if the speaker has a form
/// in the item's language.
#[test]
fn every_cited_speaker_has_a_form_in_its_items_language() {
    let (corpus, entities) = stores();
    for item in corpus.all().unwrap() {
        let speaker = entities.get(&item.speaker).unwrap().unwrap();
        assert!(speaker.forms_in(&item.language).is_some(), "{}", item.id);
    }
}

/// The panel [en-US, ja-JP] over `data/original`: both languages are read, each
/// with 14 Validation items, and no Test item is in any call.
#[test]
fn a_panel_round_reads_both_languages() {
    let (run, r) = original_round(ModelFamily::Unrecorded);
    assert_eq!(run.record().panel, PANEL);
    let tags: Vec<&str> = r.languages().iter().map(|l| l.language.as_str()).collect();
    assert_eq!(tags, PANEL);
    for l in r.languages() {
        let refused = &l.validation_refused;
        assert_eq!(
            refused.parse + refused.admit + refused.lower,
            14,
            "{}",
            l.language
        );
    }
    let (corpus, _) = stores();
    for item in &corpus.all().unwrap() {
        if SplitScheme::V1.live(item) == Some(Split::Test) {
            assert!(
                r.calls().iter().all(|c| !c.items.contains(&item.id)),
                "{}",
                item.id
            );
        }
    }
}

/// The eight ja-JP Test sentences, byte for byte: each is the Aozora Bunko
/// card file's text with only the ruby (《》, ｜) removed and NFC applied, a whole
/// sentence, checked against the card's XHTML on 2026-11-12. A change to one
/// is a new item, never an edit of these.
#[test]
fn the_eight_ja_test_sentences_are_pinned_verbatim() {
    let want = [
        (
            "item:holdout-ja:1",
            "親譲りの無鉄砲で小供の時から損ばかりしている。",
            "holdout-ja-bocchan-6",
            "entity:pd-soseki",
            "https://www.aozora.gr.jp/cards/000148/files/752_14964.html",
        ),
        (
            "item:holdout-ja:2",
            "尾張の停車場で他の乗組員は言合せたように、残らず下りたので、函の中にはただ上人と私と二人になった。",
            "holdout-ja-koya-hijiri-4",
            "entity:pd-kyoka",
            "https://www.aozora.gr.jp/cards/000050/files/521_20583.html",
        ),
        (
            "item:holdout-ja:3",
            "高瀬舟に乗る罪人の過半は、いわゆる心得違いのために、思わぬ科を犯した人であった。",
            "holdout-ja-takasebune-14",
            "entity:pd-ogai",
            "https://www.aozora.gr.jp/cards/000129/files/45245_22007.html",
        ),
        (
            "item:holdout-ja:4",
            "ジムは僕より身長が高いくせに、絵はずっと下手でした。",
            "holdout-ja-hitofusa-no-budo-4",
            "entity:pd-arishima",
            "https://www.aozora.gr.jp/cards/000025/files/211_20472.html",
        ),
        (
            "item:holdout-ja:5",
            "風がどうと吹いてきて、草はざわざわ、木の葉はかさかさ、木はごとんごとんと鳴りました。",
            "holdout-ja-chumon-6",
            "entity:pd-kenji",
            "https://www.aozora.gr.jp/cards/000081/files/43754_17659.html",
        ),
        (
            "item:holdout-ja:6",
            "えたいの知れない不吉な塊が私の心を始終圧えつけていた。",
            "holdout-ja-lemon-3",
            "entity:pd-kajii",
            "https://www.aozora.gr.jp/cards/000074/files/424_19826.html",
        ),
        (
            "item:holdout-ja:7",
            "多摩川の二子の渡しをわたって少しばかり行くと溝口という宿場がある。",
            "holdout-ja-wasureenu-7",
            "entity:pd-kunikida",
            "https://www.aozora.gr.jp/cards/000038/files/1409_34798.html",
        ),
        (
            "item:holdout-ja:8",
            "一人の下人が、羅生門の下で雨やみを待っていた。",
            "holdout-ja-rashomon-1",
            "entity:pd-akutagawa",
            "https://www.aozora.gr.jp/cards/000879/files/127_15260.html",
        ),
    ];
    let (corpus, _) = stores();
    for (id, text, group, speaker, url) in want {
        let item = corpus.get(id).unwrap().unwrap();
        assert_eq!(item.text, text, "{id}");
        assert_eq!(
            (item.group.as_str(), item.speaker.as_str()),
            (group, speaker),
            "{id}"
        );
        assert_eq!(item.language, "ja-JP", "{id}");
        assert_eq!(SplitScheme::V1.live(&item), Some(Split::Test), "{id}");
        let Source::PublicDomain { edition_url, .. } = &item.source else {
            panic!("{id}: not public-domain");
        };
        assert_eq!(edition_url, url, "{id}");
    }
}

/// The concepts the pinned inventory holds, as each language names them in the
/// passages (a surface that appears in a passage). Every passage of both
/// languages rests on at least one live concept, so the two panel legs are
/// measured on the same inventory.
const LEXICON: [(&str, &[&str], &[&str]); 21] = [
    ("endanger", &["endanger"], &["危う", "危な"]),
    ("prevent", &["prevent"], &["防げ", "妨げ"]),
    ("control", &["control"], &["操", "支配"]),
    (
        "request",
        &["please ask", "request"],
        &["くれませんか", "いただけませんか", "お願い"],
    ),
    ("sage", &["sage"], &["賢者"]),
    ("mind", &["mind"], &["心"]),
    ("belly", &["belly"], &["腹"]),
    ("will", &["will"], &["意志"]),
    ("bone", &["bone"], &["骨"]),
    ("empty", &["empt"], &["空にし", "空の器", "空なのに"]),
    ("fill", &["fill"], &["満た", "いっぱい"]),
    ("weaken", &["weaken"], &["弱ら", "弱っ"]),
    ("strengthen", &["strengthen"], &["強く", "強い"]),
    ("heaven", &["heaven"], &["天"]),
    ("earth", &["earth"], &["大地", "地は", "天と地"]),
    ("water", &["water"], &["水"]),
    ("compare", &["compare"], &["比べ"]),
    ("resemble", &["resemble"], &["似"]),
    ("continues_long", &[], &["長く続", "久しく続"]),
    ("true_words", &[], &["誠の言葉", "真実の言葉"]),
    ("seem_paradoxical", &[], &["逆さま"]),
];

#[test]
fn every_original_passage_rests_on_a_live_inventory_concept() {
    use weksa::inventory::Family;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/original");
    let tmp = tempfile::tempdir().unwrap();
    std::fs::copy(dir.join("inventory.cc"), tmp.path().join("inventory.cc")).unwrap();
    let inventory = InventoryStore::open(&tmp.path().join("inventory.cc"))
        .unwrap()
        .inventory()
        .unwrap();
    let rev = inventory.latest().unwrap().n;
    let live: Vec<&str> = inventory
        .entries()
        .filter(|e| e.family() == Family::Concept && e.live_at(rev))
        .map(|e| e.gloss.as_str())
        .collect();
    for (concept, _, _) in LEXICON {
        assert!(live.contains(&concept), "{concept} is not a live concept");
    }
    let (corpus, _) = stores();
    for (lang, col) in [("en-US", 0), ("ja-JP", 1)] {
        let mut hit = std::collections::BTreeSet::new();
        let mut passages = 0;
        for item in corpus.all().unwrap() {
            if item.language != lang || !matches!(item.source, Source::ModelWritten { .. }) {
                continue;
            }
            passages += 1;
            let text = item.text.to_lowercase();
            let found: Vec<&str> = LEXICON
                .iter()
                .filter(|(_, en, ja)| [en, ja][col].iter().any(|s| text.contains(s)))
                .map(|(c, _, _)| *c)
                .collect();
            assert!(!found.is_empty(), "{} rests on no live concept", item.id);
            hit.extend(found);
        }
        eprintln!(
            "{lang}: {passages} passages on a live concept, {} distinct concepts",
            hit.len()
        );
        assert_eq!(passages, 28);
        assert!(hit.len() >= 12, "{lang}: {} distinct concepts", hit.len());
    }
}

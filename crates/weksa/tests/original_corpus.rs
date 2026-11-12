//! The original corpus (`data/original`): 28 passages written for the
//! non-memorised golf-loop target (Train and Validation, all model-written)
//! and 8 public-domain Test items, each with its full provenance. Test is only
//! read at a checkpoint: no round or recall prompt over this store holds it.

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
    assert_eq!(items.len(), 36);
    let (mut model, mut public) = (0, 0);
    for item in &items {
        match &item.source {
            Source::ModelWritten {
                family, generator, ..
            } => {
                model += 1;
                assert!(item.id.starts_with("item:original:"), "{}", item.id);
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
                public += 1;
                assert!(item.id.starts_with("item:holdout:"), "{}", item.id);
                // The edition is the page the text was taken from.
                assert!(
                    edition_url.starts_with("https://www.gutenberg.org/cache/epub/")
                        && edition_url.ends_with(".txt"),
                    "{}",
                    item.id
                );
                assert!(licence.contains("Public domain"), "{}", item.id);
                assert!(
                    licence.contains("trademark or licence matter"),
                    "{}",
                    item.id
                );
                assert!(jurisdiction.starts_with("United States"), "{}", item.id);
                assert!(
                    [work, author, translator]
                        .iter()
                        .all(|s| !s.trim().is_empty()),
                    "{}",
                    item.id
                );
                // Every edition and every author is out of copyright by date.
                assert!((1600..=1900).contains(edition_year), "{}", item.id);
            }
            other => panic!("{}: not model-written or public-domain: {other:?}", item.id),
        }
        assert!(
            entities.get(&item.speaker).unwrap().is_some(),
            "{}",
            item.id
        );
    }
    assert_eq!((model, public), (28, 8));
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
    assert_eq!(in_split(Split::Train).len(), 14);
    assert_eq!(in_split(Split::Validation).len(), 14);
    let test = in_split(Split::Test);
    assert_eq!(test.len(), 8);
    assert!(
        test.iter()
            .all(|i| matches!(i.source, Source::PublicDomain { .. }))
    );
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
        &["en-US".to_string()],
    )
    .unwrap();
    let mut model = Unrelated(vec![]);
    let RecallOutcome::Read(reading) = run.recall(&mut model).unwrap() else {
        panic!("answered");
    };
    let r = reading.languages[0].clone();
    assert_eq!((r.n, r.answered), (28, 28));
    assert!(r.chrf < 25.0, "{}", r.chrf);
    assert_eq!(model.0.len(), 28);
    // No item outside Train and Validation is shown to the model.
    let all = CorpusStore::open(&c).unwrap().all().unwrap();
    for item in &all {
        let live = SplitScheme::V1.live(item);
        // A recall prompt shows the first half of the item, cut at a word boundary.
        let mut cut = item.text.len() / 4;
        while !item.text.is_char_boundary(cut) {
            cut -= 1;
        }
        let opening = item.text[..cut].trim_end().to_string();
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
        &["en-US".to_string()],
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
    assert_eq!(r.own_family(), 28);
    assert_eq!(
        r.parse_reasons().iter().map(|c| c.count).sum::<u32>(),
        28,
        "every item was refused at Parse and says why"
    );
    let line = run.round_line(&r);
    assert!(line.contains(" own-family 28"), "{line}");

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
        &["en-US".to_string()],
    )
    .unwrap();
    let items = CorpusStore::open(&p("corpus.cc")).unwrap().all().unwrap();
    let held: Vec<_> = items
        .iter()
        .filter(|i| SplitScheme::V1.live(i) == Some(Split::Test))
        .collect();
    assert_eq!(held.len(), 8);
    // A Test item's text, or any run of five of its words, in a prompt.
    let leaks = |prompts: &[String], item: &weksa::corpus::CorpusItem| {
        let w: Vec<&str> = item.text.split_whitespace().collect();
        prompts
            .iter()
            .any(|t| t.contains(&item.text) || w.windows(5).any(|k| t.contains(&k.join(" "))))
    };
    let scorer = weksa::score::Scorer::new(Bag);
    let mut model = Record(vec![]);
    let RoundOutcome::Recorded(r) = round(&mut run, &mut model, &scorer).unwrap() else {
        panic!("answered");
    };
    // The check bites: the round did show live items, 28 of them.
    assert!(model.0.len() >= 28, "{}", model.0.len());
    assert!(
        items
            .iter()
            .filter(|i| matches!(SplitScheme::V1.live(i), Some(Split::Train)))
            .any(|i| leaks(&model.0, i)),
        "the probe cannot see a shown Train item"
    );
    for item in &held {
        assert!(!leaks(&model.0, item), "round prompt held {}", item.id);
        assert!(r.calls().iter().all(|c| !c.items.contains(&item.id)));
        assert!(r.train().iter().all(|row| row.item != item.id));
    }
    let mut recall = Record(vec![]);
    let weksa::run::RecallOutcome::Read(reading) = run.recall(&mut recall).unwrap() else {
        panic!("answered");
    };
    let rr = reading.languages[0].clone();
    assert_eq!((rr.n, rr.answered), (28, 28));
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

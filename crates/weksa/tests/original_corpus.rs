//! The original corpus (`data/original`): passages written for the
//! non-memorised golf-loop target. It holds no public-domain text, so a model
//! cannot have it in pretraining, and every item says who wrote it.

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
fn every_original_item_records_its_author_and_none_is_public_domain() {
    let (corpus, entities) = stores();
    let items = corpus.all().unwrap();
    assert_eq!(items.len(), 28);
    for item in &items {
        match &item.source {
            Source::ModelWritten {
                family, generator, ..
            } => {
                assert_eq!(*family, ModelFamily::Anthropic, "{}", item.id);
                let generator = generator.as_deref().unwrap_or_default();
                assert!(generator.contains("claude-sonnet-5-5"), "{}", item.id);
            }
            other => panic!("{}: not model-written: {other:?}", item.id),
        }
        assert!(
            entities.get(&item.speaker).unwrap().is_some(),
            "{}",
            item.id
        );
    }
}

#[test]
fn the_original_items_spread_over_train_and_validation_and_none_is_in_test() {
    // Model-written items cannot be in Test (`SplitScheme::split` excludes a
    // model-written item of a test group), so this store holds none.
    let (corpus, _) = stores();
    let splits: Vec<Option<Split>> = corpus
        .all()
        .unwrap()
        .iter()
        .map(|i| SplitScheme::V1.live(i))
        .collect();
    let count = |s| splits.iter().filter(|x| **x == Some(s)).count();
    assert_eq!((count(Split::Train), count(Split::Validation)), (14, 14));
    assert_eq!(count(Split::Test), 0);
}

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
    )
    .unwrap();
    let mut model = Unrelated(vec![]);
    let RecallOutcome::Read(r) = run.recall(&mut model).unwrap() else {
        panic!("answered");
    };
    assert_eq!((r.n, r.answered), (28, 28));
    assert!(r.chrf < 25.0, "{}", r.chrf);
    assert_eq!(model.0.len(), 28);
    // No item outside Train and Validation is shown to the model.
    let all = CorpusStore::open(&c).unwrap().all().unwrap();
    for item in &all {
        let live = SplitScheme::V1.live(item);
        // A recall prompt shows the first half of the words.
        let words: Vec<&str> = item.text.split_whitespace().collect();
        let opening = words[..words.len() / 2].join(" ");
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

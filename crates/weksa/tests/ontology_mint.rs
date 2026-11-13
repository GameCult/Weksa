//! The ontology mint (`weksa loop mint-ontology`): a Train-only call that drafts
//! entries for `weksa ontology admit`. It shows a model the Train lines of one
//! panel language, the features a condition can read and that language's entry
//! lines; it admits nothing and writes nothing of the run.

mod common;

use std::path::Path;

use common::{cast, entities_in, group_in_bucket, item_in};
use weksa::context::Prompt;
use weksa::corpus::{ModelFamily, Source};
use weksa::inventory::{Change, seed};
use weksa::model::{AnswerFiles, ModelPort, PortError};
use weksa::run::{LowererReach, OntologyMintOutcome, RunPaths, RunStores};
use weksa::speaker::Annotator;
use weksa::{CorpusStore, InventoryStore, OntologyStore, Slot, ValueKind};

/// Records every prompt shown; answers with `reply`.
struct Record {
    prompts: Vec<(String, Vec<String>)>,
    reply: String,
}

impl Record {
    fn new(reply: &str) -> Self {
        Self { prompts: vec![], reply: reply.to_string() }
    }
}

impl ModelPort for Record {
    fn model(&self) -> &str {
        "record"
    }
    fn answer(&mut self, p: &Prompt) -> Result<String, PortError> {
        self.prompts.push((p.text().to_string(), p.item_keys().to_vec()));
        Ok(self.reply.clone())
    }
}

fn public() -> Source {
    Source::PublicDomain {
        work: "A Work".into(),
        author: "An Author".into(),
        edition_url: "https://example.org/e".into(),
        licence: "CC0".into(),
        jurisdiction: "US".into(),
        translator: "A Translator".into(),
        edition_year: 1900,
    }
}

const STANCE_G: &str = "feature:60";
const DISC_FLAG: &str = "feature:61";
const REF_ONLY: &str = "feature:62";
const PRED_ONLY: &str = "feature:63";
const STANCE_RETIRED: &str = "feature:64";
const SCENE_G: &str = "feature:65";
const SCENE_OPT: &str = "feature:66";

/// A fixture: items in en-US and ja-JP across every split and a withdrawal, an
/// inventory with condition-readable and unreadable features, an ontology with
/// en-US and ja-JP entries, and a run over them with `panel`.
fn fixture(dir: &Path, panel: &[&str]) -> RunStores {
    let entities = entities_in(dir);
    let mut corpus = CorpusStore::open(&dir.join("c.cc")).unwrap();
    let rows = [
        ("item:en:1", "en-US", "entrain mark one", 10, cast(), false),
        ("item:en:2", "en-US", "entrain mark two", 11, cast(), false),
        ("item:en:3", "en-US", "envalid mark three", 50, cast(), false),
        ("item:en:4", "en-US", "engone mark four", 12, cast(), true),
        ("item:en:5", "en-US", "entest mark five", 90, public(), false),
        ("item:ja:1", "ja-JP", "jatrain mark one", 13, cast(), false),
        ("item:ja:2", "ja-JP", "javalid mark two", 51, cast(), false),
        ("item:ja:3", "ja-JP", "jatest mark three", 91, public(), false),
    ];
    for (id, language, text, bucket, source, withdrawn) in rows {
        let mut item = item_in(id, text, &group_in_bucket(bucket), source);
        item.language = language.into();
        corpus.add(item, &entities).unwrap();
        if withdrawn {
            corpus.withdraw(id).unwrap();
        }
    }
    drop(corpus);

    let mut inv = InventoryStore::open(&dir.join("i.cc")).unwrap();
    let (rev1, entries) = seed();
    inv.admit_revision(rev1, entries).unwrap();
    let f = |id, kind, slots: &[Slot]| common::feature(id, kind, slots);
    let features = vec![
        f(STANCE_G, ValueKind::Grade, &[Slot::Stance]),
        f(DISC_FLAG, ValueKind::Flag, &[Slot::Discourse]),
        f(REF_ONLY, ValueKind::Grade, &[Slot::Referent]),
        f(PRED_ONLY, ValueKind::Grade, &[Slot::Predication]),
        f(STANCE_RETIRED, ValueKind::Grade, &[Slot::Stance]),
        f(SCENE_G, ValueKind::Grade, &[Slot::Scene]),
        f(SCENE_OPT, ValueKind::Options(weksa::OptionSet::new(vec![weksa::OptionLabel::parse("below").unwrap(), weksa::OptionLabel::parse("above").unwrap()]).unwrap()), &[Slot::Scene]),
    ];
    let changes = features.iter().map(|e| Change::Add(e.id)).collect();
    let mut features = features;
    for e in &mut features {
        e.introduced_rev = 2;
    }
    inv.admit_revision(common::rev_with(changes, 2, Some(1)), features).unwrap();
    inv.admit_revision(
        common::rev_with(vec![Change::Retire(common::iv(STANCE_RETIRED))], 3, Some(2)),
        vec![],
    )
    .unwrap();

    let mut onto = OntologyStore::open(&dir.join("ontology.cc")).unwrap();
    let annotator = Annotator::Model { family: ModelFamily::Anthropic, generator: "seed".into() };
    let entry = |language: &str, choice: &str, examples: &[&str]| {
        weksa::encode::OntologyDraft::parse(&format!(
            r#"{{"entries":[{{"layer":"Grammar","choice":"{choice}","examples":[{}]}}]}}"#,
            examples.iter().map(|x| format!("\"{x}\"")).collect::<Vec<_>>().join(",")
        ))
        .unwrap()
        .into_entries(language, 3, 1, &annotator)
    };
    onto.admit(entry("en-US", "enchoice marker", &["enexample marker"]), &inv).unwrap();
    onto.admit(entry("ja-JP", "jachoice marker", &["jaexample marker"]), &inv).unwrap();
    drop(onto);
    drop(inv);

    std::fs::write(dir.join("speaker-state.cc"), [0x90]).unwrap();
    let panel: Vec<String> = panel.iter().map(|s| s.to_string()).collect();
    RunStores::create(
        &dir.join("run"),
        "t",
        RunPaths {
            corpus: &dir.join("c.cc"),
            inventory: &dir.join("i.cc"),
            entities: &dir.join("entities.cc"),
            ontology: &dir.join("ontology.cc"),
            speaker_state: &dir.join("speaker-state.cc"),
        },
        "fake-model",
        "bag",
        LowererReach::ToolLessRelay,
        ModelFamily::Unrecorded,
        &panel,
    )
    .unwrap()
}

const DRAFT: &str =
    r#"{"entries":[{"layer":"Discourse","when":[],"choice":"a pattern","examples":[]}]}"#;

#[test]
fn mint_ontology_prompt_shows_only_train_items_of_the_language() {
    let dir = tempfile::tempdir().unwrap();
    let run = fixture(dir.path(), &["en-US", "ja-JP"]);
    let mut m = Record::new(DRAFT);
    run.mint_ontology(&mut m, "en-US").unwrap();
    let (text, keys) = &m.prompts[0];
    assert!(text.contains("language: en-US"));
    assert_eq!(keys, &["item:en:1", "item:en:2"]);
    for shown in ["entrain mark one", "entrain mark two"] {
        assert!(text.contains(shown), "{shown}");
    }
    for hidden in [
        "envalid", "engone", "entest", "jatrain", "javalid", "jatest", "item:en:3", "item:en:5",
        "item:ja",
    ] {
        assert!(!text.contains(hidden), "the en-US mint showed {hidden}");
    }
    let mut m = Record::new(DRAFT);
    run.mint_ontology(&mut m, "ja-JP").unwrap();
    let (text, keys) = &m.prompts[0];
    assert_eq!(keys, &["item:ja:1"]);
    assert!(text.contains("jatrain mark one") && !text.contains("entrain"));
}

#[test]
fn mint_ontology_refuses_a_language_outside_the_panel() {
    let dir = tempfile::tempdir().unwrap();
    let run = fixture(dir.path(), &["en-US"]);
    let mut m = Record::new(DRAFT);
    // ja-JP has Train items in the corpus but is not in the panel.
    for (language, code) in [("ja-JP", "language-not-in-panel"), ("fr-FR", "language-not-in-panel")] {
        assert_eq!(
            run.mint_ontology(&mut m, language).unwrap(),
            OntologyMintOutcome::Refused { code }
        );
    }
    assert!(m.prompts.is_empty(), "a refused mint asked the model");
}

#[test]
fn mint_ontology_prompt_lists_only_condition_readable_features() {
    let dir = tempfile::tempdir().unwrap();
    let run = fixture(dir.path(), &["en-US"]);
    let mut m = Record::new(DRAFT);
    run.mint_ontology(&mut m, "en-US").unwrap();
    let text = &m.prompts[0].0;
    let listed = |id: &str| text.contains(&format!("\n{id}: "));
    for id in [STANCE_G, DISC_FLAG, SCENE_G] {
        assert!(listed(id), "{id} is readable by a condition");
    }
    for id in [REF_ONLY, PRED_ONLY, STANCE_RETIRED] {
        assert!(!listed(id), "{id} is not");
    }
    // Nothing that is not a feature: no concept or role line.
    assert!(!text.contains("\nconcept:") && !text.contains("\nrole:"));
    assert!(text.contains("features rev:3"));
}

#[test]
fn mint_ontology_prompt_lists_only_the_languages_existing_entries() {
    let dir = tempfile::tempdir().unwrap();
    let run = fixture(dir.path(), &["en-US", "ja-JP"]);
    let mut m = Record::new(DRAFT);
    run.mint_ontology(&mut m, "en-US").unwrap();
    let text = &m.prompts[0].0;
    assert!(text.contains("onto:en-US:1 enchoice marker"));
    for hidden in ["jachoice", "onto:ja-JP", "enexample", "jaexample"] {
        assert!(!text.contains(hidden), "{hidden}");
    }
}

fn digests(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = vec![];
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if !p.starts_with(dir.join("run").join("ontology-mint")) {
                out.push((p.display().to_string(), std::fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn mint_ontology_writes_nothing_but_its_prompt_and_answer_files() {
    let dir = tempfile::tempdir().unwrap();
    let run = fixture(dir.path(), &["en-US", "ja-JP"]);
    let before = digests(dir.path());
    let mut files = AnswerFiles::new(run.ontology_mint_dir(), "m".into(), vec![]);
    let OntologyMintOutcome::Unanswered { prompt } =
        run.mint_ontology(&mut files, "en-US").unwrap()
    else {
        panic!("unanswered");
    };
    std::fs::write(run.ontology_mint_dir().join(format!("{prompt}.answer")), DRAFT).unwrap();
    assert!(matches!(
        run.mint_ontology(&mut files, "en-US").unwrap(),
        OntologyMintOutcome::Drafted { entries: 1, .. }
    ));
    assert_eq!(digests(dir.path()), before, "the mint wrote outside ontology-mint");
    let mut names: Vec<String> = std::fs::read_dir(run.ontology_mint_dir())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, [format!("{prompt}.answer"), format!("{prompt}.prompt")]);
    drop(run);
    RunStores::open(&dir.path().join("run")).expect("no Drift after a mint");
}

#[test]
fn mint_ontology_unanswered_then_drafted_then_refused() {
    let dir = tempfile::tempdir().unwrap();
    let run = fixture(dir.path(), &["en-US"]);
    let mut files = AnswerFiles::new(run.ontology_mint_dir(), "m".into(), vec![]);
    let OntologyMintOutcome::Unanswered { prompt } =
        run.mint_ontology(&mut files, "en-US").unwrap()
    else {
        panic!("unanswered");
    };
    let mint = run.ontology_mint_dir();
    assert!(mint.join(format!("{prompt}.prompt")).exists());
    let answer = mint.join(format!("{prompt}.answer"));
    std::fs::write(&answer, DRAFT).unwrap();
    assert_eq!(
        run.mint_ontology(&mut files, "en-US").unwrap(),
        OntologyMintOutcome::Drafted { prompt: prompt.clone(), entries: 1 }
    );
    let two = r#"{"entries":[{"layer":"Grammar","choice":"a"},{"layer":"Lexicon","choice":"b"}]}"#;
    std::fs::write(&answer, two).unwrap();
    assert!(matches!(
        run.mint_ontology(&mut files, "en-US").unwrap(),
        OntologyMintOutcome::Drafted { entries: 2, .. }
    ));
    let unknown = r#"{"entries":[{"layer":"Grammar","choice":"a","key":"x"}]}"#;
    std::fs::write(&answer, unknown).unwrap();
    assert_eq!(
        run.mint_ontology(&mut files, "en-US").unwrap(),
        OntologyMintOutcome::Refused { code: "unknown-field" }
    );
    std::fs::write(&answer, "not json").unwrap();
    assert_eq!(
        run.mint_ontology(&mut files, "en-US").unwrap(),
        OntologyMintOutcome::Refused { code: "syntax" }
    );
}

#[test]
fn a_mint_answer_admits_into_a_copy_and_a_new_run_pins_the_copy() {
    let dir = tempfile::tempdir().unwrap();
    let run = fixture(dir.path(), &["en-US"]);
    let pinned = std::fs::read(dir.path().join("ontology.cc")).unwrap();
    let pin = run.record().ontology_digest.clone();
    let draft = format!(
        r#"{{"entries":[{{"layer":"Discourse","when":[{{"slot":"Stance","feature":"{STANCE_G}","test":{{"AtLeast":500}}}}],"choice":"a grown choice","examples":[]}}]}}"#
    );
    let mut files = AnswerFiles::new(run.ontology_mint_dir(), "m".into(), vec![]);
    let OntologyMintOutcome::Unanswered { prompt } =
        run.mint_ontology(&mut files, "en-US").unwrap()
    else {
        panic!("unanswered");
    };
    let answer = run.ontology_mint_dir().join(format!("{prompt}.answer"));
    std::fs::write(&answer, &draft).unwrap();

    let copy = dir.path().join("grown.cc");
    std::fs::copy(dir.path().join("ontology.cc"), &copy).unwrap();
    let inv = InventoryStore::open(&dir.path().join("i.cc")).unwrap();
    let mut grown = OntologyStore::open(&copy).unwrap();
    let annotator = Annotator::Model { family: ModelFamily::Anthropic, generator: "minter".into() };
    let first_n = grown.ontology().unwrap().entries().iter().filter(|e| e.language == "en-US").count() as u32 + 1;
    let entries = weksa::encode::OntologyDraft::parse(&std::fs::read_to_string(&answer).unwrap())
        .unwrap()
        .into_entries("en-US", 3, first_n, &annotator);
    grown.admit(entries, &inv).unwrap();
    drop(grown);

    assert_eq!(std::fs::read(dir.path().join("ontology.cc")).unwrap(), pinned);
    let again = RunStores::open(&dir.path().join("run")).unwrap();
    assert_eq!(again.record().ontology_digest, pin);

    let second = RunStores::create(
        &dir.path().join("run-grown"),
        "t",
        RunPaths {
            corpus: &dir.path().join("c.cc"),
            inventory: &dir.path().join("i.cc"),
            entities: &dir.path().join("entities.cc"),
            ontology: &copy,
            speaker_state: &dir.path().join("speaker-state.cc"),
        },
        "fake-model",
        "bag",
        LowererReach::ToolLessRelay,
        ModelFamily::Unrecorded,
        &["en-US".to_string()],
    )
    .unwrap();
    assert_ne!(second.record().ontology_digest, pin);

    // A condition on a feature outside the inventory is the store's to refuse.
    let outside = draft.replace(STANCE_G, "feature:99");
    let entries = weksa::encode::OntologyDraft::parse(&outside)
        .unwrap()
        .into_entries("en-US", 3, 9, &annotator);
    let mut grown = OntologyStore::open(&copy).unwrap();
    assert!(grown.admit(entries, &inv).is_err());
}

/// The line after `example ` in a mint prompt.
fn example_of(text: &str) -> &str {
    text.lines().find_map(|l| l.strip_prefix("example ")).expect("the prompt carries an example")
}

#[test]
fn the_mint_prompts_own_example_parses_and_admits_through_the_door() {
    let dir = tempfile::tempdir().unwrap();
    let run = fixture(dir.path(), &["en-US"]);
    let mut m = Record::new(DRAFT);
    run.mint_ontology(&mut m, "en-US").unwrap();
    let example = example_of(&m.prompts[0].0);
    // It teaches the door's spelling: a capitalized layer, feature ids, each test form.
    assert!(example.contains(r#""layer":"Grammar""#), "{example}");
    for form in [r#"{"AtLeast":500}"#, r#"{"Is":{"Option":"#, r#"{"Is":{"Flag":true}}"#] {
        assert!(example.contains(form), "{form} in {example}");
    }
    let draft = weksa::encode::OntologyDraft::parse(example).unwrap();
    let inv = InventoryStore::open(&dir.path().join("i.cc")).unwrap();
    let copy = dir.path().join("grown.cc");
    std::fs::copy(dir.path().join("ontology.cc"), &copy).unwrap();
    let mut grown = OntologyStore::open(&copy).unwrap();
    let annotator = Annotator::Model { family: ModelFamily::Anthropic, generator: "minter".into() };
    grown.admit(draft.into_entries("en-US", 3, 2, &annotator), &inv).unwrap();
}

#[test]
fn the_mint_example_names_only_features_the_prompt_lists() {
    let dir = tempfile::tempdir().unwrap();
    let run = fixture(dir.path(), &["en-US"]);
    let mut m = Record::new(DRAFT);
    run.mint_ontology(&mut m, "en-US").unwrap();
    let example = example_of(&m.prompts[0].0);
    for unreadable in [REF_ONLY, PRED_ONLY, STANCE_RETIRED] {
        assert!(!example.contains(unreadable), "{unreadable}");
    }
}

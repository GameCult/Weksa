//! The golf loop: a pinned run encodes Train and Validation through a
//! `ModelPort`, admits, lowers, scores, and keeps a typed round record.

mod common;

use std::path::{Path, PathBuf};

use common::{ent, run_over, world};
use cultcache_rs::{CultCache, SingleFileMessagePackBackingStore};
use weksa::run::{DriftOf, RunError, RunPaths, RunRecord, RunStores};
use weksa::split::fnv1a64;

/// Rewrites the stored run record, as only a test (or a stale file) can.
fn rewrite_run(root: &Path, change: impl FnOnce(&mut RunRecord)) {
    let mut cache = CultCache::new();
    cache.register_entry_type::<RunRecord>().unwrap();
    cache
        .add_generic_backing_store(SingleFileMessagePackBackingStore::new(root.join("run.cc")))
        .unwrap();
    cache.pull_all_backing_stores().unwrap();
    let mut record = cache.get_all::<RunRecord>().unwrap().remove(0);
    change(&mut record);
    cache.put(format!("run:{}", record.label), &record).unwrap();
}

fn paths(dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
    (dir.join("c.cc"), dir.join("i.cc"), dir.join("entities.cc"))
}

fn hex(path: &Path) -> String {
    format!("{:016x}", fnv1a64(&std::fs::read(path).unwrap()))
}

#[test]
fn a_run_pins_its_stores_and_refuses_drift() {
    let mut w = world();
    let dir = w.dir.path().to_path_buf();
    let root = dir.join("run");
    let (c, i, e) = paths(&dir);
    let at = |label: &str, model: &str| {
        RunStores::create(
            &root,
            label,
            RunPaths {
                corpus: &c,
                inventory: &i,
                entities: &e,
            },
            model,
            "bag",
        )
    };

    // Opening a root with no run makes none.
    assert_eq!(RunStores::open(&root).err(), Some(RunError::NoRun));
    assert!(!root.join("run.cc").exists(), "open must not create run.cc");
    assert_eq!(
        at("Bad Label", "m").err(),
        Some(RunError::Malformed("label"))
    );
    assert_eq!(at("t", "").err(), Some(RunError::Malformed("model")));
    assert!(
        !root.join("run.cc").exists(),
        "a refused create writes nothing"
    );

    let run = at("t", "fake-model").unwrap();
    let r = run.record();
    assert_eq!((r.label.as_str(), r.model.as_str()), ("t", "fake-model"));
    assert_eq!(
        r.corpus,
        std::fs::canonicalize(&c).unwrap().to_str().unwrap()
    );
    assert_eq!(r.corpus_digest, hex(&c));
    assert_eq!(r.entity_digest, hex(&e));
    assert_eq!(r.chrf_version, weksa::score::CHRF_VERSION);
    assert_eq!(at("u", "fake-model").err(), Some(RunError::Exists));

    // The record is all open reads: the same run, from the same digests.
    assert_eq!(RunStores::open(&root).unwrap().record(), run.record());

    // A corpus written after the pin is another corpus.
    let item = common::item_in(
        "item:nibu:2",
        "Another line.",
        &common::group_in_bucket(10),
        common::cast(),
    );
    w.corpus.add(item, &w.entities).unwrap();
    assert_eq!(
        RunStores::open(&root).err(),
        Some(RunError::Drift(DriftOf::Corpus))
    );
}

#[test]
fn open_checks_each_pin_by_name() {
    let mut w = world();
    let dir = w.dir.path().to_path_buf();
    let root = dir.join("run");
    let drift = |change: &dyn Fn(&mut RunRecord)| {
        let _ = std::fs::remove_dir_all(&root);
        run_over(&dir);
        rewrite_run(&root, change);
        RunStores::open(&root).err()
    };
    assert_eq!(
        drift(&|r| r.prompt_digest = "0".repeat(16)),
        Some(RunError::Drift(DriftOf::Prompts))
    );
    assert_eq!(
        drift(&|r| r.lowerer_digest = "0".repeat(16)),
        Some(RunError::Drift(DriftOf::Lowerer))
    );
    assert_eq!(
        drift(&|r| r.corpus_digest = "0".repeat(16)),
        Some(RunError::Drift(DriftOf::Corpus))
    );
    assert_eq!(
        drift(&|r| r.entity_digest = "0".repeat(16)),
        Some(RunError::Drift(DriftOf::Entities))
    );
    assert_eq!(drift(&|_| {}), None, "the unchanged record opens");

    // An entity put after the pin is another entity store.
    let _ = std::fs::remove_dir_all(&root);
    run_over(&dir);
    w.entities.put(ent("kay", "en-US", &["Kay"])).unwrap();
    assert_eq!(
        RunStores::open(&root).err(),
        Some(RunError::Drift(DriftOf::Entities))
    );
}

#[test]
fn source_digests_cover_their_files() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let digest = |name: &str| fnv1a64(&std::fs::read(src.join(name)).unwrap());
    assert_eq!(weksa::run::PROMPT_SOURCE_DIGEST, digest("context.rs"));
    assert_eq!(weksa::run::LOWERER_SOURCE_DIGEST, digest("lower.rs"));
    assert_ne!(
        weksa::run::PROMPT_SOURCE_DIGEST,
        weksa::run::LOWERER_SOURCE_DIGEST
    );
}

/// The encode prompt of `item:nibu:1` over `world()` at revision 4.
fn prompt() -> weksa::context::Prompt {
    let w = world();
    let run = run_over(w.dir.path());
    let inv = run.inventory().inventory().unwrap();
    weksa::context::Prompt::encode(
        &run.offer("item:nibu:1").unwrap().unwrap(),
        &run.train().unwrap(),
        &inv,
        4,
    )
}

#[test]
fn encode_prompt_lists_the_live_inventory() {
    let w = world();
    let run = run_over(w.dir.path());
    let inv = run.inventory().inventory().unwrap();
    let train = run.train().unwrap();
    let offer = run.offer("item:nibu:1").unwrap().unwrap();
    let at = |rev| weksa::context::Prompt::encode(&offer, &train, &inv, rev);
    let listed = |text: &str| -> Vec<String> {
        let (_, tail) = text.split_once("\n\ninventory rev:").unwrap();
        tail.lines().skip(1).map(str::to_string).collect()
    };
    let ids = |lines: &[String]| -> Vec<String> {
        lines
            .iter()
            .map(|l| l.split(':').take(2).collect::<Vec<_>>().join(":"))
            .collect()
    };

    let p4 = at(4);
    assert!(p4.text().contains("\n\ninventory rev:4\n"));
    let lines4 = listed(p4.text());
    let live4: Vec<String> = inv
        .entries()
        .filter(|e| e.live_at(4))
        .map(|e| e.id.to_string())
        .collect();
    assert_eq!(ids(&lines4), live4, "every live entry, in id order");
    assert!(live4.contains(&common::CONCEPT_LATE.to_string()));
    assert!(
        !live4.contains(&common::CONCEPT_RETIRED.to_string()),
        "retired at 3"
    );

    let ids2 = ids(&listed(at(2).text()));
    assert!(
        ids2.contains(&common::CONCEPT_RETIRED.to_string()),
        "live at 2"
    );
    assert!(
        !ids2.contains(&common::CONCEPT_LATE.to_string()),
        "not yet introduced"
    );

    // A line carries the gloss; a feature its kind and slots, an options feature its labels.
    let line = |id: &str| {
        lines4
            .iter()
            .find(|l| l.starts_with(&format!("{id}: ")))
            .unwrap()
            .clone()
    };
    assert_eq!(
        line(common::CONCEPT_A),
        format!("{}: {}", common::CONCEPT_A, common::CONCEPT_A)
    );
    assert!(
        line(common::HEDGE).contains("[grade; slots "),
        "{}",
        line(common::HEDGE)
    );
    assert!(
        line(common::MODALITY).contains("[options "),
        "{}",
        line(common::MODALITY)
    );
    assert!(
        line(common::MODALITY).contains("potential"),
        "{}",
        line(common::MODALITY)
    );
    assert!(
        line(common::EXT).ends_with("[grade]"),
        "an extension has a kind and no slots: {}",
        line(common::EXT)
    );
    assert!(
        at(4)
            .text()
            .starts_with(weksa::context::ENCODE_INSTRUCTIONS)
    );
    assert_eq!(at(4).digest().len(), 16);
    assert_eq!(at(4).digest(), p4.digest());
    assert_ne!(at(4).digest(), at(2).digest());
    assert_eq!(
        p4.digest(),
        format!("{:016x}", fnv1a64(p4.text().as_bytes()))
    );
}

#[test]
fn answer_files_write_the_prompt_and_read_the_answer() {
    use std::io::ErrorKind;
    use weksa::model::{AnswerFiles, ModelPort, PortError};
    let p = prompt();
    let dir = tempfile::tempdir().unwrap();
    let round = dir.path().join("round-1");
    let mut files = AnswerFiles::new(round.clone(), "answerer".into());
    assert_eq!(files.model(), "answerer");

    // No answer: the prompt is written under its digest, and nothing else.
    assert_eq!(files.answer(&p), Err(PortError::Unanswered));
    let prompt_file = round.join(format!("{}.prompt", p.digest()));
    assert_eq!(std::fs::read_to_string(&prompt_file).unwrap(), p.text());
    assert_eq!(std::fs::read_dir(&round).unwrap().count(), 1);
    // Asking again leaves the equal file alone.
    let before = std::fs::metadata(&prompt_file).unwrap().modified().unwrap();
    assert_eq!(files.answer(&p), Err(PortError::Unanswered));
    assert_eq!(
        std::fs::metadata(&prompt_file).unwrap().modified().unwrap(),
        before
    );
    // A different file under the digest is refused, not overwritten.
    std::fs::write(&prompt_file, "other").unwrap();
    assert_eq!(
        files.answer(&p),
        Err(PortError::Io(ErrorKind::AlreadyExists))
    );
    assert_eq!(std::fs::read_to_string(&prompt_file).unwrap(), "other");

    // The answer is read as it was written, whitespace and all.
    let answer = round.join(format!("{}.answer", p.digest()));
    std::fs::write(&answer, " {\"x\": 1}\n").unwrap();
    assert_eq!(files.answer(&p).unwrap(), " {\"x\": 1}\n");
    std::fs::write(&answer, [0xff, 0xfe]).unwrap();
    assert_eq!(files.answer(&p), Err(PortError::Io(ErrorKind::InvalidData)));

    // A directory that cannot be made is an io error of its kind.
    std::fs::write(dir.path().join("file"), "").unwrap();
    let mut blocked = AnswerFiles::new(dir.path().join("file").join("round"), "m".into());
    assert!(matches!(blocked.answer(&p), Err(PortError::Io(_))));

    assert_eq!(
        PortError::Unanswered.to_string(),
        "the prompt is unanswered"
    );
    assert_eq!(
        PortError::Io(ErrorKind::InvalidData).to_string(),
        "io error InvalidData"
    );
}

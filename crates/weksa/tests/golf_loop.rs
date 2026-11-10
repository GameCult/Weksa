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

// ---- rounds over the Dao De Jing test bed ----

use std::collections::HashMap;

use common::Bag;
use weksa::context::{ENCODE_INSTRUCTIONS, Prompt};
use weksa::daodejing::packets;
use weksa::model::{AnswerFiles, ModelPort, PortError};
use weksa::run::{Outcome, RoundOutcome, RoundRecord, Stage, round};
use weksa::{Embedder, PacketDraft, PacketStore, Scorer};

/// Answers by the item the prompt holds; panics on an item it was not given,
/// and records every prompt it was shown.
struct FakeModel {
    name: &'static str,
    answers: HashMap<String, String>,
    seen: Vec<(String, String)>,
}

impl ModelPort for FakeModel {
    fn model(&self) -> &str {
        self.name
    }
    fn answer(&mut self, prompt: &Prompt) -> Result<String, PortError> {
        let key = prompt.item_keys()[0].clone();
        self.seen.push((prompt.digest(), prompt.text().to_string()));
        match self.answers.get(&key) {
            Some(answer) => Ok(answer.clone()),
            None => panic!("the model was asked about {key}"),
        }
    }
}

fn draft_of(item: usize) -> PacketDraft {
    PacketDraft::of(&packets()[item - 1])
}

fn json(draft: &PacketDraft) -> String {
    serde_json::to_string(draft).unwrap()
}

/// A fake that answers the Dao packets for the Train and Validation items.
fn fake(answers: &[(usize, String)]) -> FakeModel {
    FakeModel {
        name: "fake-model",
        answers: answers
            .iter()
            .map(|(n, a)| (format!("item:daodejing:{n}"), a.clone()))
            .collect(),
        seen: vec![],
    }
}

fn good_answers() -> Vec<(usize, String)> {
    (1..=3).map(|n| (n, json(&draft_of(n)))).collect()
}

struct Dao {
    _dir: tempfile::TempDir,
    root: PathBuf,
    corpus: PathBuf,
    run: RunStores,
}

/// A run over copies of `data/daodejing/{corpus,inventory,entity}.cc`.
fn dao() -> Dao {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/daodejing");
    let dir = tempfile::tempdir().unwrap();
    let copy = |from: &str, to: &str| {
        std::fs::copy(src.join(from), dir.path().join(to)).unwrap();
        dir.path().join(to)
    };
    let (corpus, inventory, entities) = (
        copy("corpus.cc", "c.cc"),
        copy("inventory.cc", "i.cc"),
        copy("entity.cc", "e.cc"),
    );
    let root = dir.path().join("run");
    let run = RunStores::create(
        &root,
        "dao",
        RunPaths {
            corpus: &corpus,
            inventory: &inventory,
            entities: &entities,
        },
        "fake-model",
        "bag",
    )
    .unwrap();
    Dao {
        _dir: dir,
        root,
        corpus,
        run,
    }
}

fn recorded(outcome: RoundOutcome) -> RoundRecord {
    match outcome {
        RoundOutcome::Recorded(r) => r,
        RoundOutcome::Unanswered { prompts } => panic!("unanswered: {prompts:?}"),
    }
}

fn two_dp(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

#[test]
fn round_over_the_dao_stores_scores_train_and_validation() {
    let mut dao = dao();
    let mut model = fake(&good_answers());
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    let pinned = dao.run.record().clone();

    assert_eq!((r.n(), r.inventory_rev()), (1, 3));
    let items: Vec<&[String]> = r.calls().iter().map(|c| c.items.as_slice()).collect();
    assert_eq!(
        items,
        [
            ["item:daodejing:1"],
            ["item:daodejing:2"],
            ["item:daodejing:3"]
        ]
    );
    for (call, (digest, _)) in r.calls().iter().zip(&model.seen) {
        assert_eq!(&call.prompt, digest);
    }

    // Train: a row per item, with the packet that was admitted and its score.
    assert_eq!(r.train().len(), 1);
    let row = &r.train()[0];
    assert_eq!(row.item, "item:daodejing:1");
    let Outcome::Scored { chrf, meaning } = row.outcome else {
        panic!("{:?}", row.outcome)
    };
    assert_eq!(two_dp(chrf), 44.18);
    assert!(meaning.is_some());
    assert_eq!((r.train_mean().count, r.train_mean().chrf), (1, Some(chrf)));

    // Validation: means and refusal counts only.
    let v = r.validation_mean();
    assert_eq!(v.count, 2);
    assert!((v.chrf.unwrap() - 78.56).abs() < 0.01, "{:?}", v.chrf);
    assert!(v.meaning.is_some());
    assert_eq!(r.validation_refused(), Default::default());

    // The record carries the run's pins.
    assert_eq!(r.prompt_digest(), pinned.prompt_digest);
    assert_eq!(r.lowerer_digest(), pinned.lowerer_digest);
    assert_eq!(r.corpus_digest(), pinned.corpus_digest);
    assert_eq!(r.entity_digest(), pinned.entity_digest);
    assert_eq!(r.model(), "fake-model");
    assert_eq!(r.embedder(), "bag");
    assert_eq!(r.chrf_version(), weksa::score::CHRF_VERSION);

    // It is stored, and the packets are in the round's own store, made by the encoder step.
    assert_eq!(
        RunStores::open(&dao.root).unwrap().rounds().unwrap(),
        std::slice::from_ref(&r)
    );
    let packets = PacketStore::open(&dao.root.join("round-1/packets.cc")).unwrap();
    let all = packets.all().unwrap();
    assert_eq!(all.len(), 3);
    let encoder = weksa::encode::encoder_id("fake-model", &pinned.prompt_digest);
    for p in &all {
        assert_eq!(p.provenance.encoder, encoder);
        assert_eq!(p.inventory_rev, 3);
        assert_eq!(p.trace, Default::default());
    }
    assert_eq!(
        row.packet.as_deref(),
        Some(
            all.iter()
                .find(|p| p.provenance.corpus_item.as_str() == "item:daodejing:1")
                .unwrap()
                .id
                .as_str()
        )
    );
    assert!(!dao.root.join("round-2").exists());
}

#[test]
fn round_never_offers_or_records_test() {
    let mut dao = dao();
    // The fake panics if asked about item 4 or 5.
    let mut model = fake(&good_answers());
    recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    assert!(
        model
            .seen
            .iter()
            .all(|(_, t)| !t.contains("item:daodejing:4") && !t.contains("item:daodejing:5"))
    );

    let corpus = weksa::CorpusStore::open(&dao.corpus).unwrap();
    let mut files = vec![dao.root.join("run.cc")];
    for entry in std::fs::read_dir(dao.root.join("round-1")).unwrap() {
        files.push(entry.unwrap().path());
    }
    let mut needles: Vec<String> = vec!["item:daodejing:4".into(), "item:daodejing:5".into()];
    needles.extend(
        ["item:daodejing:4", "item:daodejing:5"].map(|k| corpus.get(k).unwrap().unwrap().text),
    );
    for file in files {
        let bytes = std::fs::read(&file).unwrap();
        for needle in &needles {
            assert!(
                !bytes.windows(needle.len()).any(|w| w == needle.as_bytes()),
                "{} holds a Test key or text",
                file.display()
            );
        }
    }
    // Nor does the record hold any Train or Validation text.
    let run_cc = std::fs::read(dao.root.join("run.cc")).unwrap();
    for k in ["item:daodejing:1", "item:daodejing:2", "item:daodejing:3"] {
        let text = corpus.get(k).unwrap().unwrap().text;
        assert!(
            !run_cc.windows(text.len()).any(|w| w == text.as_bytes()),
            "{k} text in run.cc"
        );
    }
}

/// The key and prompt of every item a round offers, as a round would build them.
fn prompts(run: &RunStores) -> Vec<(String, Prompt)> {
    let inv = run.inventory().inventory().unwrap();
    let rev = inv.latest().unwrap().n;
    let train = run.train().unwrap();
    run.offers()
        .unwrap()
        .iter()
        .map(|o| (o.key().to_string(), Prompt::encode(o, &train, &inv, rev)))
        .collect()
}

#[test]
fn unanswered_round_writes_nothing() {
    let mut dao = dao();
    let run_cc = std::fs::read(dao.root.join("run.cc")).unwrap();
    let expected = prompts(&dao.run);
    assert_eq!(expected.len(), 3);

    let mut files = AnswerFiles::new(dao.run.next_round_dir().unwrap(), "fake-model".into());
    let RoundOutcome::Unanswered { prompts: digests } =
        round(&mut dao.run, &mut files, &Scorer::new(Bag)).unwrap()
    else {
        panic!("answered from nothing")
    };
    let want: Vec<String> = expected.iter().map(|(_, p)| p.digest()).collect();
    assert_eq!(digests, want);
    let round1 = dao.root.join("round-1");
    for (_, p) in &expected {
        assert_eq!(
            std::fs::read_to_string(round1.join(format!("{}.prompt", p.digest()))).unwrap(),
            p.text()
        );
    }
    assert_eq!(
        std::fs::read_dir(&round1).unwrap().count(),
        3,
        "prompts only: no packets.cc"
    );
    assert!(dao.run.rounds().unwrap().is_empty());
    assert_eq!(std::fs::read(dao.root.join("run.cc")).unwrap(), run_cc);

    // Two of three answers is still nothing written.
    let answers: HashMap<String, String> = good_answers()
        .into_iter()
        .map(|(n, a)| (format!("item:daodejing:{n}"), a))
        .collect();
    for (key, p) in &expected[..2] {
        std::fs::write(round1.join(format!("{}.answer", p.digest())), &answers[key]).unwrap();
    }
    let mut files = AnswerFiles::new(dao.run.next_round_dir().unwrap(), "fake-model".into());
    let RoundOutcome::Unanswered { prompts: digests } =
        round(&mut dao.run, &mut files, &Scorer::new(Bag)).unwrap()
    else {
        panic!("answered from two of three")
    };
    assert_eq!(digests, [want[2].clone()]);
    assert!(!round1.join("packets.cc").exists());
    assert_eq!(std::fs::read(dao.root.join("run.cc")).unwrap(), run_cc);

    // The last answer completes round 1; the next round is round 2 and asks again.
    let (key, p) = &expected[2];
    std::fs::write(round1.join(format!("{}.answer", p.digest())), &answers[key]).unwrap();
    let mut files = AnswerFiles::new(dao.run.next_round_dir().unwrap(), "fake-model".into());
    let r = recorded(round(&mut dao.run, &mut files, &Scorer::new(Bag)).unwrap());
    assert_eq!(r.n(), 1);
    let mut files = AnswerFiles::new(dao.run.next_round_dir().unwrap(), "fake-model".into());
    assert!(dao.root.join("round-2").parent().is_some());
    let RoundOutcome::Unanswered { prompts: again } =
        round(&mut dao.run, &mut files, &Scorer::new(Bag)).unwrap()
    else {
        panic!("round 2 answered from round 1's files")
    };
    assert_eq!(again.len(), 3);
    assert_eq!(
        std::fs::read_dir(dao.root.join("round-2")).unwrap().count(),
        3
    );
    assert_eq!(dao.run.rounds().unwrap().len(), 1);
}

struct OtherEmbedder;

impl Embedder for OtherEmbedder {
    fn model(&self) -> &str {
        "other-embedder"
    }
    fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        Bag.embed(texts)
    }
}

#[test]
fn drift_refuses_the_round() {
    let mut dao = dao();
    let mut other = fake(&good_answers());
    other.name = "other-model";
    assert_eq!(
        round(&mut dao.run, &mut other, &Scorer::new(Bag)).err(),
        Some(RunError::Drift(DriftOf::Model))
    );
    let mut model = fake(&good_answers());
    assert_eq!(
        round(&mut dao.run, &mut model, &Scorer::new(OtherEmbedder)).err(),
        Some(RunError::Drift(DriftOf::Embedder))
    );
    assert!(
        other.seen.is_empty() && model.seen.is_empty(),
        "drift is found before any call"
    );
    assert!(dao.run.rounds().unwrap().is_empty() && !dao.root.join("round-1").exists());

    // An inventory with no revision has no round.
    let w = world();
    std::fs::write(w.dir.path().join("i.cc"), [0x90]).unwrap();
    let mut empty = run_over(w.dir.path());
    let mut model = fake(&[]);
    assert_eq!(
        round(&mut empty, &mut model, &Scorer::new(Bag)).err(),
        Some(RunError::NoInventory)
    );
}

/// `n`'s good draft, edited.
fn edited(n: usize, edit: impl FnOnce(&mut PacketDraft)) -> String {
    let mut d = draft_of(n);
    edit(&mut d);
    json(&d)
}

fn unknown_concept(d: &mut PacketDraft) {
    d.referents[0].concept = "concept:999".parse().unwrap();
}

fn with_relation(d: &mut PacketDraft) {
    d.relations.push(weksa::packet::Relation {
        relation: "relation:1".parse().unwrap(),
        from: d.referents[0].id.clone(),
        to: d.referents[1].id.clone(),
        features: vec![],
    });
}

#[test]
fn refusal_stages_are_counted() {
    let run = |answers: [String; 3]| {
        let mut dao = dao();
        let mut model = fake(
            &answers
                .into_iter()
                .enumerate()
                .map(|(i, a)| (i + 1, a))
                .collect::<Vec<_>>(),
        );
        recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap())
    };
    let junk = "not json at all".to_string();

    // Train refused at each stage; Validation refused at the other two.
    let r = run([
        junk.clone(),
        edited(2, unknown_concept),
        edited(3, with_relation),
    ]);
    assert_eq!(r.train()[0].outcome, Outcome::Refused(Stage::Parse));
    assert_eq!(r.train()[0].packet, None);
    assert_eq!((r.train_mean().count, r.train_mean().chrf), (0, None));
    assert_eq!(r.validation_refused().parse, 0);
    assert_eq!(
        (r.validation_refused().admit, r.validation_refused().lower),
        (1, 1)
    );
    assert_eq!(
        (r.validation_mean().count, r.validation_mean().chrf),
        (0, None)
    );

    let r = run([
        edited(1, unknown_concept),
        junk.clone(),
        good_answers()[2].1.clone(),
    ]);
    assert_eq!(r.train()[0].outcome, Outcome::Refused(Stage::Admit));
    assert_eq!(r.train()[0].packet, None, "a refused packet is not stored");
    let v = r.validation_refused();
    assert_eq!((v.parse, v.admit, v.lower), (1, 0, 0));
    assert_eq!(
        r.validation_mean().count,
        1,
        "only the scored item is counted"
    );
    assert_eq!(two_dp(r.validation_mean().chrf.unwrap()), 74.99);

    let r = run([edited(1, with_relation), good_answers()[1].1.clone(), junk]);
    assert_eq!(r.train()[0].outcome, Outcome::Refused(Stage::Lower));
    assert!(
        r.train()[0].packet.is_some(),
        "a packet that lowers badly was admitted"
    );
    assert_eq!(r.validation_refused().parse, 1);
    assert_eq!(two_dp(r.validation_mean().chrf.unwrap()), 82.12);
}

#[test]
fn errors_echo_no_input() {
    let canary = "CANARYBYTES";
    let tmp = tempfile::tempdir().unwrap();
    let mut all: Vec<String> = Vec::new();
    let mut note = |e: &dyn std::fmt::Debug, d: &dyn std::fmt::Display| {
        all.push(format!("{e:?}"));
        all.push(d.to_string());
    };

    // A draft error, from canary bytes in an answer.
    let e = PacketDraft::parse(&format!("{{\"confidence\": {canary}}}")).unwrap_err();
    note(&e, &e);
    let e = PacketDraft::parse(&format!("{{\"{canary}\": 1}}")).unwrap_err();
    note(&e, &e);
    // Run errors, from a canary root, label and store path.
    let root = tmp.path().join("canaryroot");
    let e = RunStores::open(&root).err().unwrap();
    note(&e, &e);
    let missing = tmp.path().join("canarystore.cc");
    let stores = RunPaths {
        corpus: &missing,
        inventory: &missing,
        entities: &missing,
    };
    let e = RunStores::create(&root, "ok", stores, "m", "bag")
        .err()
        .unwrap();
    note(&e, &e);
    let e = RunStores::create(&root, &format!("{canary}!"), stores, "m", "bag")
        .err()
        .unwrap();
    note(&e, &e);
    let e = RunStores::create(&root, "ok", stores, &format!("{canary}\n"), "bag")
        .err()
        .unwrap();
    note(&e, &e);
    let w = world();
    let run = run_over(w.dir.path());
    let e = run.offer(&format!("item:{canary}:1")).err().unwrap();
    note(&e, &e);
    // A port error, from a canary directory.
    let blocker = tmp.path().join("canaryfile");
    std::fs::write(&blocker, "").unwrap();
    let mut files = AnswerFiles::new(blocker.join("canarydir"), "m".into());
    let e = files.answer(&prompt()).err().unwrap();
    note(&e, &e);
    // A round over canary answers records refusals, not the answers.
    let mut dao = dao();
    let mut model = fake(&[
        (1, canary.into()),
        (2, format!("{{\"{canary}\":1}}")),
        (3, canary.into()),
    ]);
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    all.push(format!("{r:?}"));

    for text in &all {
        assert!(!text.to_uppercase().contains("CANARY"), "{text}");
    }
    assert!(all.len() >= 12);
}

#[test]
fn prompts_come_only_from_context() {
    let mut dao = dao();
    let mut model = fake(&good_answers());
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    assert_eq!(model.seen.len(), r.calls().len());
    for ((digest, text), call) in model.seen.iter().zip(r.calls()) {
        assert!(text.starts_with(ENCODE_INSTRUCTIONS));
        assert_eq!(call.items.len(), 1);
        assert_eq!(text.matches("[item:").count(), 1);
        assert!(text.contains(&format!("[{}]", call.items[0])));
        assert_eq!(&call.prompt, digest);
        assert_eq!(*digest, format!("{:016x}", fnv1a64(text.as_bytes())));
    }
}

/// Tripwires over the source, with the limits stated: they read text, so they
/// catch a spelling and not an alias.
#[test]
fn source_tripwires() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let read = |rel: &str| std::fs::read_to_string(src.join(rel)).unwrap();
    for file in ["run.rs", "model.rs", "encode.rs"] {
        let text = read(file);
        for banned in ["Split::Test", "Checkpoint"] {
            assert!(!text.contains(banned), "{file} names {banned}");
        }
    }
    let mut all = Vec::new();
    let mut stack = vec![src.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                all.push((
                    path.strip_prefix(&src)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string(),
                    std::fs::read_to_string(&path).unwrap(),
                ));
            }
        }
    }
    for (file, text) in &all {
        let code: String = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        if file != "main.rs" {
            assert!(!code.contains("std::process"), "{file}");
        }
        // The Ollama reply is the other xenos boundary: JSON read from a server.
        if file != "encode.rs" && file != "score/embed.rs" {
            assert!(!code.contains("serde_json::from_str"), "{file}");
        }
        if file != "context.rs" {
            assert!(!code.contains("Prompt {"), "{file}");
        }
    }
    let context = read("context.rs");
    assert!(!context.contains("pub fn from_corpus") && !context.contains("pub fn for_round"));
}

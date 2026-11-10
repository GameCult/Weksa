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
            "bag", weksa::run::LowererReach::ToolLessRelay,
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
        drift(&|r| r.corpus_digest = "0".repeat(16)),
        Some(RunError::Drift(DriftOf::Corpus))
    );
    assert_eq!(
        drift(&|r| r.entity_digest = "0".repeat(16)),
        Some(RunError::Drift(DriftOf::Entities))
    );
    assert_eq!(
        drift(&|r| r.inventory_digest = "0".repeat(16)),
        Some(RunError::Drift(DriftOf::Inventory))
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
fn a_run_works_on_its_own_copy_of_the_inventory() {
    let w = world();
    let dir = w.dir.path().to_path_buf();
    let root = dir.join("run");
    let (c, i, e) = paths(&dir);
    let source_bytes = std::fs::read(&i).unwrap();
    let mut run = RunStores::create(
        &root,
        "t",
        RunPaths {
            corpus: &c,
            inventory: &i,
            entities: &e,
        },
        "fake-model",
        "bag", weksa::run::LowererReach::ToolLessRelay,
    )
    .unwrap();

    // The copy is byte for byte the source, and the record pins its digest.
    let copy = root.join("inventory.cc");
    assert_eq!(std::fs::read(&copy).unwrap(), source_bytes);
    assert_eq!(run.record().inventory_digest, hex(&i));
    assert_eq!(run.record().inventory_digest, hex(&copy));
    assert_eq!(
        run.record().inventory,
        std::fs::canonicalize(&i).unwrap().to_str().unwrap(),
        "the source path is provenance"
    );
    let rev = run.inventory().inventory().unwrap().latest().unwrap().n;

    // A revision written to the source after open is a revision the run never
    // reads, and it does not drift the run.
    let mut source = weksa::InventoryStore::open(&i).unwrap();
    source
        .admit_revision(
            weksa::InventoryRevision {
                n: rev + 1,
                parent: Some(rev),
                changes: vec![],
                frozen: false,
            },
            vec![],
        )
        .unwrap();
    assert_ne!(hex(&i), run.record().inventory_digest);
    run = RunStores::open(&root).unwrap();
    assert_eq!(
        run.inventory().inventory().unwrap().latest().unwrap().n,
        rev,
        "the run reads its copy"
    );
    assert_eq!(std::fs::read(&copy).unwrap(), source_bytes);

    // Even with the source gone the run opens.
    std::fs::remove_file(&i).unwrap();
    assert!(RunStores::open(&root).is_ok());

    // A revision appended to the copy is drift.
    let mut head = weksa::InventoryStore::open(&copy).unwrap();
    head.admit_revision(
        weksa::InventoryRevision {
            n: rev + 1,
            parent: Some(rev),
            changes: vec![],
            frozen: false,
        },
        vec![],
    )
    .unwrap();
    assert_eq!(
        RunStores::open(&root).err(),
        Some(RunError::Drift(DriftOf::Inventory))
    );
}

#[test]
fn create_refuses_a_root_that_holds_an_inventory_and_never_truncates_it() {
    let dao = dao();
    let bytes = std::fs::read(&dao.inventory).unwrap();
    let create = |root: &Path, inventory: &Path| {
        RunStores::create(
            root,
            "x",
            RunPaths {
                corpus: &dao.corpus,
                inventory,
                entities: &dao.entities,
            },
            "fake-model",
            "bag", weksa::run::LowererReach::ToolLessRelay,
        )
    };

    // The source is the root's own inventory.cc: the copy would open it as its
    // destination. It is refused and the source bytes survive.
    let own = dao.root.join("own");
    std::fs::create_dir_all(&own).unwrap();
    std::fs::write(own.join("inventory.cc"), &bytes).unwrap();
    assert_eq!(
        create(&own, &own.join("inventory.cc")).err(),
        Some(RunError::Io(std::io::ErrorKind::AlreadyExists))
    );
    assert_eq!(std::fs::read(own.join("inventory.cc")).unwrap(), bytes);

    // Any other file of that name in the root is refused untouched, too.
    let other = dao.root.join("other");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("inventory.cc"), b"not yours").unwrap();
    assert_eq!(
        create(&other, &dao.inventory).err(),
        Some(RunError::Io(std::io::ErrorKind::AlreadyExists))
    );
    assert_eq!(
        std::fs::read(other.join("inventory.cc")).unwrap(),
        b"not yours"
    );
    assert_eq!(std::fs::read(&dao.inventory).unwrap(), bytes);
}

#[test]
fn a_create_that_fails_leaves_no_inventory_and_the_retry_succeeds() {
    let dao = dao();
    let root = dao.root.join("retry");
    std::fs::create_dir_all(&root).unwrap();
    let create = |inventory: &Path, entities: &Path| {
        RunStores::create(
            &root,
            "x",
            RunPaths {
                corpus: &dao.corpus,
                inventory,
                entities,
            },
            "fake-model",
            "bag", weksa::run::LowererReach::ToolLessRelay,
        )
    };
    // A directory opens for reading and fails mid-copy, after the destination exists.
    let dir = dao.root.join("a-directory");
    std::fs::create_dir_all(&dir).unwrap();
    assert!(create(&dir, &dao.entities).is_err());
    assert!(!root.join("inventory.cc").exists());
    // A directory as the entities store fails after the inventory was copied.
    assert!(create(&dao.inventory, &dir).is_err());
    assert!(!root.join("inventory.cc").exists());
    assert!(create(&dao.inventory, &dao.entities).is_ok());
}

/// `weksa.loop_run` as it was before the run kept its own inventory copy: keys
/// 0 to 10.
#[derive(Clone, cultcache_rs::DatabaseEntry)]
#[cultcache(type = "weksa.loop_run")]
struct LegacyRun {
    #[cultcache(key = 0)]
    label: String,
    #[cultcache(key = 1)]
    corpus: String,
    #[cultcache(key = 2)]
    inventory: String,
    #[cultcache(key = 3)]
    entities: String,
    #[cultcache(key = 4)]
    corpus_digest: String,
    #[cultcache(key = 5)]
    entity_digest: String,
    #[cultcache(key = 6)]
    prompt_digest: String,
    #[cultcache(key = 7)]
    lowerer_digest: String,
    #[cultcache(key = 8)]
    model: String,
    #[cultcache(key = 9)]
    embedder: String,
    #[cultcache(key = 10)]
    chrf_version: String,
}

#[test]
fn a_run_from_before_the_copy_does_not_open() {
    // A run.cc without key 11 is a scratch run off git: its open fails Store.
    let w = world();
    let dir = w.dir.path().to_path_buf();
    let current = run_over(&dir).record().clone();
    let root = dir.join("run");
    std::fs::remove_file(root.join("run.cc")).unwrap();
    let mut cache = CultCache::new();
    cache.register_entry_type::<LegacyRun>().unwrap();
    cache
        .add_generic_backing_store(SingleFileMessagePackBackingStore::new(root.join("run.cc")))
        .unwrap();
    cache
        .put(
            "run:t",
            &LegacyRun {
                label: current.label,
                corpus: current.corpus,
                inventory: current.inventory,
                entities: current.entities,
                corpus_digest: current.corpus_digest,
                entity_digest: current.entity_digest,
                prompt_digest: current.prompt_digest,
                lowerer_digest: String::new(),
                model: current.model,
                embedder: current.embedder,
                chrf_version: current.chrf_version,
            },
        )
        .unwrap();
    assert_eq!(RunStores::open(&root).err(), Some(RunError::Store));
}

#[test]
fn a_store_changed_between_its_digest_and_its_open_is_drift() {
    let mut w = world();
    let dir = w.dir.path().to_path_buf();
    let root = dir.join("run");
    run_over(&dir);
    let more = |w: &mut common::World| {
        let item = common::item_in(
            "item:nibu:2",
            "Another line.",
            &common::group_in_bucket(10),
            common::cast(),
        );
        w.corpus.add(item, &w.entities).unwrap();
    };
    // The pins hold at the check; the corpus changes before the stores open.
    assert_eq!(
        RunStores::open_between(&root, || more(&mut w)).err(),
        Some(RunError::Drift(DriftOf::Corpus))
    );

    // The same window at create: nothing is pinned and the inventory copy and its
    // lock are taken away again, so the retry creates.
    let mut w = world();
    let dir = w.dir.path().to_path_buf();
    let root = dir.join("run");
    for name in ["c.cc", "i.cc", "entities.cc"] {
        assert!(dir.join(name).exists(), "{name}");
    }
    let (c, i, e) = (dir.join("c.cc"), dir.join("i.cc"), dir.join("entities.cc"));
    let paths = || weksa::run::RunPaths {
        corpus: &c,
        inventory: &i,
        entities: &e,
    };
    assert_eq!(
        RunStores::create_between(&root, "t", paths(), "fake-model", "bag", weksa::run::LowererReach::ToolLessRelay, || more(&mut w)).err(),
        Some(RunError::Drift(DriftOf::Corpus))
    );
    assert!(!root.join("inventory.cc").exists());
    // Not the copy and not the lock the store's open left beside it.
    assert!(!root.join("inventory.cc.lock").exists());
    assert_eq!(RunStores::open(&root).err(), Some(RunError::NoRun));
    RunStores::create(&root, "t", paths(), "fake-model", "bag", weksa::run::LowererReach::ToolLessRelay).unwrap();
}

/// Runs `change` in the window between the pins and the open, at `open` and at
/// `create`, and returns what each refused with plus what `create` left behind.
fn window(
    change: impl Fn(&mut common::World, &Path),
) -> (Option<RunError>, Option<RunError>, bool, Option<RunError>) {
    let mut w = world();
    let dir = w.dir.path().to_path_buf();
    let root = dir.join("run");
    run_over(&dir);
    let opened = RunStores::open_between(&root, || change(&mut w, &root)).err();

    let mut w = world();
    let dir = w.dir.path().to_path_buf();
    let root = dir.join("run");
    let (c, i, e) = (dir.join("c.cc"), dir.join("i.cc"), dir.join("entities.cc"));
    let paths = weksa::run::RunPaths {
        corpus: &c,
        inventory: &i,
        entities: &e,
    };
    let created = RunStores::create_between(&root, "t", paths, "fake-model", "bag", weksa::run::LowererReach::ToolLessRelay, || {
        change(&mut w, &root)
    })
    .err();
    let left = root.join("inventory.cc").exists() || root.join("inventory.cc.lock").exists();
    (opened, created, left, RunStores::open(&root).err())
}

#[test]
fn an_entity_store_changed_between_its_digest_and_its_open_is_drift() {
    let (opened, created, left, after) = window(|w, _| {
        w.entities.put(ent("kay", "en-US", &["Kay"])).unwrap();
    });
    assert_eq!(opened, Some(RunError::Drift(DriftOf::Entities)));
    assert_eq!(created, Some(RunError::Drift(DriftOf::Entities)));
    assert!(!left);
    assert_eq!(after, Some(RunError::NoRun));
}

#[test]
fn a_head_inventory_changed_between_its_digest_and_its_open_is_drift() {
    let (opened, created, left, after) = window(|_, root| {
        weksa::InventoryStore::open(&root.join("inventory.cc"))
            .unwrap()
            .admit_revision(
                weksa::InventoryRevision {
                    n: 5,
                    parent: Some(4),
                    changes: vec![],
                    frozen: false,
                },
                vec![],
            )
            .unwrap();
    });
    assert_eq!(opened, Some(RunError::Drift(DriftOf::Inventory)));
    assert_eq!(created, Some(RunError::Drift(DriftOf::Inventory)));
    assert!(!left);
    assert_eq!(after, Some(RunError::NoRun));
}

#[test]
fn source_digests_cover_their_files() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let read = |name: &str| std::fs::read(src.join(name)).unwrap();
    let digest = |name: &str| fnv1a64(&read(name));
    // The prompt's partition and names come from split.rs, and the lowering
    // prompt reads lower.rs's target language and entity port and prints
    // packet.rs's serde shapes: all four are covered.
    let (context, split, lower, packet) = (
        read("context.rs"),
        read("split.rs"),
        read("lower.rs"),
        read("packet.rs"),
    );
    // Each file enters behind its length, so the parts have one framing.
    let framed = [
        (context.len() as u64).to_le_bytes().as_slice(),
        &context,
        (split.len() as u64).to_le_bytes().as_slice(),
        &split,
        (lower.len() as u64).to_le_bytes().as_slice(),
        &lower,
        (packet.len() as u64).to_le_bytes().as_slice(),
        &packet,
    ]
    .concat();
    assert_eq!(weksa::run::PROMPT_SOURCE_DIGEST, fnv1a64(&framed));
    assert_ne!(
        weksa::run::PROMPT_SOURCE_DIGEST,
        fnv1a64(&[context.clone(), split.clone(), lower.clone(), packet.clone()].concat()),
        "bare concatenation is not the digest"
    );
    // Moving the boundary between the parts moves the digest.
    assert_ne!(
        weksa::split::fnv1a64_of(&[b"ab", b"c"]),
        weksa::split::fnv1a64_of(&[b"a", b"bc"])
    );
    assert_ne!(weksa::run::PROMPT_SOURCE_DIGEST, digest("context.rs"));
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

    // A reply or prompt file that cannot be read is an io error, never "unanswered".
    use std::os::unix::fs::PermissionsExt;
    let unreadable = |path: &Path| {
        std::fs::write(path, "x").unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o200)).unwrap();
    };
    unreadable(&answer);
    assert_eq!(
        files.answer(&p),
        Err(PortError::Io(ErrorKind::PermissionDenied))
    );
    std::fs::remove_file(&answer).unwrap();
    unreadable(&prompt_file);
    assert_eq!(
        files.answer(&p),
        Err(PortError::Io(ErrorKind::PermissionDenied))
    );
    std::fs::set_permissions(&prompt_file, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        std::fs::read_to_string(&prompt_file).unwrap(),
        "x",
        "an unreadable prompt file is not rewritten"
    );

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
use weksa::context::{ENCODE_INSTRUCTIONS, LOWER_INSTRUCTIONS, MINT_INSTRUCTIONS, Prompt};
use weksa::daodejing::packets;
use weksa::model::{AnswerFiles, ModelPort, PortError};
use weksa::run::{Outcome, RoundOutcome, RoundRecord, Stage, round};
use weksa::{CorpusStore, Embedder, InventoryStore, PacketDraft, PacketStore, Scorer};

/// Answers by the item the prompt holds; panics on an item it was not given,
/// and records every prompt it was shown. A prompt that starts with
/// `MINT_INSTRUCTIONS` is the editor's and is answered from `edit` (a panic when
/// none was given). With `candidate` set, an encode prompt that lists
/// `inventory rev:<that revision>` is answered from the candidate answers.
struct FakeModel {
    name: &'static str,
    answers: HashMap<String, String>,
    /// The lowering answered for an item: `{"text": lowered[item]}`; a panic on
    /// an item with none.
    lowered: HashMap<String, String>,
    /// When set, the answer to every lowering prompt instead of `lowered`.
    raw_lowering: Option<Result<String, PortError>>,
    candidate: Option<(u32, HashMap<String, String>)>,
    edit: Option<Result<String, PortError>>,
    seen: Vec<(String, String)>,
    mint_keys: Vec<Vec<String>>,
}

impl ModelPort for FakeModel {
    fn model(&self) -> &str {
        self.name
    }
    fn answer(&mut self, prompt: &Prompt) -> Result<String, PortError> {
        self.seen.push((prompt.digest(), prompt.text().to_string()));
        if prompt.text().starts_with(MINT_INSTRUCTIONS) {
            self.mint_keys.push(prompt.item_keys().to_vec());
            return self.edit.clone().expect("the model was asked for an edit");
        }
        let key = prompt.item_keys()[0].clone();
        if prompt.text().starts_with(LOWER_INSTRUCTIONS) {
            if let Some(raw) = &self.raw_lowering {
                return raw.clone();
            }
            let text = self
                .lowered
                .get(&key)
                .unwrap_or_else(|| panic!("the model was asked to lower {key}"));
            return Ok(serde_json::json!({ "text": text }).to_string());
        }
        let answers = match &self.candidate {
            Some((rev, candidate))
                if prompt
                    .text()
                    .contains(&format!("\n\ninventory rev:{rev}\n")) =>
            {
                candidate
            }
            _ => &self.answers,
        };
        match answers.get(&key) {
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

fn keyed(answers: &[(usize, String)]) -> HashMap<String, String> {
    answers
        .iter()
        .map(|(n, a)| (format!("item:daodejing:{n}"), a.clone()))
        .collect()
}

/// The five English texts the deleted template table gave the Dao packets
/// (`tests/daodejing.rs`): the lowering the fake model answers, so the score
/// pins hold.
fn table_english() -> HashMap<String, String> {
    [
        "The sage empties the minds. The sage fills the bellies. The sage weakens the wills. The sage strengthens the bones.",
        "The heaven is long-enduring. The earth continues long.",
        "The highest excellence is like the water.",
        "The words that are strictly true seem paradoxical.",
        "The space between heaven and earth may be compared to the bellows.",
    ]
    .iter()
    .enumerate()
    .map(|(i, text)| (format!("item:daodejing:{}", i + 1), (*text).to_string()))
    .collect()
}

/// A fake that answers the Dao packets for the Train and Validation items.
fn fake(answers: &[(usize, String)]) -> FakeModel {
    FakeModel {
        name: "fake-model",
        answers: keyed(answers),
        lowered: table_english(),
        raw_lowering: None,
        candidate: None,
        edit: None,
        seen: vec![],
        mint_keys: vec![],
    }
}

fn good_answers() -> Vec<(usize, String)> {
    (1..=3).map(|n| (n, json(&draft_of(n)))).collect()
}

struct Dao {
    _dir: tempfile::TempDir,
    root: PathBuf,
    corpus: PathBuf,
    inventory: PathBuf,
    entities: PathBuf,
    run: RunStores,
}

/// A run over copies of `data/daodejing/{corpus,inventory,entity}.cc`.
fn dao() -> Dao {
    dao_over(|_, _| {})
}

/// A Dao run whose stores `edit` (given the corpus and entity paths) changed
/// before the run pinned them.
fn dao_over(edit: impl FnOnce(&Path, &Path)) -> Dao {
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
    edit(&corpus, &entities);
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
        "bag", weksa::run::LowererReach::ToolLessRelay,
    )
    .unwrap();
    Dao {
        _dir: dir,
        root,
        corpus,
        inventory,
        entities,
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
            ["item:daodejing:3"],
            ["item:daodejing:1"],
            ["item:daodejing:2"],
            ["item:daodejing:3"]
        ],
        "the encode calls, then the lowering calls"
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
    assert_eq!((r.train_scored(), r.validation_scored()), (1, 2));

    // The record carries the run's pins.
    assert_eq!(r.prompt_digest(), pinned.prompt_digest);
    assert_eq!(r.corpus_digest(), pinned.corpus_digest);
    assert_eq!(r.entity_digest(), pinned.entity_digest);
    assert_eq!(r.model(), "fake-model");
    assert_eq!(r.embedder(), "bag");
    assert_eq!(r.chrf_version(), weksa::score::CHRF_VERSION);
    assert_eq!(r.inventory_digest(), pinned.inventory_digest);
    assert_eq!(r.cycle(), None, "a plain round belongs to no cycle");

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

/// The key and prompt of every lowering a round over `dao`'s run asks for when
/// every item encodes to its Dao packet and is admitted, as a round builds them.
fn lowering_prompts(dao: &Dao, inventory: &InventoryStore) -> Vec<(String, Prompt)> {
    let corpus = CorpusStore::open(&dao.corpus).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut store = PacketStore::open(&dir.path().join("p.cc")).unwrap();
    let inv = inventory.inventory().unwrap();
    (1..=3)
        .map(|n| {
            let mut packet = packets().remove(n - 1);
            // The encoder step stamps the inventory's latest revision.
            packet.inventory_rev = inv.latest().unwrap().n;
            let key = packet.provenance.corpus_item.clone();
            let view = store
                .admit(packet, inventory, &corpus, dao.run.entities())
                .unwrap();
            let prompt = Prompt::lower(&view, &key, &inv, dao.run.entities()).unwrap();
            (key.to_string(), prompt)
        })
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

    // The last encode answer moves the round to its second phase: the lowering
    // prompts are asked, and still nothing is written.
    let (key, p) = &expected[2];
    std::fs::write(round1.join(format!("{}.answer", p.digest())), &answers[key]).unwrap();
    let lowering = lowering_prompts(&dao, dao.run.inventory());
    let mut files = AnswerFiles::new(dao.run.next_round_dir().unwrap(), "fake-model".into());
    let RoundOutcome::Unanswered { prompts: digests } =
        round(&mut dao.run, &mut files, &Scorer::new(Bag)).unwrap()
    else {
        panic!("answered lowering from nothing")
    };
    let want_lower: Vec<String> = lowering.iter().map(|(_, p)| p.digest()).collect();
    assert_eq!(digests, want_lower);
    assert!(dao.run.rounds().unwrap().is_empty());
    assert!(!round1.join("packets.cc").exists());
    assert_eq!(std::fs::read(dao.root.join("run.cc")).unwrap(), run_cc);
    for (_, p) in &lowering {
        assert_eq!(
            std::fs::read_to_string(round1.join(format!("{}.prompt", p.digest()))).unwrap(),
            p.text()
        );
    }

    // Two of three lowerings is still nothing written; the third completes round 1.
    let table = table_english();
    let lowered = |key: &str| serde_json::json!({ "text": table[key] }).to_string();
    for (key, p) in &lowering[..2] {
        std::fs::write(round1.join(format!("{}.answer", p.digest())), lowered(key)).unwrap();
    }
    let mut files = AnswerFiles::new(dao.run.next_round_dir().unwrap(), "fake-model".into());
    let RoundOutcome::Unanswered { prompts: digests } =
        round(&mut dao.run, &mut files, &Scorer::new(Bag)).unwrap()
    else {
        panic!("answered from two of three lowerings")
    };
    assert_eq!(digests, [want_lower[2].clone()]);
    assert!(dao.run.rounds().unwrap().is_empty());
    assert!(!round1.join("packets.cc").exists());
    assert_eq!(std::fs::read(dao.root.join("run.cc")).unwrap(), run_cc);
    let (key, p) = &lowering[2];
    std::fs::write(round1.join(format!("{}.answer", p.digest())), lowered(key)).unwrap();
    let mut files = AnswerFiles::new(dao.run.next_round_dir().unwrap(), "fake-model".into());
    let r = recorded(round(&mut dao.run, &mut files, &Scorer::new(Bag)).unwrap());
    assert_eq!((r.n(), r.calls().len()), (1, 6));
    assert_eq!(two_dp(r.train_mean().chrf.unwrap()), 44.18);

    // Round 2 asks again, encode first.
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

    // Answered in both phases, round 2 is recorded as round 2, after round 1.
    for (key, p) in expected.iter().chain(&lowering) {
        let answer = answers.get(key).cloned().unwrap_or_else(|| lowered(key));
        std::fs::write(
            dao.root.join(format!("round-2/{}.answer", p.digest())),
            answer,
        )
        .unwrap();
    }
    let mut files = AnswerFiles::new(dao.run.next_round_dir().unwrap(), "fake-model".into());
    let second = recorded(round(&mut dao.run, &mut files, &Scorer::new(Bag)).unwrap());
    assert_eq!(second.n(), 2);
    let ns: Vec<u32> = dao.run.rounds().unwrap().iter().map(|r| r.n()).collect();
    assert_eq!(ns, [1, 2]);
    assert!(dao.root.join("round-2/packets.cc").is_file());
    assert_eq!(dao.run.next_round_dir().unwrap(), dao.root.join("round-3"));
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

    // Train refused at Parse; Validation refused at Admit, and a relation
    // lowers (the model renders relations; the deleted table did not).
    let r = run([
        junk.clone(),
        edited(2, unknown_concept),
        edited(3, with_relation),
    ]);
    assert_eq!(r.train()[0].outcome, Outcome::Refused(Stage::Parse));
    assert_eq!(r.train()[0].packet, None);
    // A refused item is in the mean as 0: n counts every item, scored the real ones.
    let m = r.train_mean();
    assert_eq!((m.count, m.chrf, m.meaning), (1, Some(0.0), Some(0.0)));
    assert_eq!((r.train_scored(), r.validation_scored()), (0, 1));
    assert_eq!(r.validation_refused().parse, 0);
    assert_eq!(
        (r.validation_refused().admit, r.validation_refused().lower),
        (1, 0)
    );
    let m = r.validation_mean();
    assert_eq!(m.count, 2);
    assert!(m.chrf.unwrap() > 0.0, "the relation item lowered and scored");

    // Validation items 2 and 3 score a and b when both lower (the Dao bed:
    // 82.12 and 74.99). Refusing one halves the mean; it cannot raise it.
    let full = run([
        good_answers()[0].1.clone(),
        good_answers()[1].1.clone(),
        good_answers()[2].1.clone(),
    ]);
    let b_only = run([
        edited(1, unknown_concept),
        junk.clone(),
        good_answers()[2].1.clone(),
    ]);
    assert_eq!(b_only.train()[0].outcome, Outcome::Refused(Stage::Admit));
    assert_eq!(
        b_only.train()[0].packet,
        None,
        "a refused packet is not stored"
    );
    let v = b_only.validation_refused();
    assert_eq!((v.parse, v.admit, v.lower), (1, 0, 0));
    assert_eq!(
        b_only.validation_mean().count,
        2,
        "every Validation item counts"
    );
    assert_eq!(b_only.validation_scored(), 1);
    let b_half = b_only.validation_mean().chrf.unwrap();
    assert!(
        (b_half - 74.99 / 2.0).abs() < 0.01,
        "(74.99 + 0) / 2 = {b_half}"
    );

    let a_only = run([good_answers()[0].1.clone(), good_answers()[1].1.clone(), junk]);
    assert_eq!(a_only.validation_refused().parse, 1);
    let a_half = a_only.validation_mean().chrf.unwrap();
    assert!(
        (a_half - 82.12 / 2.0).abs() < 0.01,
        "(82.12 + 0) / 2 = {a_half}"
    );
    // The halves add to the full mean: (a + 0) / 2 + (0 + b) / 2 = (a + b) / 2.
    let whole = full.validation_mean().chrf.unwrap();
    assert!(
        (a_half + b_half - whole).abs() < 1e-9,
        "{a_half} + {b_half} vs {whole}"
    );
    assert!(
        b_half < whole && a_half < whole,
        "refusing never raises the mean"
    );
    assert_eq!(full.validation_scored(), 2);
}

#[test]
fn non_utf8_encode_answer_is_a_parse_refusal() {
    let mut dao = dao();
    let expected = prompts(&dao.run);
    let answers: HashMap<String, String> = good_answers()
        .into_iter()
        .map(|(n, a)| (format!("item:daodejing:{n}"), a))
        .collect();
    let round1 = dao.root.join("round-1");
    std::fs::create_dir_all(&round1).unwrap();
    for (key, p) in &expected {
        let bytes = if key == "item:daodejing:3" {
            vec![0xff, 0xfe, 0xfd]
        } else {
            answers[key].clone().into_bytes()
        };
        std::fs::write(round1.join(format!("{}.answer", p.digest())), bytes).unwrap();
    }
    // Item 3 never reaches the second phase.
    let table = table_english();
    for (key, p) in lowering_prompts(&dao, dao.run.inventory())
        .iter()
        .filter(|(key, _)| key != "item:daodejing:3")
    {
        let answer = serde_json::json!({ "text": table[key] }).to_string();
        std::fs::write(round1.join(format!("{}.answer", p.digest())), answer).unwrap();
    }
    let mut files = AnswerFiles::new(round1.clone(), "fake-model".into());
    let r = recorded(round(&mut dao.run, &mut files, &Scorer::new(Bag)).unwrap());
    assert_eq!(r.calls().len(), 5);
    // Item 3 is Validation: its refusal is a Parse refusal and the round stands.
    let v = r.validation_refused();
    assert_eq!((v.parse, v.admit, v.lower), (1, 0, 0));
    assert_eq!((r.train_scored(), r.validation_scored()), (1, 1));
    assert_eq!(r.validation_mean().count, 2);
    assert_eq!(dao.run.rounds().unwrap(), [r]);
}

/// An embedder that is down.
struct Down;

impl Embedder for Down {
    fn model(&self) -> &str {
        "bag"
    }
    fn embed(&self, _: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        anyhow::bail!("down")
    }
}

#[test]
fn a_failed_round_leaves_nothing_a_rerun_collides_with() {
    let mut dao = dao();
    let mut model = fake(&good_answers());
    assert_eq!(
        round(&mut dao.run, &mut model, &Scorer::new(Down)).err(),
        Some(RunError::Embed)
    );
    assert!(dao.run.rounds().unwrap().is_empty());
    assert!(
        !dao.root.join("round-1/packets.cc").exists(),
        "the failed attempt's packets are discarded"
    );

    // The session revises item 1's answer and reruns: its packet is admitted
    // as the revised one, not refused as a changed packet under the same key.
    let mut revised = draft_of(1);
    revised.confidence = weksa::packet::Grade::new(123).unwrap();
    let mut answers = good_answers();
    answers[0].1 = json(&revised);
    let mut model = fake(&answers);
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    assert!(
        matches!(r.train()[0].outcome, Outcome::Scored { .. }),
        "{:?}",
        r.train()[0]
    );
    assert_eq!(r.validation_refused(), Default::default());
    let stored = PacketStore::open(&dao.root.join("round-1/packets.cc"))
        .unwrap()
        .all()
        .unwrap();
    assert_eq!(stored.len(), 3);
    assert!(stored.iter().any(|p| p.provenance.confidence.get() == 123));
}

#[test]
fn a_stale_packet_store_in_an_unrecorded_round_dir_is_discarded_at_start() {
    // A previous attempt that died after admitting (a crash, a kill) left its
    // packets and lock in the next round's directory.
    let mut first = dao();
    let mut model = fake(&good_answers());
    recorded(round(&mut first.run, &mut model, &Scorer::new(Bag)).unwrap());

    let mut dao = dao();
    let dir = dao.root.join("round-1");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(
        first.root.join("round-1/packets.cc"),
        dir.join("packets.cc"),
    )
    .unwrap();
    std::fs::write(dir.join("packets.cc.lock"), "stale").unwrap();

    // The session revised item 1's answer; the stale packet for it is the old one.
    let mut revised = draft_of(1);
    revised.confidence = weksa::packet::Grade::new(321).unwrap();
    let mut answers = good_answers();
    answers[0].1 = json(&revised);
    let mut model = fake(&answers);
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    assert!(
        matches!(r.train()[0].outcome, Outcome::Scored { .. }),
        "the revised draft is not refused as a changed packet: {:?}",
        r.train()[0]
    );
    assert_eq!(r.validation_refused(), Default::default());
    let stored = PacketStore::open(&dir.join("packets.cc"))
        .unwrap()
        .all()
        .unwrap();
    assert_eq!(stored.len(), 3);
    assert!(stored.iter().any(|p| p.provenance.confidence.get() == 321));
    assert!(
        !stored.iter().any(|p| p.provenance.confidence.get() == 600
            && p.provenance.corpus_item.as_str() == "item:daodejing:1"),
        "the stale packet is gone"
    );
    assert_ne!(
        std::fs::read(dir.join("packets.cc.lock")).unwrap_or_default(),
        b"stale",
        "the stale lock was removed"
    );
}

#[test]
fn the_round_line_prints_each_split_under_its_own_name() {
    let mut dao = dao();
    let mut model = fake(&good_answers());
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    let line = dao.run.round_line(&r);
    // Train is item 1 alone, Validation items 2 and 3; the numbers differ.
    let (t, v) = (r.train_mean(), r.validation_mean());
    assert_eq!(
        line,
        format!(
            "round 1 rev 3 train n=1 scored=1 chrf={:.2} meaning={:.4} refused=0 validation n=2 scored=2 chrf={:.2} meaning={:.4} refused parse=0 admit=0 lower=0 lowering=tool-less-relay",
            t.chrf.unwrap(),
            t.meaning.unwrap(),
            v.chrf.unwrap(),
            v.meaning.unwrap()
        )
    );
    assert!(
        line.contains("train n=1 scored=1 chrf=44.18 meaning=0."),
        "{line}"
    );
    assert!(
        line.contains("validation n=2 scored=2 chrf=78.56 meaning=0."),
        "{line}"
    );
    assert_ne!(t.meaning, v.meaning);
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
    let e = RunStores::create(&root, "ok", stores, "m", "bag", weksa::run::LowererReach::ToolLessRelay)
        .err()
        .unwrap();
    note(&e, &e);
    let e = RunStores::create(&root, &format!("{canary}!"), stores, "m", "bag", weksa::run::LowererReach::ToolLessRelay)
        .err()
        .unwrap();
    note(&e, &e);
    let e = RunStores::create(&root, "ok", stores, &format!("{canary}\n"), "bag", weksa::run::LowererReach::ToolLessRelay)
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
    // Cycle errors, from a canary root, and a decision over a canary edit answer.
    let canary_root = tmp.path().join("canaryrun");
    let mut canary_run = RunStores::create(
        &canary_root,
        "ok",
        RunPaths {
            corpus: &dao.corpus,
            inventory: &dao.inventory,
            entities: &dao.entities,
        },
        "fake-model",
        "bag", weksa::run::LowererReach::ToolLessRelay,
    )
    .unwrap();
    let mut model = cycle_fake(&format!("{{\"{canary}\":1}}"));
    let e = cycle(&mut canary_run, &mut model, &Scorer::new(Bag))
        .err()
        .unwrap();
    assert_eq!(e, RunError::NoFloor);
    all.extend([format!("{e:?}"), e.to_string()]);
    for _ in 0..2 {
        let mut m = fake(&degraded_answers());
        recorded(round(&mut canary_run, &mut m, &Scorer::new(Bag)).unwrap());
    }
    let d = decided(cycle(&mut canary_run, &mut model, &Scorer::new(Bag)).unwrap());
    assert_eq!(d.verdict(), Verdict::Rejected(Why::EditParse));
    all.extend([format!("{d:?}"), canary_run.cycle_line(&d)]);
    let mut non_utf8 = cycle_fake("");
    non_utf8.edit = Some(Err(PortError::Io(std::io::ErrorKind::InvalidData)));
    let d = decided(cycle(&mut canary_run, &mut non_utf8, &Scorer::new(Bag)).unwrap());
    all.extend([format!("{d:?}"), canary_run.cycle_line(&d)]);
    weksa::InventoryStore::open(&canary_root.join("inventory.cc"))
        .unwrap()
        .admit_revision(
            weksa::InventoryRevision {
                n: 4,
                parent: Some(3),
                changes: vec![],
                frozen: false,
            },
            vec![],
        )
        .unwrap();
    let e = RunStores::open(&canary_root).err().unwrap();
    assert_eq!(e, RunError::Drift(DriftOf::Inventory));
    all.extend([format!("{e:?}"), e.to_string()]);
    let e = RunError::NoValidation;
    all.extend([format!("{e:?}"), e.to_string()]);
    assert_eq!(
        format!("{:?}", RunError::NoFloor),
        RunError::NoFloor.to_string()
    );

    for text in &all {
        assert!(!text.to_uppercase().contains("CANARY"), "{text}");
    }
    assert!(all.len() >= 22);
    // Debug names the same kind Display does.
    assert_eq!(format!("{:?}", RunError::NoRun), "the root holds no run");
    assert_eq!(
        format!("{:?}", RunError::Drift(DriftOf::Inventory)),
        "drift in Inventory"
    );
    assert_eq!(
        format!("{:?}", RunError::Drift(DriftOf::Model)),
        "drift in Model"
    );
    // The Prompts text names the three files its digest covers.
    let prompts = RunError::Drift(DriftOf::Prompts).to_string();
    assert!(["context.rs", "split.rs", "lower.rs", "packet.rs"].iter().all(|f| prompts.contains(f)));
    assert_eq!(format!("{:?}", RunError::Drift(DriftOf::Prompts)), prompts);
}

#[test]
fn prompts_come_only_from_context() {
    let mut dao = dao();
    let mut model = fake(&good_answers());
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    assert_eq!(model.seen.len(), r.calls().len());
    assert_eq!(model.seen.len(), 6);
    for ((digest, text), call) in model.seen.iter().zip(r.calls()) {
        assert_eq!(call.items.len(), 1);
        if text.starts_with(ENCODE_INSTRUCTIONS) {
            assert_eq!(text.matches("[item:").count(), 1);
            assert!(text.contains(&format!("[{}]", call.items[0])));
        } else {
            // A lowering prompt holds no item block and prints no item key.
            assert!(text.starts_with(LOWER_INSTRUCTIONS));
            assert_eq!(text.matches("[item:").count(), 0);
            assert!(!text.contains("item:daodejing"));
        }
        assert_eq!(&call.prompt, digest);
        assert_eq!(*digest, format!("{:016x}", fnv1a64(text.as_bytes())));
    }
}

// ---- edit-accept cycles ----

use weksa::run::{CycleOutcome, Decision, LADDER_STEP, Reading, Verdict, Why, cycle};

const ONE_CONCEPT: &str =
    r#"{"add":[{"family":"Concept","gloss":"a general meaning the Dao names"}]}"#;

/// Item 2 refused at Admit (an unknown concept), items 1 and 3 good.
fn degraded_answers() -> Vec<(usize, String)> {
    vec![
        (1, json(&draft_of(1))),
        (2, edited(2, unknown_concept)),
        (3, json(&draft_of(3))),
    ]
}

/// Head answers degrade item 2; candidate answers (for `inventory rev:4`) are
/// the good Dao drafts; the editor answers `edit`.
fn cycle_fake(edit: &str) -> FakeModel {
    let mut model = fake(&degraded_answers());
    model.candidate = Some((4, keyed(&good_answers())));
    model.edit = Some(Ok(edit.to_string()));
    model
}

fn two_rounds(dao: &mut Dao) {
    for _ in 0..2 {
        let mut model = fake(&degraded_answers());
        recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    }
}

fn decided(outcome: CycleOutcome) -> Decision {
    match outcome {
        CycleOutcome::Decided(d) => d,
        CycleOutcome::Unanswered { prompts } => panic!("unanswered: {prompts:?}"),
    }
}

fn run_cycle(dao: &mut Dao, model: &mut FakeModel) -> Decision {
    decided(cycle(&mut dao.run, model, &Scorer::new(Bag)).unwrap())
}

fn head_rev(root: &Path) -> u32 {
    let run = RunStores::open(root).unwrap();
    let inv = run.inventory().inventory().unwrap();
    inv.latest().unwrap().n
}

fn validation(r: &RoundRecord) -> Reading {
    Reading::of(r.validation_mean()).unwrap()
}

/// The feedback line the editor is shown for a head round, built from outside.
fn feedback_line(last: &str, head: &RoundRecord) -> String {
    let rounded = (validation(head).composite / LADDER_STEP).round() * LADDER_STEP;
    format!("last edit: {last}; validation composite at head: {rounded:.3}")
}

fn mint_texts(model: &FakeModel) -> Vec<&str> {
    model
        .seen
        .iter()
        .map(|(_, t)| t.as_str())
        .filter(|t| t.starts_with(MINT_INSTRUCTIONS))
        .collect()
}

#[test]
fn cycle_accepts_a_gain_past_the_floor() {
    let mut dao = dao();
    two_rounds(&mut dao);
    let source = std::fs::read(&dao.inventory).unwrap();
    let rounds = dao.run.rounds().unwrap();
    assert_eq!(validation(&rounds[0]), validation(&rounds[1]), "floor 0");

    let mut model = cycle_fake(ONE_CONCEPT);
    let d = run_cycle(&mut dao, &mut model);
    assert_eq!(
        (d.k(), d.base_round(), d.candidate_round()),
        (1, 1, Some(3))
    );
    assert_eq!((d.base_rev(), d.candidate_rev()), (3, Some(4)));
    assert_eq!(d.verdict(), Verdict::Accepted);
    assert_eq!(d.threshold(), LADDER_STEP);
    assert_eq!(
        *d.floor(),
        Reading {
            chrf: 0.0,
            meaning: 0.0,
            composite: 0.0
        }
    );
    assert_eq!(*d.base(), validation(&rounds[0]));
    let record = dao.run.rounds().unwrap().remove(2);
    assert_eq!(d.candidate(), Some(&validation(&record)));
    assert!(d.candidate().unwrap().composite - d.base().composite > LADDER_STEP);

    // The candidate round was scored under the candidate file, as cycle 1's.
    let file = dao.root.join("cycle-1/inventory.cc");
    assert_eq!(
        (record.n(), record.inventory_rev(), record.cycle()),
        (3, 4, Some(1))
    );
    assert_eq!(record.inventory_digest(), hex(&file));
    assert_eq!(d.candidate_inventory_digest(), Some(hex(&file).as_str()));
    assert_eq!(
        d.base_inventory_digest(),
        hex(&dao.root.join("inventory.cc"))
    );
    assert_eq!(d.base_inventory_digest(), dao.run.record().inventory_digest);
    assert!(dao.root.join("cycle-1/packets.cc").is_file());
    assert_eq!(d.edit().items, ["item:daodejing:1"]);
    assert_eq!(
        d.edit_answer_digest(),
        Some(format!("{:016x}", fnv1a64(ONE_CONCEPT.as_bytes())).as_str())
    );
    assert_eq!(d.edit().prompt, model.seen[0].0);

    // The line, with the readings printed to four places.
    let (b, c) = (d.base().composite, d.candidate().unwrap().composite);
    assert_eq!(
        dao.run.cycle_line(&d),
        format!(
            "cycle 1 rev 3->4 base={b:.4} candidate={c:.4} floor=0.0000 threshold=0.0050 accepted lowering=tool-less-relay"
        )
    );

    // The decision made it the head: a reopened run reads rev 4 from the cycle's
    // file, and the caller's inventory file was never touched.
    assert_eq!(head_rev(&dao.root), 4);
    assert_eq!(std::fs::read(&dao.inventory).unwrap(), source);
    assert_eq!(dao.run.decisions().unwrap(), std::slice::from_ref(&d));
    assert_eq!(
        RunStores::open(&dao.root).unwrap().decisions().unwrap(),
        std::slice::from_ref(&d)
    );
    for (_, p) in prompts(&dao.run) {
        assert!(p.text().contains("\n\ninventory rev:4\n"));
    }

    // `loop round` reads the head and never moves it; the next cycle's base is
    // the establishing round 3, not the replicate recorded after it.
    let mut again = fake(&good_answers());
    let r = recorded(round(&mut dao.run, &mut again, &Scorer::new(Bag)).unwrap());
    assert_eq!((r.n(), r.inventory_rev(), r.cycle()), (4, 4, None));
    assert_eq!(r.inventory_digest(), hex(&file));
    let mut second = fake(&good_answers());
    second.candidate = Some((5, keyed(&good_answers())));
    second.edit = Some(Ok(ONE_CONCEPT.to_string()));
    let d2 = run_cycle(&mut dao, &mut second);
    assert_eq!((d2.k(), d2.base_round(), d2.base_rev()), (2, 3, 4));
    assert_eq!(d2.candidate_rev(), Some(5));
    assert_eq!(d2.candidate_round(), Some(5));
    assert_eq!(d2.verdict(), Verdict::Rejected(Why::NoGain));
    assert_eq!(*d2.floor(), *d.floor(), "the floor is measured once");
    assert_eq!(head_rev(&dao.root), 4, "a rejected edit moves nothing");
    assert!(
        mint_texts(&second)[0].contains(&feedback_line("accepted", &dao.run.rounds().unwrap()[2])),
        "{}",
        mint_texts(&second)[0]
    );
    assert!(dao.root.join("cycle-2/inventory.cc").is_file());
    assert_eq!(
        d2.base_inventory_digest(),
        d.candidate_inventory_digest().unwrap()
    );
}

#[test]
fn cycle_rejects_no_gain_and_keeps_the_head() {
    let mut dao = dao();
    two_rounds(&mut dao);
    let mut model = fake(&degraded_answers());
    model.candidate = Some((4, keyed(&degraded_answers())));
    model.edit = Some(Ok(ONE_CONCEPT.to_string()));
    let d = run_cycle(&mut dao, &mut model);
    assert_eq!(d.verdict(), Verdict::Rejected(Why::NoGain));
    assert_eq!((d.candidate_rev(), d.candidate_round()), (Some(4), Some(3)));
    assert_eq!(d.candidate(), Some(d.base()));
    let c = d.base().composite;
    assert_eq!(
        dao.run.cycle_line(&d),
        format!(
            "cycle 1 rev 3->4 base={c:.4} candidate={c:.4} floor=0.0000 threshold=0.0050 rejected no-gain lowering=tool-less-relay"
        )
    );
    assert_eq!(head_rev(&dao.root), 3);
    assert_eq!(
        RunStores::open(&dao.root)
            .unwrap()
            .record()
            .inventory_digest,
        hex(&dao.root.join("inventory.cc"))
    );
    assert_eq!(
        dao.run.rounds().unwrap().len(),
        3,
        "the candidate round is recorded"
    );

    // Cycle 2 goes to its own directory and is told the last edit was rejected,
    // with the same head composite.
    assert_eq!(dao.run.next_cycle_dir().unwrap(), dao.root.join("cycle-2"));
    let d2 = run_cycle(&mut dao, &mut model);
    assert_eq!((d2.k(), d2.base_round(), d2.base_rev()), (2, 1, 3));
    assert!(dao.root.join("cycle-2/inventory.cc").is_file());
    let mints = mint_texts(&model);
    assert_eq!(mints.len(), 2);
    let round1 = dao.run.rounds().unwrap().remove(0);
    assert!(mints[0].contains(&feedback_line("none", &round1)));
    assert!(mints[1].contains(&feedback_line("rejected", &round1)));
}

/// Item 2 and item 3 both refused at Admit: the weakest reading.
fn worst_answers() -> Vec<(usize, String)> {
    vec![
        (1, json(&draft_of(1))),
        (2, edited(2, unknown_concept)),
        (3, edited(3, unknown_concept)),
    ]
}

#[test]
fn distinct_replicates_set_the_floor_and_a_gain_inside_it_is_no_gain() {
    let mut dao = dao();
    // Replicate 1 is degraded, replicate 2 is good: the two readings differ.
    for answers in [degraded_answers(), good_answers()] {
        let mut m = fake(&answers);
        recorded(round(&mut dao.run, &mut m, &Scorer::new(Bag)).unwrap());
    }
    let rounds = dao.run.rounds().unwrap();
    let (a, b) = (validation(&rounds[0]), validation(&rounds[1]));
    let want = Reading {
        chrf: (a.chrf - b.chrf).abs(),
        meaning: (a.meaning - b.meaning).abs(),
        composite: (a.composite - b.composite).abs(),
    };
    assert!(want.chrf > 0.0 && want.meaning > 0.0 && want.composite > LADDER_STEP);

    // The candidate reads exactly as the good replicate: its gain over the head
    // (round 1) is the floor, which is past a step and inside the noise.
    let mut model = cycle_fake(ONE_CONCEPT);
    let d = run_cycle(&mut dao, &mut model);
    assert_eq!(*d.floor(), want);
    assert_eq!(d.threshold(), want.composite);
    assert_eq!(d.candidate(), Some(&b));
    assert!(d.candidate().unwrap().composite - d.base().composite > LADDER_STEP);
    assert_eq!(d.verdict(), Verdict::Rejected(Why::NoGain));
    assert_eq!(head_rev(&dao.root), 3);
}

#[test]
fn a_rejected_candidates_score_never_reaches_the_editor() {
    let mut dao = dao();
    two_rounds(&mut dao);
    let mut model = fake(&degraded_answers());
    model.candidate = Some((4, keyed(&worst_answers())));
    model.edit = Some(Ok(ONE_CONCEPT.to_string()));
    let d = run_cycle(&mut dao, &mut model);
    assert!(matches!(d.verdict(), Verdict::Rejected(Why::Regressed(_))));
    let rounds = dao.run.rounds().unwrap();
    let (head, candidate) = (&rounds[0], &rounds[2]);
    let ladder = |r: &RoundRecord| feedback_line("rejected", r);
    assert_ne!(
        ladder(head),
        ladder(candidate),
        "the candidate reads differently on the ladder"
    );

    let d2 = run_cycle(&mut dao, &mut model);
    assert_eq!(d2.base_round(), 1);
    let mints = mint_texts(&model);
    assert!(mints[1].contains(&ladder(head)), "{}", mints[1]);
    assert!(!mints[1].contains(&ladder(candidate)), "{}", mints[1]);
    let rounds = dao.run.rounds().unwrap();
    assert!(!mints[1].contains(&ladder(&rounds[3])), "{}", mints[1]);
}

#[test]
fn two_accepts_make_the_latest_the_head_and_a_reject_after_them_is_told_so() {
    let mut dao = dao();
    // Level 0 twice: a floor of 0 and a base reading of the weakest.
    for _ in 0..2 {
        let mut m = fake(&worst_answers());
        recorded(round(&mut dao.run, &mut m, &Scorer::new(Bag)).unwrap());
    }
    let mut model = fake(&worst_answers());
    let mut step = |dao: &mut Dao, rev: u32, answers: &[(usize, String)]| {
        model.candidate = Some((rev, keyed(answers)));
        model.edit = Some(Ok(ONE_CONCEPT.to_string()));
        run_cycle(dao, &mut model)
    };
    let d1 = step(&mut dao, 4, &degraded_answers());
    let d2 = step(&mut dao, 5, &good_answers());
    assert_eq!(d1.verdict(), Verdict::Accepted);
    assert_eq!(d2.verdict(), Verdict::Accepted);
    assert_eq!((d2.base_round(), d2.base_rev()), (3, 4));
    assert_eq!(
        (d2.candidate_round(), d2.candidate_rev()),
        (Some(4), Some(5))
    );

    // The head is the second accept, live and reopened.
    assert_eq!(head_rev(&dao.root), 5);
    assert_eq!(
        d2.candidate_inventory_digest(),
        Some(hex(&dao.root.join("cycle-2/inventory.cc")).as_str())
    );

    // Cycle 3 builds on cycle 2's file and round, gains nothing, and is rejected.
    let d3 = step(&mut dao, 6, &good_answers());
    assert_eq!((d3.k(), d3.base_round(), d3.base_rev()), (3, 4, 5));
    assert_eq!(
        d3.base_inventory_digest(),
        d2.candidate_inventory_digest().unwrap()
    );
    assert_eq!(d3.verdict(), Verdict::Rejected(Why::NoGain));
    assert_eq!(head_rev(&dao.root), 5, "a reject moves nothing");

    // Cycle 4 is told by the latest decision alone: cycle 3's reject, not cycle
    // 2's accept, and the composite at the head, which the reject did not move.
    let d4 = step(&mut dao, 6, &good_answers());
    assert_eq!((d4.k(), d4.base_round(), d4.base_rev()), (4, 4, 5));
    let rounds = dao.run.rounds().unwrap();
    let mints = mint_texts(&model);
    assert_eq!(mints.len(), 4);
    assert!(mints[0].contains(&feedback_line("none", &rounds[0])));
    assert!(mints[1].contains(&feedback_line("accepted", &rounds[2])));
    assert!(mints[2].contains(&feedback_line("accepted", &rounds[3])));
    assert!(
        mints[3].contains(&feedback_line("rejected", &rounds[3])),
        "{}",
        mints[3]
    );
    assert!(mints[1].contains("\n\ninventory rev:4\n"));
    assert!(mints[3].contains("\n\ninventory rev:5\n"));
}

#[test]
fn a_rejection_leaves_no_candidate_inventory_from_an_earlier_attempt() {
    for (name, rewrite, why) in [
        ("zero changes", "{}", Why::EditAdmit),
        ("not json", "not json", Why::EditParse),
    ] {
        let mut dao = dao();
        two_rounds(&mut dao);
        let scorer = Scorer::new(Bag);
        let ask = |dao: &mut Dao| {
            let mut files =
                AnswerFiles::new(dao.run.next_cycle_dir().unwrap(), "fake-model".into());
            cycle(&mut dao.run, &mut files, &scorer).unwrap()
        };
        let dir = dao.root.join("cycle-1");
        let CycleOutcome::Unanswered { prompts } = ask(&mut dao) else {
            panic!("{name}: answered from nothing")
        };
        let answer = dir.join(format!("{}.answer", prompts[0]));
        std::fs::write(&answer, ONE_CONCEPT).unwrap();

        // The candidate round is unanswered, which leaves the candidate inventory.
        let CycleOutcome::Unanswered { .. } = ask(&mut dao) else {
            panic!("{name}: answered the candidate round from nothing")
        };
        assert!(dir.join("inventory.cc").is_file(), "{name}");

        // The editor's answer is replaced by one the door refuses.
        std::fs::write(&answer, rewrite).unwrap();
        let d = decided(ask(&mut dao));
        assert_eq!(d.verdict(), Verdict::Rejected(why), "{name}");
        assert!(!dir.join("inventory.cc").exists(), "{name}");
        assert!(!dir.join("inventory.cc.lock").exists(), "{name}");
        assert_eq!(head_rev(&dao.root), 3, "{name}");
    }
}

#[cfg(unix)]
#[test]
fn a_failed_accept_write_leaves_neither_the_round_nor_the_decision() {
    use std::os::unix::fs::PermissionsExt;
    let mut dao = dao();
    two_rounds(&mut dao);
    let run_cc = std::fs::read(dao.root.join("run.cc")).unwrap();
    let mut model = cycle_fake(ONE_CONCEPT);
    // The cycle's directory exists, so everything but run.cc can still be
    // written once the run's root takes no new files: the batch's staging file
    // is refused, and it is the only write to run.cc a cycle makes.
    std::fs::create_dir_all(dao.root.join("cycle-1")).unwrap();
    let chmod = |mode| std::fs::set_permissions(&dao.root, std::fs::Permissions::from_mode(mode));
    chmod(0o500).unwrap();
    if std::fs::File::create(dao.root.join("probe")).is_ok() {
        // Running as a user the mode does not bind: the fault cannot be injected.
        std::fs::remove_file(dao.root.join("probe")).unwrap();
        chmod(0o700).unwrap();
        return;
    }
    let err = cycle(&mut dao.run, &mut model, &Scorer::new(Bag)).err();
    chmod(0o700).unwrap();
    assert_eq!(err, Some(RunError::Store));
    assert_eq!(std::fs::read(dao.root.join("run.cc")).unwrap(), run_cc);
    assert_eq!(dao.run.rounds().unwrap().len(), 2, "no orphan round");
    assert!(dao.run.decisions().unwrap().is_empty(), "no decision");
    let reopened = RunStores::open(&dao.root).unwrap();
    assert_eq!(reopened.rounds().unwrap().len(), 2);
    assert!(reopened.decisions().unwrap().is_empty());
    assert_eq!(head_rev(&dao.root), 3);

    // The rerun decides cycle 1 on round 3, as if the failure had not happened.
    let d = run_cycle(&mut dao, &mut model);
    assert_eq!((d.k(), d.candidate_round()), (1, Some(3)));
    assert_eq!(d.verdict(), Verdict::Accepted);
}

#[test]
fn unanswered_cycle_writes_nothing() {
    let mut dao = dao();
    two_rounds(&mut dao);
    let run_cc = std::fs::read(dao.root.join("run.cc")).unwrap();
    let scorer = Scorer::new(Bag);
    let ask = |dao: &mut Dao| {
        let mut files = AnswerFiles::new(dao.run.next_cycle_dir().unwrap(), "fake-model".into());
        cycle(&mut dao.run, &mut files, &scorer).unwrap()
    };
    let dir = dao.root.join("cycle-1");

    // The first call asks the editor and nothing else.
    let CycleOutcome::Unanswered { prompts: asked } = ask(&mut dao) else {
        panic!("answered from nothing")
    };
    assert_eq!(asked.len(), 1);
    let mint_file = dir.join(format!("{}.prompt", asked[0]));
    let mint = std::fs::read_to_string(&mint_file).unwrap();
    assert_eq!(format!("{:016x}", fnv1a64(mint.as_bytes())), asked[0]);
    assert!(mint.starts_with(MINT_INSTRUCTIONS));
    assert!(mint.contains("\n\nlast edit: none; validation composite at head: 0.435\n\n"));
    assert!(mint.contains("\n\ninventory rev:3\n"));
    assert_eq!(mint.matches("[item:").count(), 1, "one Train block");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    assert_eq!(std::fs::read(dao.root.join("run.cc")).unwrap(), run_cc);

    // With the edit answered, the candidate round asks three encode prompts at rev 4.
    std::fs::write(dir.join(format!("{}.answer", asked[0])), ONE_CONCEPT).unwrap();
    let CycleOutcome::Unanswered { prompts: encode } = ask(&mut dao) else {
        panic!("answered the candidate round from nothing")
    };
    assert_eq!(encode.len(), 3);
    let answers: HashMap<String, String> = keyed(&good_answers());
    for digest in &encode {
        let text = std::fs::read_to_string(dir.join(format!("{digest}.prompt"))).unwrap();
        assert!(text.contains("\n\ninventory rev:4\n"), "{text}");
        assert!(text.starts_with(ENCODE_INSTRUCTIONS));
        let key = text
            .split("[item:")
            .nth(1)
            .and_then(|t| t.split(']').next())
            .map(|k| format!("item:{k}"))
            .unwrap();
        std::fs::write(dir.join(format!("{digest}.answer")), &answers[&key]).unwrap();
    }
    assert_eq!(
        std::fs::read(dao.root.join("run.cc")).unwrap(),
        run_cc,
        "no round 3 and no decision"
    );
    assert_eq!(dao.run.rounds().unwrap().len(), 2);
    assert!(dao.run.decisions().unwrap().is_empty());

    // With the encodes answered, the candidate round asks its lowerings, listing
    // the candidate inventory; still nothing is written.
    let candidate = InventoryStore::open(&dir.join("inventory.cc")).unwrap();
    let lowering = lowering_prompts(&dao, &candidate);
    let CycleOutcome::Unanswered { prompts: lower } = ask(&mut dao) else {
        panic!("answered the candidate lowerings from nothing")
    };
    assert_eq!(
        lower,
        lowering.iter().map(|(_, p)| p.digest()).collect::<Vec<_>>()
    );
    assert!(lowering[0].1.text().contains("\n\ninventory rev:4\n"));
    assert!(!dir.join("packets.cc").exists());
    assert_eq!(std::fs::read(dao.root.join("run.cc")).unwrap(), run_cc);
    assert_eq!(dao.run.rounds().unwrap().len(), 2);
    assert!(dao.run.decisions().unwrap().is_empty());
    let table = table_english();
    for (key, p) in &lowering {
        let answer = serde_json::json!({ "text": table[key] }).to_string();
        std::fs::write(dir.join(format!("{}.answer", p.digest())), answer).unwrap();
    }

    // Answered, the cycle is decided; the next one uses cycle-2.
    let d = decided(ask(&mut dao));
    assert_eq!((d.k(), d.verdict()), (1, Verdict::Accepted));
    assert_eq!(dao.run.next_cycle_dir().unwrap(), dao.root.join("cycle-2"));
    let CycleOutcome::Unanswered { prompts: next } = ask(&mut dao) else {
        panic!("cycle 2 answered from cycle 1's files")
    };
    assert_eq!(next.len(), 1);
    assert!(
        dao.root
            .join(format!("cycle-2/{}.prompt", next[0]))
            .is_file()
    );
    assert_ne!(next[0], asked[0]);
}

#[test]
fn the_editor_sees_train_only() {
    run_the_editor_canary(CANARY_LOWERED);
}

/// What the lowerer says for the canary Validation item.
const CANARY_LOWERED: &str = "CANARYLOWERED";

#[test]
fn validation_lowerings_never_reach_the_editor() {
    // The same run under another lowering for the canary: the editor's prompts
    // never print either, and nothing recorded holds either.
    run_the_editor_canary("CANARYLOWERED");
    run_the_editor_canary("the zebra ate the quartz");
}

fn run_the_editor_canary(canary_lowered: &str) {
    let canary_key = "item:canary:1";
    let canary_text = "CANARYVALIDATIONTEXT";
    let mut dao = dao_over(|corpus, entities| {
        let entities = weksa::EntityStore::open(entities).unwrap();
        let mut store = weksa::CorpusStore::open(corpus).unwrap();
        let mut item = store.get("item:daodejing:2").unwrap().unwrap();
        item.id = canary_key.into();
        item.text = canary_text.into();
        item.names = vec![];
        item.prior_turns = vec![];
        store.add(item, &entities).unwrap();
    });
    let with_canary = |mut m: FakeModel| {
        m.answers.insert(canary_key.into(), json(&draft_of(2)));
        m.lowered.insert(canary_key.into(), canary_lowered.into());
        if let Some((_, c)) = m.candidate.as_mut() {
            c.insert(canary_key.into(), json(&draft_of(2)));
        }
        m
    };
    for _ in 0..2 {
        let mut m = with_canary(fake(&degraded_answers()));
        recorded(round(&mut dao.run, &mut m, &Scorer::new(Bag)).unwrap());
    }
    let mut model = fake(&degraded_answers());
    model.candidate = Some((4, keyed(&degraded_answers())));
    model.edit = Some(Ok(ONE_CONCEPT.to_string()));
    let mut model = with_canary(model);
    let d = run_cycle(&mut dao, &mut model);
    assert_eq!(d.verdict(), Verdict::Rejected(Why::NoGain));

    // A plain round with different answers changes the latest reading, not the
    // head's: the line the next cycle shows is the head round's.
    let mut other = with_canary(fake(&good_answers()));
    let r4 = recorded(round(&mut dao.run, &mut other, &Scorer::new(Bag)).unwrap());
    let rounds = dao.run.rounds().unwrap();
    assert_ne!(validation(&r4), validation(&rounds[0]));
    let d2 = run_cycle(&mut dao, &mut model);
    assert_eq!(d2.base_round(), 1);

    let corpus = weksa::CorpusStore::open(&dao.corpus).unwrap();
    let mut held_out: Vec<String> = vec![
        canary_key.into(),
        canary_text.into(),
        "canary".into(),
        canary_lowered.into(),
    ];
    for n in 2..=5 {
        let item = corpus.get(&format!("item:daodejing:{n}")).unwrap().unwrap();
        held_out.extend([item.id, item.text]);
    }
    let mints = mint_texts(&model);
    assert_eq!(mints.len(), 2);
    for text in &mints {
        assert!(text.starts_with(MINT_INSTRUCTIONS));
        for needle in &held_out {
            assert!(
                !text.to_lowercase().contains(&needle.to_lowercase()),
                "{needle}"
            );
        }
        assert_eq!(text.matches("[item:").count(), 1);
        assert!(text.contains("[item:daodejing:1]"));
    }
    assert_eq!(
        model.mint_keys,
        [["item:daodejing:1"], ["item:daodejing:1"]]
    );
    // The one number from the held-out split is the head composite on the ladder.
    let head = &rounds[0];
    assert!(mints[0].contains(&format!("\n\n{}\n\n", feedback_line("none", head))));
    assert!(mints[1].contains(&format!("\n\n{}\n\n", feedback_line("rejected", head))));
    assert_eq!(
        mints[0].replace("last edit: none;", "last edit: rejected;"),
        mints[1],
        "the two prompts differ by the last-edit word alone"
    );

    // Nothing the cycle writes or prints holds a held-out key or text.
    for text in [
        format!("{d:?}"),
        dao.run.cycle_line(&d),
        format!("{d2:?}"),
        dao.run.cycle_line(&d2),
    ] {
        for needle in &held_out {
            assert!(
                !text.to_lowercase().contains(&needle.to_lowercase()),
                "{needle}: {text}"
            );
        }
    }
    // Round records list the keys of the items they called, never lowered text.
    for text in [
        format!("{:?}", dao.run.rounds().unwrap()),
        format!("{:?}", dao.run.decisions().unwrap()),
    ] {
        for needle in [canary_lowered, canary_text] {
            assert!(!text.to_lowercase().contains(&needle.to_lowercase()));
        }
    }
    let run_cc = std::fs::read(dao.root.join("run.cc")).unwrap();
    for needle in [
        canary_text,
        canary_lowered,
        "item:daodejing:4",
        "item:daodejing:5",
    ] {
        assert!(
            !run_cc.windows(needle.len()).any(|w| w == needle.as_bytes()),
            "{needle}"
        );
    }
}

#[test]
fn edit_refusals_are_decisions() {
    use std::io::ErrorKind;
    let nine = format!(
        r#"{{"add":[{}]}}"#,
        [r#"{"family":"Concept","gloss":"g"}"#; 9].join(",")
    );
    let long = format!(
        r#"{{"add":[{{"family":"Concept","gloss":"{}"}}]}}"#,
        "x".repeat(121)
    );
    let cases: Vec<(&str, Result<String, PortError>, Why)> = vec![
        ("not json", Ok("not json at all".into()), Why::EditParse),
        (
            "non-utf8",
            Err(PortError::Io(ErrorKind::InvalidData)),
            Why::EditParse,
        ),
        (
            "an id in an entry",
            Ok(r#"{"add":[{"family":"Concept","gloss":"g","id":"concept:1"}]}"#.into()),
            Why::EditParse,
        ),
        ("no changes", Ok("{}".into()), Why::EditAdmit),
        ("nine changes", Ok(nine), Why::EditAdmit),
        (
            "retiring an absent id",
            Ok(r#"{"retire":["concept:9999"]}"#.into()),
            Why::EditAdmit,
        ),
        ("a 121-byte gloss", Ok(long), Why::EditAdmit),
        (
            "a control character in a gloss",
            Ok("{\"add\":[{\"family\":\"Concept\",\"gloss\":\"a\\u0007b\"}]}".into()),
            Why::EditAdmit,
        ),
    ];
    for (name, edit, why) in cases {
        let mut dao = dao();
        two_rounds(&mut dao);
        let mut model = cycle_fake("");
        model.edit = Some(edit.clone());
        let d = run_cycle(&mut dao, &mut model);
        assert_eq!(d.verdict(), Verdict::Rejected(why), "{name}");
        assert_eq!(
            (d.candidate_rev(), d.candidate_round(), d.candidate()),
            (None, None, None),
            "{name}"
        );
        assert_eq!(d.candidate_inventory_digest(), None, "{name}");
        let want = match &edit {
            Ok(a) => Some(format!("{:016x}", fnv1a64(a.as_bytes()))),
            Err(_) => None,
        };
        assert_eq!(d.edit_answer_digest(), want.as_deref(), "{name}");
        assert_eq!(
            dao.run.rounds().unwrap().len(),
            2,
            "{name}: no candidate round"
        );
        assert_eq!(model.seen.len(), 1, "{name}: no encode prompt was asked");
        let dir = dao.root.join("cycle-1");
        assert!(!dir.join("inventory.cc").exists(), "{name}");
        assert!(!dir.join("inventory.cc.lock").exists(), "{name}");
        assert_eq!(head_rev(&dao.root), 3, "{name}");
        let tag = if why == Why::EditParse {
            "edit-parse"
        } else {
            "edit-admit"
        };
        assert_eq!(
            dao.run.cycle_line(&d),
            format!(
                "cycle 1 rev 3->none base={:.4} candidate=none floor=0.0000 threshold=0.0050 rejected {tag} lowering=tool-less-relay",
                d.base().composite
            ),
            "{name}"
        );
        assert_eq!(dao.run.decisions().unwrap(), [d], "{name}");

        // The next cycle is told the last edit was rejected and asks afresh, in
        // a directory of its own.
        assert_eq!(dao.run.next_cycle_dir().unwrap(), dao.root.join("cycle-2"));
        model.edit = Some(Ok(ONE_CONCEPT.into()));
        let d2 = run_cycle(&mut dao, &mut model);
        assert_eq!(d2.k(), 2, "{name}");
        assert!(
            mint_texts(&model)[1].contains("last edit: rejected;"),
            "{name}"
        );
    }
}

#[test]
fn cycle_needs_a_floor() {
    let none = |dao: &mut Dao, model: &mut FakeModel| {
        let before = std::fs::read(dao.root.join("run.cc")).unwrap();
        let err = cycle(&mut dao.run, model, &Scorer::new(Bag)).err();
        assert_eq!(std::fs::read(dao.root.join("run.cc")).unwrap(), before);
        assert!(dao.run.decisions().unwrap().is_empty());
        assert!(!dao.root.join("cycle-1").exists());
        assert!(model.seen.is_empty(), "no prompt before the floor");
        err
    };
    let mut dao = dao();
    let mut model = cycle_fake(ONE_CONCEPT);
    assert_eq!(none(&mut dao, &mut model), Some(RunError::NoFloor));
    let mut one = fake(&degraded_answers());
    recorded(round(&mut dao.run, &mut one, &Scorer::new(Bag)).unwrap());
    assert_eq!(none(&mut dao, &mut model), Some(RunError::NoFloor));
    assert_eq!(
        RunError::NoFloor.to_string(),
        "the run has no noise floor: record two rounds first"
    );

    // Rounds whose Validation items are all withdrawn have no reading.
    let mut dao = dao_over(|corpus, _| {
        let mut store = weksa::CorpusStore::open(corpus).unwrap();
        for n in [2, 3] {
            store.withdraw(&format!("item:daodejing:{n}")).unwrap();
        }
    });
    for _ in 0..2 {
        let mut m = fake(&[(1, json(&draft_of(1)))]);
        let r = recorded(round(&mut dao.run, &mut m, &Scorer::new(Bag)).unwrap());
        assert_eq!(r.validation_mean().count, 0);
    }
    let mut model = cycle_fake(ONE_CONCEPT);
    assert_eq!(none(&mut dao, &mut model), Some(RunError::NoValidation));
    assert_eq!(
        RunError::NoValidation.to_string(),
        "a reading has no validation item"
    );
}

#[test]
fn a_failed_candidate_round_writes_nothing() {
    let mut dao = dao();
    two_rounds(&mut dao);
    let run_cc = std::fs::read(dao.root.join("run.cc")).unwrap();
    let mut model = cycle_fake(ONE_CONCEPT);
    assert_eq!(
        cycle(&mut dao.run, &mut model, &Scorer::new(Down)).err(),
        Some(RunError::Embed)
    );
    assert_eq!(std::fs::read(dao.root.join("run.cc")).unwrap(), run_cc);
    assert_eq!(dao.run.rounds().unwrap().len(), 2);
    assert!(dao.run.decisions().unwrap().is_empty());
    assert!(!dao.root.join("cycle-1/packets.cc").exists());
    assert_eq!(head_rev(&dao.root), 3);

    // The rerun starts clean from the head and decides on round 3.
    let d = run_cycle(&mut dao, &mut model);
    assert_eq!((d.k(), d.candidate_round()), (1, Some(3)));
    assert_eq!(d.verdict(), Verdict::Accepted);
}

#[test]
fn a_changed_head_is_drift() {
    let revision = |rev: u32| weksa::InventoryRevision {
        n: rev + 1,
        parent: Some(rev),
        changes: vec![],
        frozen: false,
    };
    let append = |file: &Path, rev: u32| {
        weksa::InventoryStore::open(file)
            .unwrap()
            .admit_revision(revision(rev), vec![])
            .unwrap();
    };
    // Before any accept the head is the run's copy.
    let mut dao = dao();
    append(&dao.root.join("inventory.cc"), 3);
    assert_eq!(
        RunStores::open(&dao.root).err(),
        Some(RunError::Drift(DriftOf::Inventory))
    );

    // After an accept it is the cycle's file; the first copy is no longer read.
    let mut dao2 = self::dao();
    two_rounds(&mut dao2);
    let mut model = cycle_fake(ONE_CONCEPT);
    assert_eq!(
        run_cycle(&mut dao2, &mut model).verdict(),
        Verdict::Accepted
    );
    drop(dao2.run);
    assert!(RunStores::open(&dao2.root).is_ok());
    append(&dao2.root.join("inventory.cc"), 3);
    assert!(
        RunStores::open(&dao2.root).is_ok(),
        "the superseded copy is not the head"
    );
    append(&dao2.root.join("cycle-1/inventory.cc"), 4);
    assert_eq!(
        RunStores::open(&dao2.root).err(),
        Some(RunError::Drift(DriftOf::Inventory))
    );
    dao.run = RunStores::create(
        &dao.root.join("again"),
        "x",
        RunPaths {
            corpus: &dao.corpus,
            inventory: &dao.inventory,
            entities: &dao.entities,
        },
        "fake-model",
        "bag", weksa::run::LowererReach::ToolLessRelay,
    )
    .unwrap();
    assert_eq!(
        dao.run.record().inventory,
        std::fs::canonicalize(&dao.inventory)
            .unwrap()
            .to_str()
            .unwrap()
    );
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
    // The editor's prompt reads Train items, an inventory and an EditFeedback:
    // no round record, decision, held-out mean or refusal count is named there.
    for banned in [
        "RoundRecord",
        "Decision",
        "SplitMean",
        "Aggregate",
        "validation_mean",
        "validation_refused",
    ] {
        assert!(!context.contains(banned), "context.rs names {banned}");
    }
    // `EditFeedback::none` is the one public constructor; Decision has none.
    assert!(!context.contains("pub fn new"));
    let run = read("run.rs");
    let code: Vec<&str> = run
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect();
    let count = |needle: &str| code.iter().filter(|l| l.contains(needle)).count();
    assert!(!run.contains("pub fn new"));
    // The round does not call the template table: the model lowers.
    assert_eq!(count("lower(&"), 0, "run.rs calls the table");
    assert_eq!(count("crate::lower::lower"), 0, "run.rs imports the table");
    assert_eq!(count("Prompt::lower("), 1, "one lowering call site");
    // Prompt::lower takes the view, the item's key, the inventory and the entity
    // port: no corpus item, train set, offer or store.
    let start = context.find("pub fn lower(").unwrap();
    let signature = &context[start..start + context[start..].find(") -> ").unwrap()];
    for banned in ["CorpusItem,", "CorpusItem)", "TrainItems", "EncoderOffer", "Store"] {
        assert!(!signature.contains(banned), "Prompt::lower takes {banned}");
    }
    assert!(signature.contains("&LoweringInput") && signature.contains("&CorpusItemId"));
    assert_eq!(count("admit_revision("), 1, "one door call");
    assert_eq!(
        count("std::fs::copy"),
        1,
        "the cycle's copy; create's is copy_new"
    );
    assert_eq!(
        count("Path::new(&record.inventory)"),
        1,
        "the source path is read once"
    );
    assert_eq!(
        count("InventoryStore::open"),
        2,
        "the head and the cycle's file"
    );
    // An entry draft carries no id or revision field.
    let encode = read("encode.rs");
    let entry = encode
        .split("pub struct EntryDraft {")
        .nth(1)
        .and_then(|t| t.split('}').next())
        .unwrap();
    for banned in ["id", "introduced_rev", "retired_rev"] {
        assert!(
            !entry
                .split_whitespace()
                .any(|w| w.trim_end_matches(':') == banned),
            "EntryDraft has a {banned} field"
        );
    }
}

// ---- the lowerer ----

/// Lowercase letters and digits only: what survives any re-spacing or re-casing.
fn squashed(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[test]
fn the_lowerer_sees_no_item_text() {
    let canary_key = "item:canary:1";
    let mut dao = dao_over(|corpus, entities| {
        let entities = weksa::EntityStore::open(entities).unwrap();
        let mut store = weksa::CorpusStore::open(corpus).unwrap();
        let mut item = store.get("item:daodejing:2").unwrap().unwrap();
        item.id = canary_key.into();
        item.text = "CANARYVALIDATIONTEXT of the zebra".into();
        item.names = vec![];
        item.prior_turns = vec![];
        store.add(item, &entities).unwrap();
    });
    let mut model = fake(&good_answers());
    model.answers.insert(canary_key.into(), json(&draft_of(2)));
    model.lowered.insert(canary_key.into(), "CANARYLOWERED".into());
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());

    let corpus = weksa::CorpusStore::open(&dao.corpus).unwrap();
    let items = corpus.all().unwrap();
    assert!(items.len() >= 6, "the canary and the five Legge items");
    let packet_keys: Vec<String> = PacketStore::open(&dao.root.join("round-1/packets.cc"))
        .unwrap()
        .all()
        .unwrap()
        .iter()
        .map(|p| p.id.to_string())
        .collect();
    assert_eq!(packet_keys.len(), 4);

    let lowering: Vec<&(String, String)> = model
        .seen
        .iter()
        .filter(|(_, t)| t.starts_with(LOWER_INSTRUCTIONS))
        .collect();
    assert_eq!(lowering.len(), 4, "one lowering per admitted packet");
    for (digest, text) in &lowering {
        let flat = squashed(text);
        for item in &items {
            // Any split: Train, Validation and Test text alike.
            assert!(!text.contains(&item.text), "{}", item.id);
            assert!(!flat.contains(&squashed(&item.text)), "{}", item.id);
            for needle in [item.id.as_str(), item.group.as_str(), item.speaker.as_str()] {
                assert!(!text.contains(needle), "{needle}");
            }
        }
        for key in &packet_keys {
            assert!(!text.contains(key.as_str()), "{key}");
        }
        assert!(!text.to_lowercase().contains("canary"));
        assert!(!text.contains("confidence"));
        // Its one call is its item.
        let call = r.calls().iter().find(|c| &c.prompt == digest).unwrap();
        assert_eq!(call.items.len(), 1);
        assert!(call.items[0].starts_with("item:"));
    }
    // The encode and lowering calls name the same four items.
    let keys = |lowering: bool| {
        let mut keys: Vec<&str> = r
            .calls()
            .iter()
            .filter(|c| {
                lowering
                    == model.seen.iter().any(|(d, t)| {
                        d == &c.prompt && t.starts_with(LOWER_INSTRUCTIONS)
                    })
            })
            .map(|c| c.items[0].as_str())
            .collect();
        keys.sort_unstable();
        keys
    };
    assert_eq!(keys(true), keys(false));
}

#[test]
fn lowering_refusals_count_at_lower() {
    let run = |raw: Result<String, PortError>| {
        let mut dao = dao();
        let mut model = fake(&good_answers());
        model.raw_lowering = Some(raw);
        recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap())
    };
    for (what, raw) in [
        ("non-JSON", Ok("The sage empties the minds.".to_string())),
        ("unknown key", Ok(r#"{"text":"a","trace":"b"}"#.to_string())),
        ("missing text", Ok("{}".to_string())),
        ("empty text", Ok(r#"{"text":""}"#.to_string())),
        ("blank text", Ok("{\"text\":\" \\n \"}".to_string())),
        ("non-UTF-8", Err(PortError::Io(std::io::ErrorKind::InvalidData))),
        ("NUL", Ok(serde_json::json!({ "text": "The sage\u{0}acts." }).to_string())),
        ("bell", Ok(serde_json::json!({ "text": "The sage \u{7}acts." }).to_string())),
        ("carriage return", Ok(serde_json::json!({ "text": "The sage\racts." }).to_string())),
        (
            "over length in bytes, within it in chars",
            Ok(serde_json::json!({ "text": "\u{e9}".repeat(weksa::encode::MAX_LOWERING_BYTES / 2 + 1) })
                .to_string()),
        ),
        (
            "over length",
            Ok(serde_json::json!({ "text": "a".repeat(weksa::encode::MAX_LOWERING_BYTES + 1) })
                .to_string()),
        ),
    ] {
        let r = run(raw);
        assert_eq!(r.train()[0].outcome, Outcome::Refused(Stage::Lower), "{what}");
        assert!(
            r.train()[0].packet.is_some(),
            "{what}: the admitted packet is kept on the Train row"
        );
        let v = r.validation_refused();
        assert_eq!((v.parse, v.admit, v.lower), (0, 0, 2), "{what}");
        assert_eq!((r.train_scored(), r.validation_scored()), (0, 0), "{what}");
        let m = r.validation_mean();
        assert_eq!((m.count, m.chrf), (2, Some(0.0)), "{what}");
    }
}

#[test]
fn an_added_entry_can_be_lowered() {
    // The edit adds a concept; the candidate encode answer cites it. The
    // candidate's lowering prompt lists it and the item scores: the gain path a
    // table with no row for it blocked.
    let mut dao = dao();
    two_rounds(&mut dao);
    let mut cited = draft_of(1);
    cited.referents[0].concept = "concept:28".parse().unwrap();
    let mut model = cycle_fake(ONE_CONCEPT);
    model
        .candidate
        .as_mut()
        .unwrap()
        .1
        .insert("item:daodejing:1".into(), json(&cited));
    run_cycle(&mut dao, &mut model);
    let candidate_lowerings: Vec<&str> = model
        .seen
        .iter()
        .map(|(_, t)| t.as_str())
        .filter(|t| t.starts_with(LOWER_INSTRUCTIONS) && t.contains("\n\ninventory rev:4\n"))
        .collect();
    assert_eq!(candidate_lowerings.len(), 3);
    for text in &candidate_lowerings {
        assert!(text.contains("\nconcept:28: a general meaning the Dao names"));
    }
    assert_eq!(
        candidate_lowerings
            .iter()
            .filter(|t| t.contains(r#""concept":"concept:28""#))
            .count(),
        1,
        "exactly the item that cites the new concept"
    );
    let rounds = dao.run.rounds().unwrap();
    let candidate = rounds.last().unwrap();
    assert_eq!(candidate.inventory_rev(), 4);
    assert!(matches!(
        candidate.train()[0].outcome,
        Outcome::Scored { .. }
    ));
}

#[test]
fn a_summary_line_says_how_lowerings_were_answered() {
    use weksa::run::LowererReach;
    assert_eq!("briefed-subagent".parse(), Ok(LowererReach::BriefedSubagent));
    assert_eq!("tool-less-relay".parse(), Ok(LowererReach::ToolLessRelay));
    assert!("Tool-Less-Relay".parse::<LowererReach>().is_err());
    let dao = dao();
    assert_eq!(
        dao.run.labelled("round 1"),
        "round 1 lowering=tool-less-relay"
    );
    // A briefed-subagent run is pinned as one, in the record that reopens, and
    // its lines say its numbers are smoke-round only.
    let root = dao.root.join("briefed");
    let paths = RunPaths {
        corpus: &dao.corpus,
        inventory: &dao.inventory,
        entities: &dao.entities,
    };
    RunStores::create(&root, "b", paths, "fake-model", "bag", LowererReach::BriefedSubagent)
        .unwrap();
    let run = RunStores::open(&root).unwrap();
    assert_eq!(run.record().lowerer_reach, LowererReach::BriefedSubagent);
    assert_eq!(
        run.labelled("cycle 1"),
        "cycle 1 lowering=briefed-subagent (smoke-round only)"
    );
}

#[test]
fn lowering_text_at_the_bound_is_scored_and_line_breaks_are_text() {
    for text in [
        "a".repeat(weksa::encode::MAX_LOWERING_BYTES),
        "\u{e9}".repeat(weksa::encode::MAX_LOWERING_BYTES / 2),
        "The sage acts.\n\tThe sage rests.".to_string(),
    ] {
        let mut dao = dao();
        let mut model = fake(&good_answers());
        model.raw_lowering = Some(Ok(serde_json::json!({ "text": text }).to_string()));
        let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
        assert_eq!(
            (r.train_scored(), r.validation_scored()),
            (1, 2),
            "{}",
            text.len()
        );
        assert_eq!(r.validation_refused(), Default::default());
    }
}

/// Two canary items, one Train and one Validation, whose speaker is an entity
/// with forms only in another language; each encodes to a packet citing it.
#[test]
fn a_lowering_prompt_that_cannot_be_built_refuses_at_lower_and_keeps_the_packet() {
    use weksa::entity::{Entity, EntityOrigin, LanguageForms};
    use weksa::packet::EntityId;
    let id = EntityId::parse("entity:sagexx").unwrap();
    let mut dao = dao_over(|corpus, entities| {
        let mut entity_store = weksa::EntityStore::open(entities).unwrap();
        entity_store
            .put(Entity {
                id: EntityId::parse("entity:sagexx").unwrap(),
                forms: vec![LanguageForms {
                    language: "sa-Latn".into(),
                    forms: vec!["Sagexa".into()],
                    pronunciation: None,
                }],
                origin: EntityOrigin::Cast {
                    persona: "sage".into(),
                    fixture: "f.yaml".into(),
                },
            })
            .unwrap();
        let mut store = weksa::CorpusStore::open(corpus).unwrap();
        for (from, key) in [
            ("item:daodejing:1", "item:cite:1"),
            ("item:daodejing:2", "item:cite:2"),
        ] {
            let mut item = store.get(from).unwrap().unwrap();
            item.id = key.into();
            item.text = format!("A held-apart sentence for {key}.");
            item.speaker = EntityId::parse("entity:sagexx").unwrap();
            item.names = vec![];
            item.prior_turns = vec![];
            store.add(item, &entity_store).unwrap();
        }
    });
    let citing = |n: usize| {
        let mut d = draft_of(n);
        d.referents[0].entity = Some(id.clone());
        json(&d)
    };
    let mut model = fake(&good_answers());
    model.answers.insert("item:cite:1".into(), citing(1));
    model.answers.insert("item:cite:2".into(), citing(2));
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    // Admitted, so the packet is on the Train row; refused at Lower, so no score.
    let row = r.train().iter().find(|row| row.item == "item:cite:1").unwrap();
    assert_eq!(row.outcome, Outcome::Refused(Stage::Lower));
    let kept = row.packet.as_deref().expect("the admitted packet's key is kept");
    let packets = PacketStore::open(&dao.root.join("round-1/packets.cc")).unwrap();
    assert!(packets.get(kept).unwrap().is_some());
    let v = r.validation_refused();
    assert_eq!((v.parse, v.admit, v.lower), (0, 0, 1));
    assert_eq!((r.train_scored(), r.validation_scored()), (1, 2));
    // Five items encoded, three lowering prompts built and asked.
    assert_eq!(r.calls().len(), 5 + 3);
}

#[test]
fn a_port_failure_in_the_lowering_phase_discards_the_packets() {
    let mut dao = dao();
    let mut model = fake(&good_answers());
    model.raw_lowering = Some(Err(PortError::Io(std::io::ErrorKind::PermissionDenied)));
    assert_eq!(
        round(&mut dao.run, &mut model, &Scorer::new(Bag)).err(),
        Some(RunError::Io(std::io::ErrorKind::PermissionDenied))
    );
    assert!(!dao.root.join("round-1/packets.cc").exists());
    assert!(dao.run.rounds().unwrap().is_empty());
}

#[test]
fn views_with_one_meaning_share_one_unanswered_lowering_prompt() {
    let mut dao = dao();
    // Items 1 and 3 encode to the same meaning; item 2 differs.
    let same = json(&draft_of(1));
    let expected = prompts(&dao.run);
    let round1 = dao.root.join("round-1");
    std::fs::create_dir_all(&round1).unwrap();
    for (key, p) in &expected {
        let answer = match key.as_str() {
            "item:daodejing:2" => json(&draft_of(2)),
            _ => same.clone(),
        };
        std::fs::write(round1.join(format!("{}.answer", p.digest())), answer).unwrap();
    }
    let mut files = AnswerFiles::new(round1.clone(), "fake-model".into());
    let RoundOutcome::Unanswered { prompts: asked } =
        round(&mut dao.run, &mut files, &Scorer::new(Bag)).unwrap()
    else {
        panic!("answered lowering from nothing")
    };
    assert_eq!(asked.len(), 2, "{asked:?}");
    let mut unique = asked.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 2);
}

/// Several refusals under one rule in one packet: the item still counts once.
fn every_concept_unknown(d: &mut PacketDraft) {
    for r in &mut d.referents {
        r.concept = "concept:999".parse().unwrap();
    }
    for p in &mut d.predications {
        p.concept = "concept:999".parse().unwrap();
    }
}

#[test]
fn admit_refusals_are_counted_by_rule_and_printed_with_the_label() {
    use weksa::packet::EntityId;
    let mut dao = dao();
    let empty = |d: &mut PacketDraft| {
        d.referents.clear();
        d.predications.clear();
        d.relations.clear();
    };
    let nobody = |d: &mut PacketDraft| {
        d.referents[0].entity = Some("entity:nobody".parse::<EntityId>().unwrap());
    };
    let mut model = fake(&[
        (1, edited(1, nobody)),
        (2, edited(2, every_concept_unknown)),
        (3, edited(3, empty)),
    ]);
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    let rules: Vec<(&str, u32)> = r
        .admit_rules()
        .iter()
        .map(|c| (c.rule.as_str(), c.count))
        .collect();
    assert_eq!(rules, [("UnknownEntity", 1), ("V2", 1), ("V5", 1)]);
    let line = dao.run.round_line(&r);
    assert!(
        line.ends_with("admit-rules=UnknownEntityx1,V2x1,V5x1 lowering=tool-less-relay"),
        "{line}"
    );

    // A round nothing refused names no rule and prints none.
    let mut dao = self::dao();
    let mut model = fake(&good_answers());
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    assert!(r.admit_rules().is_empty());
    assert!(!dao.run.round_line(&r).contains("admit-rules"));
}

/// Answers a recall probe from the corpus: the item's own second half, a
/// fixed wrong text, or junk.
struct Reciter {
    corpus: weksa::store::CorpusStore,
    answer: fn(&str) -> String,
    /// An item whose text holds this gets an answer file that is not UTF-8.
    unreadable: Option<&'static str>,
    seen: Vec<String>,
}

impl ModelPort for Reciter {
    fn model(&self) -> &str {
        "reciter"
    }
    fn answer(&mut self, prompt: &Prompt) -> Result<String, PortError> {
        self.seen.push(prompt.text().to_string());
        let key = &prompt.item_keys()[0];
        let item = self.corpus.get(key).unwrap().unwrap();
        if self.unreadable.is_some_and(|t| item.text.contains(t)) {
            return Err(PortError::Io(std::io::ErrorKind::InvalidData));
        }
        Ok((self.answer)(&item.text))
    }
}

fn second_half(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let draft = serde_json::json!({ "text": words[words.len() / 2..].join(" ") });
    draft.to_string()
}

fn reciter(dao: &Dao, answer: fn(&str) -> String) -> Reciter {
    Reciter {
        corpus: weksa::store::CorpusStore::open(&dao.corpus).unwrap(),
        answer,
        unreadable: None,
        seen: vec![],
    }
}

#[test]
fn a_recall_probe_asks_for_the_second_half_of_each_live_item_and_never_shows_it() {
    use weksa::run::RecallOutcome;
    let dao = dao();
    let mut perfect = reciter(&dao, second_half);
    let RecallOutcome::Read(r) = dao.run.recall(&mut perfect).unwrap() else {
        panic!("every prompt was answered");
    };
    // Items 1 (Train) and 2, 3 (Validation) are live; 4 and 5 are not offered.
    assert_eq!((r.n, r.answered), (3, 3));
    assert!((r.chrf - 100.0).abs() < 1e-9, "{}", r.chrf);
    assert_eq!(perfect.seen.len(), 3);
    for (text, key) in perfect.seen.iter().zip(["item:daodejing:1", "item:daodejing:2", "item:daodejing:3"]) {
        let item = perfect.corpus.get(key).unwrap().unwrap();
        let rest = item.text.split_whitespace().collect::<Vec<_>>();
        let tail = rest[rest.len() / 2..].join(" ");
        let head = rest[..rest.len() / 2].join(" ");
        assert!(text.contains(&head), "{key}: the opening is shown");
        assert!(!text.contains(&tail), "{key}: the held half is not shown");
    }
}

#[test]
fn a_recall_of_the_wrong_words_or_no_answer_reads_low_and_a_missing_answer_waits() {
    use weksa::run::RecallOutcome;
    let dao = dao();
    let mut wrong = reciter(&dao, |_| r#"{"text":"zzz qqq xxx"}"#.to_string());
    let RecallOutcome::Read(r) = dao.run.recall(&mut wrong).unwrap() else {
        panic!("answered");
    };
    assert_eq!((r.n, r.answered), (3, 3));
    assert!(r.chrf < 10.0, "{}", r.chrf);
    let mut junk = reciter(&dao, |_| "not json".to_string());
    let RecallOutcome::Read(r) = dao.run.recall(&mut junk).unwrap() else {
        panic!("answered");
    };
    assert_eq!((r.n, r.answered, r.chrf), (3, 0, 0.0));
    // With no answers on disk the files port waits on all three prompts.
    let mut files = AnswerFiles::new(dao.run.recall_dir(), "m".into());
    let RecallOutcome::Unanswered { prompts } = dao.run.recall(&mut files).unwrap() else {
        panic!("nobody answered");
    };
    assert_eq!(prompts.len(), 3);
    assert!(dao.run.recall_dir().join(format!("{}.prompt", prompts[0])).exists());
}

#[test]
fn the_published_round_schema_does_not_require_a_field_older_records_lack() {
    let (_, json) = weksa::schema::published()
        .into_iter()
        .find(|(name, _)| name.contains("loop_round"))
        .expect("the round schema is published");
    let schema: serde_json::Value = serde_json::from_str(&json).unwrap();
    let required = schema["required"].as_array().unwrap();
    assert!(required.iter().any(|r| r == "inventory_digest"));
    assert!(!required.iter().any(|r| r == "admit_rules"), "{required:?}");
    assert!(schema["properties"]["admit_rules"].is_object());
}

#[test]
fn a_recall_mean_is_over_every_probe_an_unreadable_answer_counts_0_and_a_withdrawn_item_is_not_probed() {
    use weksa::corpus::Source;
    use weksa::run::RecallOutcome;
    let dao = dao_over(|corpus, entities| {
        let entities = weksa::EntityStore::open(entities).unwrap();
        let mut store = weksa::CorpusStore::open(corpus).unwrap();
        // Another source: probed too, but it names no work.
        let mut written = store.get("item:daodejing:2").unwrap().unwrap();
        written.id = "item:written:1".into();
        written.text = "A line of some other source with words enough to split.".into();
        written.names = vec![];
        written.prior_turns = vec![];
        written.source = Source::OperatorWritten;
        store.add(written, &entities).unwrap();
        store.withdraw("item:daodejing:3").unwrap();
    });
    let both = |dao: &Dao, unreadable: Option<&'static str>| {
        let mut m = reciter(dao, second_half);
        m.unreadable = unreadable;
        let RecallOutcome::Read(r) = dao.run.recall(&mut m).unwrap() else {
            panic!("answered");
        };
        r
    };
    let full = both(&dao, None);
    assert_eq!(
        (full.n, full.answered),
        (3, 3),
        "the written item is probed, the withdrawn one is not"
    );
    // One of three unreadable: the mean falls to two thirds.
    let part = both(&dao, Some("Heaven is long-enduring"));
    assert_eq!((part.n, part.answered), (3, 2));
    assert!((part.chrf - full.chrf * 2.0 / 3.0).abs() < 1e-9, "{}", part.chrf);
    // A public-domain prompt names its work; any other says it is not published.
    let mut m = reciter(&dao, second_half);
    dao.run.recall(&mut m).unwrap();
    let named = m.seen.iter().filter(|t| t.contains("translator: James Legge")).count();
    let unnamed = m.seen.iter().filter(|t| t.contains("work: not published")).count();
    assert_eq!((named, unnamed), (2, 1));
}

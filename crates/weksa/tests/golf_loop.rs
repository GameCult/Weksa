//! The golf loop: a pinned run encodes Train and Validation through a
//! `ModelPort`, admits, lowers, scores, and keeps a typed round record.

mod common;

use std::path::{Path, PathBuf};

use common::{ent, run_over, world};
use cultcache_rs::{CultCache, SingleFileMessagePackBackingStore};
use weksa::corpus::ModelFamily;
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

fn paths(dir: &Path) -> (PathBuf, PathBuf, PathBuf, PathBuf, PathBuf) {
    (
        dir.join("c.cc"),
        dir.join("i.cc"),
        dir.join("entities.cc"),
        common::ontology_in(dir),
        common::speaker_state_in(dir),
    )
}

fn hex(path: &Path) -> String {
    format!("{:016x}", fnv1a64(&std::fs::read(path).unwrap()))
}

#[test]
fn a_run_pins_its_stores_and_refuses_drift() {
    let mut w = world();
    let dir = w.dir.path().to_path_buf();
    let root = dir.join("run");
    let (c, i, e, o, s) = paths(&dir);
    let at = |label: &str, model: &str| {
        RunStores::create(
            &root,
            label,
            RunPaths {
                corpus: &c,
                inventory: &i,
                entities: &e,
                ontology: &o,
                speaker_state: &s,
            },
            model,
            "bag",
            weksa::run::LowererReach::ToolLessRelay,
            ModelFamily::Unrecorded,
            &["en-US".to_string()],
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
    let (c, i, e, o, s) = paths(&dir);
    let source_bytes = std::fs::read(&i).unwrap();
    let mut run = RunStores::create(
        &root,
        "t",
        RunPaths {
            corpus: &c,
            inventory: &i,
            entities: &e,
            ontology: &o,
            speaker_state: &s,
        },
        "fake-model",
        "bag",
        weksa::run::LowererReach::ToolLessRelay,
        ModelFamily::Unrecorded,
        &["en-US".to_string()],
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
                ontology: &dao.ontology,
                speaker_state: &common::speaker_state_beside(&dao.ontology),
            },
            "fake-model",
            "bag",
            weksa::run::LowererReach::ToolLessRelay,
            ModelFamily::Unrecorded,
            &["en-US".to_string()],
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
                ontology: &dao.ontology,
                speaker_state: &common::speaker_state_beside(&dao.ontology),
            },
            "fake-model",
            "bag",
            weksa::run::LowererReach::ToolLessRelay,
            ModelFamily::Unrecorded,
            &["en-US".to_string()],
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
    let o = dir.join("ontology.cc");
    let s = common::speaker_state_beside(&o);
    let paths = || weksa::run::RunPaths {
        corpus: &c,
        inventory: &i,
        entities: &e,
        ontology: &o,
        speaker_state: &s,
    };
    assert_eq!(
        RunStores::create_between(
            &root,
            "t",
            paths(),
            "fake-model",
            "bag",
            weksa::run::LowererReach::ToolLessRelay,
            ModelFamily::Unrecorded,
            &["en-US".to_string()],
            || more(&mut w)
        )
        .err(),
        Some(RunError::Drift(DriftOf::Corpus))
    );
    assert!(!root.join("inventory.cc").exists());
    // Not the copy and not the lock the store's open left beside it.
    assert!(!root.join("inventory.cc.lock").exists());
    assert_eq!(RunStores::open(&root).err(), Some(RunError::NoRun));
    RunStores::create(
        &root,
        "t",
        paths(),
        "fake-model",
        "bag",
        weksa::run::LowererReach::ToolLessRelay,
        ModelFamily::Unrecorded,
        &["en-US".to_string()],
    )
    .unwrap();
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
    let o = dir.join("ontology.cc");
    let paths = weksa::run::RunPaths {
        corpus: &c,
        inventory: &i,
        entities: &e,
        ontology: &o,
        speaker_state: &common::speaker_state_beside(&o),
    };
    let created = RunStores::create_between(
        &root,
        "t",
        paths,
        "fake-model",
        "bag",
        weksa::run::LowererReach::ToolLessRelay,
        ModelFamily::Unrecorded,
        &["en-US".to_string()],
        || change(&mut w, &root),
    )
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
    // packet.rs's serde shapes and ontology.rs's selection: all five are covered.
    let (context, split, lower, packet, ontology) = (
        read("context.rs"),
        read("split.rs"),
        read("lower.rs"),
        read("packet.rs"),
        read("ontology.rs"),
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
        (ontology.len() as u64).to_le_bytes().as_slice(),
        &ontology,
    ]
    .concat();
    assert_eq!(weksa::run::PROMPT_SOURCE_DIGEST, fnv1a64(&framed));
    assert_ne!(
        weksa::run::PROMPT_SOURCE_DIGEST,
        fnv1a64(&[context.clone(), split.clone(), lower.clone(), packet.clone(), ontology.clone()].concat()),
        "bare concatenation is not the digest"
    );
    // Moving the boundary between the parts moves the digest.
    assert_ne!(
        weksa::split::fnv1a64_of(&[b"ab", b"c"]),
        weksa::split::fnv1a64_of(&[b"a", b"bc"])
    );
    assert_ne!(weksa::run::PROMPT_SOURCE_DIGEST, digest("context.rs"));
    assert_ne!(weksa::run::PROMPT_SOURCE_DIGEST, digest("ontology.rs"));
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
    let mut files = AnswerFiles::new(round.clone(), "answerer".into(), vec![]);
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
    let mut blocked = AnswerFiles::new(dir.path().join("file").join("round"), "m".into(), vec![]);
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
    ontology: PathBuf,
    run: RunStores,
}

/// A run over copies of `data/daodejing/{corpus,inventory,entity}.cc`.
fn dao() -> Dao {
    dao_over(|_, _| {})
}

/// A Dao run whose stores `edit` (given the corpus and entity paths) changed
/// before the run pinned them.
fn dao_over(edit: impl FnOnce(&Path, &Path)) -> Dao {
    dao_panel(edit, &["en-US"])
}

/// `dao_over` with the run opened on `panel`.
fn dao_panel(edit: impl FnOnce(&Path, &Path), panel: &[&str]) -> Dao {
    dao_build(edit, panel, |_, _| {})
}

/// `dao_panel` with `seed_ontology` (given the ontology and inventory paths)
/// run before the run pins the ontology.
fn dao_build(
    edit: impl FnOnce(&Path, &Path),
    panel: &[&str],
    seed_ontology: impl FnOnce(&Path, &Path),
) -> Dao {
    let panel: Vec<String> = panel.iter().map(|t| t.to_string()).collect();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/daodejing");
    let dir = tempfile::tempdir().unwrap();
    let copy = |from: &str, to: &str| {
        std::fs::copy(src.join(from), dir.path().join(to)).unwrap();
        dir.path().join(to)
    };
    // The Dao inventory at rev 3: the pins below are written against it.
    common::dao_rev3_at(&dir.path().join("i.cc"));
    let (corpus, inventory, entities) = (
        copy("corpus.cc", "c.cc"),
        dir.path().join("i.cc"),
        copy("entity.cc", "e.cc"),
    );
    edit(&corpus, &entities);
    let ontology = common::ontology_in(dir.path());
    seed_ontology(&ontology, &inventory);
    let root = dir.path().join("run");
    let run = RunStores::create(
        &root,
        "dao",
        RunPaths {
            corpus: &corpus,
            inventory: &inventory,
            entities: &entities,
            ontology: &ontology,
            speaker_state: &common::speaker_state_beside(&ontology),
        },
        "fake-model",
        "bag",
        weksa::run::LowererReach::ToolLessRelay,
        ModelFamily::Unrecorded,
        &panel,
    )
    .unwrap();
    Dao {
        _dir: dir,
        root,
        corpus,
        inventory,
        entities,
        ontology,
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
    assert_eq!((r.language("en-US").unwrap().train_mean.count, r.language("en-US").unwrap().train_mean.chrf), (1, Some(chrf)));

    // Validation: means and refusal counts only.
    let v = &r.language("en-US").unwrap().validation_mean;
    assert_eq!(v.count, 2);
    assert!((v.chrf.unwrap() - 78.56).abs() < 0.01, "{:?}", v.chrf);
    assert!(v.meaning.is_some());
    assert_eq!(r.language("en-US").unwrap().validation_refused, Default::default());
    assert_eq!((r.language("en-US").unwrap().train_scored, r.language("en-US").unwrap().validation_scored), (1, 2));

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
            let state = weksa::SpeakerStateStore::open(&dir.path().join("s.cc")).unwrap();
            let view = view.with_scene(&inv, &state, &corpus).unwrap();
            let prompt = Prompt::lower(&view, &key, "en-US", &inv, dao.run.entities(), &weksa::Ontology::empty()).unwrap();
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

    let mut files = AnswerFiles::new(dao.run.next_round_dir().unwrap(), "fake-model".into(), vec![]);
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
    let mut files = AnswerFiles::new(dao.run.next_round_dir().unwrap(), "fake-model".into(), vec![]);
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
    let mut files = AnswerFiles::new(dao.run.next_round_dir().unwrap(), "fake-model".into(), vec![]);
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
    let mut files = AnswerFiles::new(dao.run.next_round_dir().unwrap(), "fake-model".into(), vec![]);
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
    let mut files = AnswerFiles::new(dao.run.next_round_dir().unwrap(), "fake-model".into(), vec![]);
    let r = recorded(round(&mut dao.run, &mut files, &Scorer::new(Bag)).unwrap());
    assert_eq!((r.n(), r.calls().len()), (1, 6));
    assert_eq!(two_dp(r.language("en-US").unwrap().train_mean.chrf.unwrap()), 44.18);

    // Round 2 asks again, encode first.
    let mut files = AnswerFiles::new(dao.run.next_round_dir().unwrap(), "fake-model".into(), vec![]);
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
    let mut files = AnswerFiles::new(dao.run.next_round_dir().unwrap(), "fake-model".into(), vec![]);
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
    let m = &r.language("en-US").unwrap().train_mean;
    assert_eq!((m.count, m.chrf, m.meaning), (1, Some(0.0), Some(0.0)));
    assert_eq!((r.language("en-US").unwrap().train_scored, r.language("en-US").unwrap().validation_scored), (0, 1));
    assert_eq!(r.language("en-US").unwrap().validation_refused.parse, 0);
    assert_eq!(
        (r.language("en-US").unwrap().validation_refused.admit, r.language("en-US").unwrap().validation_refused.lower),
        (1, 0)
    );
    let m = &r.language("en-US").unwrap().validation_mean;
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
    let v = b_only.language("en-US").unwrap().validation_refused;
    assert_eq!((v.parse, v.admit, v.lower), (1, 0, 0));
    assert_eq!(
        b_only.language("en-US").unwrap().validation_mean.count,
        2,
        "every Validation item counts"
    );
    assert_eq!(b_only.language("en-US").unwrap().validation_scored, 1);
    let b_half = b_only.language("en-US").unwrap().validation_mean.chrf.unwrap();
    assert!(
        (b_half - 74.99 / 2.0).abs() < 0.01,
        "(74.99 + 0) / 2 = {b_half}"
    );

    let a_only = run([good_answers()[0].1.clone(), good_answers()[1].1.clone(), junk]);
    assert_eq!(a_only.language("en-US").unwrap().validation_refused.parse, 1);
    let a_half = a_only.language("en-US").unwrap().validation_mean.chrf.unwrap();
    assert!(
        (a_half - 82.12 / 2.0).abs() < 0.01,
        "(82.12 + 0) / 2 = {a_half}"
    );
    // The halves add to the full mean: (a + 0) / 2 + (0 + b) / 2 = (a + b) / 2.
    let whole = full.language("en-US").unwrap().validation_mean.chrf.unwrap();
    assert!(
        (a_half + b_half - whole).abs() < 1e-9,
        "{a_half} + {b_half} vs {whole}"
    );
    assert!(
        b_half < whole && a_half < whole,
        "refusing never raises the mean"
    );
    assert_eq!(full.language("en-US").unwrap().validation_scored, 2);
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
    let mut files = AnswerFiles::new(round1.clone(), "fake-model".into(), vec![]);
    let r = recorded(round(&mut dao.run, &mut files, &Scorer::new(Bag)).unwrap());
    assert_eq!(r.calls().len(), 5);
    // Item 3 is Validation: its refusal is a Parse refusal and the round stands.
    let v = r.language("en-US").unwrap().validation_refused;
    assert_eq!((v.parse, v.admit, v.lower), (1, 0, 0));
    assert_eq!((r.language("en-US").unwrap().train_scored, r.language("en-US").unwrap().validation_scored), (1, 1));
    assert_eq!(r.language("en-US").unwrap().validation_mean.count, 2);
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
    assert_eq!(r.language("en-US").unwrap().validation_refused, Default::default());
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
    assert_eq!(r.language("en-US").unwrap().validation_refused, Default::default());
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
    let (t, v) = (&r.language("en-US").unwrap().train_mean, &r.language("en-US").unwrap().validation_mean);
    assert_eq!(
        line,
        format!(
            "round 1 rev 3 en-US train n=1 scored=1 chrf={:.2} meaning={:.4} refused=0 validation n=2 scored=2 chrf={:.2} meaning={:.4} refused parse=0 admit=0 lower=0 lowering=tool-less-relay",
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
        ontology: &missing,
        speaker_state: &common::speaker_state_beside(&missing),
    };
    let e = RunStores::create(
        &root,
        "ok",
        stores,
        "m",
        "bag",
        weksa::run::LowererReach::ToolLessRelay,
        ModelFamily::Unrecorded,
        &["en-US".to_string()],
    )
    .err()
    .unwrap();
    note(&e, &e);
    let e = RunStores::create(
        &root,
        &format!("{canary}!"),
        stores,
        "m",
        "bag",
        weksa::run::LowererReach::ToolLessRelay,
        ModelFamily::Unrecorded,
        &["en-US".to_string()],
    )
    .err()
    .unwrap();
    note(&e, &e);
    let e = RunStores::create(
        &root,
        "ok",
        stores,
        &format!("{canary}\n"),
        "bag",
        weksa::run::LowererReach::ToolLessRelay,
        ModelFamily::Unrecorded,
        &["en-US".to_string()],
    )
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
    let mut files = AnswerFiles::new(blocker.join("canarydir"), "m".into(), vec![]);
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
            ontology: &dao.ontology,
            speaker_state: &common::speaker_state_beside(&dao.ontology),
        },
        "fake-model",
        "bag",
        weksa::run::LowererReach::ToolLessRelay,
        ModelFamily::Unrecorded,
        &["en-US".to_string()],
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
    Reading::of(&r.language("en-US").unwrap().validation_mean).unwrap()
}

/// The feedback line the editor is shown for a head round, built from outside.
fn feedback_line(last: &str, head: &RoundRecord) -> String {
    let rounded = (validation(head).composite / LADDER_STEP).round() * LADDER_STEP;
    format!("last edit: {last}; validation composite at head: en-US {rounded:.3}")
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
    assert_eq!(d.panel()[0].threshold, LADDER_STEP);
    assert_eq!(
        d.panel()[0].floor,
        Reading {
            chrf: 0.0,
            meaning: 0.0,
            composite: 0.0
        }
    );
    assert_eq!(d.panel()[0].base, validation(&rounds[0]));
    let record = dao.run.rounds().unwrap().remove(2);
    assert_eq!(d.panel()[0].candidate, Some(validation(&record)));
    assert!(d.panel()[0].candidate.unwrap().composite - d.panel()[0].base.composite > LADDER_STEP);

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
    let (b, c) = (d.panel()[0].base.composite, d.panel()[0].candidate.unwrap().composite);
    assert_eq!(
        dao.run.cycle_line(&d),
        format!(
            "cycle 1 rev 3->4 en-US base={b:.4} candidate={c:.4} floor=0.0000 threshold=0.0050 accepted lowering=tool-less-relay"
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
    assert_eq!(d2.panel()[0].floor, d.panel()[0].floor, "the floor is measured once");
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
    assert_eq!(d.panel()[0].candidate, Some(d.panel()[0].base));
    let c = d.panel()[0].base.composite;
    assert_eq!(
        dao.run.cycle_line(&d),
        format!(
            "cycle 1 rev 3->4 en-US base={c:.4} candidate={c:.4} floor=0.0000 threshold=0.0050 rejected no-gain lowering=tool-less-relay"
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
    assert_eq!(d.panel()[0].floor, want);
    assert_eq!(d.panel()[0].threshold, want.composite);
    assert_eq!(d.panel()[0].candidate, Some(b));
    assert!(d.panel()[0].candidate.unwrap().composite - d.panel()[0].base.composite > LADDER_STEP);
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
                AnswerFiles::new(dao.run.next_cycle_dir().unwrap(), "fake-model".into(), vec![]);
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
        let mut files = AnswerFiles::new(dao.run.next_cycle_dir().unwrap(), "fake-model".into(), vec![]);
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
    assert!(mint.contains("\n\nlast edit: none; validation composite at head: en-US 0.435\n\n"));
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
            (d.candidate_rev(), d.candidate_round(), d.panel()[0].candidate),
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
                "cycle 1 rev 3->none en-US base={:.4} candidate=none floor=0.0000 threshold=0.0050 rejected {tag} lowering=tool-less-relay",
                d.panel()[0].base.composite
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

    // A panel language with no live Validation item never reaches a round:
    // `create` refuses it (open_refuses_a_bad_panel).
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
            ontology: &dao.ontology,
            speaker_state: &common::speaker_state_beside(&dao.ontology),
        },
        "fake-model",
        "bag",
        weksa::run::LowererReach::ToolLessRelay,
        ModelFamily::Unrecorded,
        &["en-US".to_string()],
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
    // Every prompt file leaves through RunStores::answer_files, which withholds Test text.
    assert!(!read("main.rs").contains("AnswerFiles::new"));
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
        "ItemScore",
        "LanguageScores",
        "validation_scores",
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
    assert_eq!(
        code.iter()
            .filter(|l| l.contains("lower(&") && !l.contains("Prompt::lower("))
            .count(),
        0,
        "run.rs calls the table"
    );
    assert_eq!(count("crate::lower::lower"), 0, "run.rs imports the table");
    assert_eq!(
        count("Prompt::lower("),
        2,
        "two lowering call sites: the round's and the stance ablation's replay of it"
    );
    // Prompt::lower takes the view, the item's key, the inventory and the entity
    // port: no corpus item, train set, offer or store.
    let start = context.find("pub fn lower(").unwrap();
    let signature = &context[start..start + context[start..].find(") -> ").unwrap()];
    for banned in ["CorpusItem,", "CorpusItem)", "TrainItems", "EncoderOffer", "Store"] {
        assert!(!signature.contains(banned), "Prompt::lower takes {banned}");
    }
    assert!(signature.contains("&LoweringInput") && signature.contains("&CorpusItemId"));
    assert!(signature.contains("&Ontology"), "the whole ontology goes in; select chooses");
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
        store.add(item.clone(), &entities).unwrap();
        // A held-out ja-JP line too: the panel is en-US, so it is never offered.
        item.id = "item:canary:2".into();
        item.text = "CANARYJAPANESE 検証テキスト".into();
        item.language = "ja-JP".into();
        store.add(item, &entities).unwrap();
    });
    let mut model = fake(&good_answers());
    model.answers.insert(canary_key.into(), json(&draft_of(2)));
    model.lowered.insert(canary_key.into(), "CANARYLOWERED".into());
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());

    let corpus = weksa::CorpusStore::open(&dao.corpus).unwrap();
    let items = corpus.all().unwrap();
    assert!(items.len() >= 7, "the canaries and the five Legge items");
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
        let v = r.language("en-US").unwrap().validation_refused;
        assert_eq!((v.parse, v.admit, v.lower), (0, 0, 2), "{what}");
        assert_eq!((r.language("en-US").unwrap().train_scored, r.language("en-US").unwrap().validation_scored), (0, 0), "{what}");
        let m = &r.language("en-US").unwrap().validation_mean;
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
        ontology: &dao.ontology,
        speaker_state: &common::speaker_state_beside(&dao.ontology),
    };
    RunStores::create(
        &root,
        "b",
        paths,
        "fake-model",
        "bag",
        LowererReach::BriefedSubagent,
        ModelFamily::Unrecorded,
        &["en-US".to_string()],
    )
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
            (r.language("en-US").unwrap().train_scored, r.language("en-US").unwrap().validation_scored),
            (1, 2),
            "{}",
            text.len()
        );
        assert_eq!(r.language("en-US").unwrap().validation_refused, Default::default());
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
    let v = r.language("en-US").unwrap().validation_refused;
    assert_eq!((v.parse, v.admit, v.lower), (0, 0, 1));
    assert_eq!((r.language("en-US").unwrap().train_scored, r.language("en-US").unwrap().validation_scored), (1, 2));
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
    let mut files = AnswerFiles::new(round1.clone(), "fake-model".into(), vec![]);
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

/// Item 1's good draft as JSON, changed by `change`, as the model's answer.
fn mangled(change: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut v: serde_json::Value = serde_json::from_str(&json(&draft_of(1))).unwrap();
    change(&mut v);
    v.to_string()
}

/// The round record of a Dao run whose item 1 is answered with `answer` and
/// the rest well.
fn round_with_item_1(answer: String) -> (Dao, RoundRecord) {
    let mut dao = dao();
    let mut answers = good_answers();
    answers[0].1 = answer;
    let mut model = fake(&answers);
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    (dao, r)
}

fn reasons(r: &RoundRecord) -> Vec<(&str, u32)> {
    r.parse_reasons()
        .iter()
        .map(|c| (c.rule.as_str(), c.count))
        .collect()
}

#[test]
fn parse_reasons_by_class() {
    let good = json(&draft_of(1));
    let truncated = good[..good.len() / 2].to_string();
    let cases: Vec<(&str, String)> = vec![
        ("syntax", "not json at all".to_string()),
        ("eof", truncated),
        (
            "invalid-type",
            mangled(|v| v["referents"][0]["concept"] = serde_json::json!(7)),
        ),
        (
            "bad-form",
            mangled(|v| v["referents"][0]["concept"] = serde_json::json!("entity:x")),
        ),
        (
            "unknown-field",
            mangled(|v| v["surprise"] = serde_json::json!(1)),
        ),
        (
            "missing-field",
            mangled(|v| {
                v.as_object_mut().unwrap().remove("referents");
            }),
        ),
    ];
    for (code, answer) in cases {
        let (dao, r) = round_with_item_1(answer);
        assert_eq!(reasons(&r), [(code, 1)], "{code}");
        assert_eq!(
            r.train()[0].outcome,
            Outcome::Refused(Stage::Parse),
            "{code}"
        );
        let line = dao.run.round_line(&r);
        assert!(line.contains(&format!("parse-reasons={code}x1")), "{line}");
    }
    // A round nothing refused at Parse carries and prints none.
    let (dao, r) = round_with_item_1(good);
    assert!(r.parse_reasons().is_empty());
    assert!(!dao.run.round_line(&r).contains("parse-reasons"));
}

#[test]
fn parse_reasons_count_per_code_over_both_splits_and_name_unreadable() {
    let mut dao = dao();
    let expected = prompts(&dao.run);
    let answers = keyed(&good_answers());
    let round1 = dao.root.join("round-1");
    std::fs::create_dir_all(&round1).unwrap();
    for (key, p) in &expected {
        let bytes = match key.as_str() {
            // Train: not UTF-8, so no answer parsed from the file.
            "item:daodejing:1" => vec![0xff, 0xfe],
            // Validation: not JSON.
            "item:daodejing:2" => b"nope".to_vec(),
            _ => answers[key].clone().into_bytes(),
        };
        std::fs::write(round1.join(format!("{}.answer", p.digest())), bytes).unwrap();
    }
    let table = table_english();
    for (key, p) in lowering_prompts(&dao, dao.run.inventory())
        .iter()
        .filter(|(key, _)| key == "item:daodejing:3")
    {
        let answer = serde_json::json!({ "text": table[key] }).to_string();
        std::fs::write(round1.join(format!("{}.answer", p.digest())), answer).unwrap();
    }
    let mut files = AnswerFiles::new(round1, "fake-model".into(), vec![]);
    let r = recorded(round(&mut dao.run, &mut files, &Scorer::new(Bag)).unwrap());
    assert_eq!(reasons(&r), [("syntax", 1), ("unreadable", 1)]);
}

#[test]
fn parse_reasons_never_echo_answer() {
    let canary = "CANARY-7f3a-held-out";
    // The canary sits where serde would quote it: an unknown field name, an
    // invalid value, and the raw text of a non-JSON answer.
    let answers = [
        mangled(|v| v[canary] = serde_json::json!(1)),
        mangled(|v| v["referents"][0]["concept"] = serde_json::json!(canary)),
        format!("{canary} is not json"),
    ];
    for answer in answers {
        // Item 2 is Validation: the held-out item carries the canary.
        let mut dao = dao();
        let mut a = good_answers();
        a[1].1 = answer;
        let mut model = fake(&a);
        let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
        assert_eq!(r.language("en-US").unwrap().validation_refused.parse, 1);
        let line = dao.run.round_line(&r);
        assert!(!line.contains(canary) && !line.contains("CANARY"), "{line}");
        let bytes = std::fs::read(dao.root.join("run.cc")).unwrap();
        assert!(!bytes.windows(6).any(|w| w == b"CANARY"));
        assert!(!format!("{r:?}").contains("CANARY"));
    }
}

/// A run record as written before `answerer_family` (key 13) and `panel` (key 14) existed.
#[derive(Clone, Debug, schemars::JsonSchema, cultcache_rs::DatabaseEntry)]
#[cultcache(type = "weksa.loop_run")]
struct PreFamilyRun {
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
    #[cultcache(key = 8)]
    model: String,
    #[cultcache(key = 9)]
    embedder: String,
    #[cultcache(key = 10)]
    chrf_version: String,
    #[cultcache(key = 11)]
    inventory_digest: String,
    #[cultcache(key = 12)]
    lowerer_reach: weksa::run::LowererReach,
}

/// A round record as written with the pooled means (keys 11 to 15) and before
/// `parse_reasons`, `own_family` and `languages`.
#[derive(Clone, Debug, schemars::JsonSchema, cultcache_rs::DatabaseEntry)]
#[cultcache(type = "weksa.loop_round")]
struct LegacyRound {
    #[cultcache(key = 0)]
    n: u32,
    #[cultcache(key = 1)]
    inventory_rev: u32,
    #[cultcache(key = 2)]
    prompt_digest: String,
    #[cultcache(key = 4)]
    corpus_digest: String,
    #[cultcache(key = 5)]
    entity_digest: String,
    #[cultcache(key = 6)]
    model: String,
    #[cultcache(key = 7)]
    embedder: String,
    #[cultcache(key = 8)]
    chrf_version: String,
    #[cultcache(key = 9)]
    calls: Vec<weksa::run::Call>,
    #[cultcache(key = 10)]
    train: Vec<weksa::run::TrainRow>,
    #[cultcache(key = 11)]
    train_mean: weksa::score::SplitMean,
    #[cultcache(key = 12)]
    validation_mean: weksa::score::SplitMean,
    #[cultcache(key = 13)]
    validation_refused: weksa::run::RefusalCounts,
    #[cultcache(key = 14)]
    train_scored: u32,
    #[cultcache(key = 15)]
    validation_scored: u32,
    #[cultcache(key = 16)]
    inventory_digest: String,
    #[cultcache(key = 17)]
    cycle: Option<u32>,
}

#[test]
fn a_run_from_before_does_not_open() {
    let mut dao = dao();
    let mut model = fake(&good_answers());
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    let rec = dao.run.record().clone();
    let root = dao.root.clone();
    let legacy = LegacyRound {
        n: r.n(),
        inventory_rev: r.inventory_rev(),
        prompt_digest: r.prompt_digest().into(),
        corpus_digest: r.corpus_digest().into(),
        entity_digest: r.entity_digest().into(),
        model: r.model().into(),
        embedder: r.embedder().into(),
        chrf_version: r.chrf_version().into(),
        calls: r.calls().to_vec(),
        train: r.train().to_vec(),
        train_mean: r.language("en-US").unwrap().train_mean.clone(),
        validation_mean: r.language("en-US").unwrap().validation_mean.clone(),
        validation_refused: r.language("en-US").unwrap().validation_refused,
        train_scored: r.language("en-US").unwrap().train_scored,
        validation_scored: r.language("en-US").unwrap().validation_scored,
        inventory_digest: r.inventory_digest().into(),
        cycle: r.cycle(),
    };
    let run = PreFamilyRun {
        label: rec.label.clone(),
        corpus: rec.corpus.clone(),
        inventory: rec.inventory.clone(),
        entities: rec.entities.clone(),
        corpus_digest: rec.corpus_digest.clone(),
        entity_digest: rec.entity_digest.clone(),
        prompt_digest: rec.prompt_digest.clone(),
        model: rec.model.clone(),
        embedder: rec.embedder.clone(),
        chrf_version: rec.chrf_version.clone(),
        inventory_digest: rec.inventory_digest.clone(),
        lowerer_reach: rec.lowerer_reach,
    };
    drop(dao.run);
    let store = || SingleFileMessagePackBackingStore::new(root.join("run.cc"));
    // A run record without a panel (no key 14) is a scratch run off git.
    {
        let mut cache = CultCache::new();
        cache.register_entry_type::<PreFamilyRun>().unwrap();
        cache.register_entry_type::<LegacyRound>().unwrap();
        cache.add_generic_backing_store(store()).unwrap();
        cache.pull_all_backing_stores().unwrap();
        cache.put("run:dao".to_string(), &run).unwrap();
        cache.put("run:dao:round:1".to_string(), &legacy).unwrap();
    }
    assert_eq!(RunStores::open(&root).err(), Some(RunError::Store));
    // A round with the pooled means (keys 11 to 15) and no languages is
    // unreadable, under a record that has its panel.
    let _ = std::fs::remove_file(root.join("run.cc"));
    {
        let mut cache = CultCache::new();
        cache.register_entry_type::<RunRecord>().unwrap();
        cache.register_entry_type::<LegacyRound>().unwrap();
        cache.add_generic_backing_store(store()).unwrap();
        cache.pull_all_backing_stores().unwrap();
        cache.put("run:dao".to_string(), &rec).unwrap();
        cache.put("run:dao:round:1".to_string(), &legacy).unwrap();
    }
    // No round of that run can be read: it stops at its first round or cycle.
    let reopened = RunStores::open(&root).unwrap();
    assert!(reopened.rounds().is_err());
}

/// A round as the file holds it, with key 22 the lib keeps private.
#[derive(Clone, Debug, schemars::JsonSchema, cultcache_rs::DatabaseEntry)]
#[cultcache(type = "weksa.loop_round")]
struct ScoredRound {
    #[cultcache(key = 0)]
    n: u32,
    #[cultcache(key = 1)]
    inventory_rev: u32,
    #[cultcache(key = 2)]
    prompt_digest: String,
    #[cultcache(key = 4)]
    corpus_digest: String,
    #[cultcache(key = 5)]
    entity_digest: String,
    #[cultcache(key = 6)]
    model: String,
    #[cultcache(key = 7)]
    embedder: String,
    #[cultcache(key = 8)]
    chrf_version: String,
    #[cultcache(key = 9)]
    calls: Vec<weksa::run::Call>,
    #[cultcache(key = 10)]
    train: Vec<weksa::run::TrainRow>,
    #[cultcache(key = 16)]
    inventory_digest: String,
    #[cultcache(key = 17)]
    cycle: Option<u32>,
    #[cultcache(key = 18)]
    admit_rules: Vec<weksa::run::RuleCount>,
    #[cultcache(key = 19)]
    parse_reasons: Vec<weksa::run::RuleCount>,
    #[cultcache(key = 20)]
    own_family: u32,
    #[cultcache(key = 21)]
    languages: Vec<weksa::run::LanguageRound>,
    #[cultcache(key = 22)]
    validation_scores: Vec<weksa::run::LanguageScores>,
}

/// A round as written before key 22 existed.
#[derive(Clone, Debug, schemars::JsonSchema, cultcache_rs::DatabaseEntry)]
#[cultcache(type = "weksa.loop_round")]
struct PreScoresRound {
    #[cultcache(key = 0)]
    n: u32,
    #[cultcache(key = 1)]
    inventory_rev: u32,
    #[cultcache(key = 2)]
    prompt_digest: String,
    #[cultcache(key = 4)]
    corpus_digest: String,
    #[cultcache(key = 5)]
    entity_digest: String,
    #[cultcache(key = 6)]
    model: String,
    #[cultcache(key = 7)]
    embedder: String,
    #[cultcache(key = 8)]
    chrf_version: String,
    #[cultcache(key = 9)]
    calls: Vec<weksa::run::Call>,
    #[cultcache(key = 10)]
    train: Vec<weksa::run::TrainRow>,
    #[cultcache(key = 16)]
    inventory_digest: String,
    #[cultcache(key = 17)]
    cycle: Option<u32>,
    #[cultcache(key = 18)]
    admit_rules: Vec<weksa::run::RuleCount>,
    #[cultcache(key = 19)]
    parse_reasons: Vec<weksa::run::RuleCount>,
    #[cultcache(key = 20)]
    own_family: u32,
    #[cultcache(key = 21)]
    languages: Vec<weksa::run::LanguageRound>,
}

/// The scores a round put in `run.cc`, read through the mirror (the lib's field
/// is private). The run must be dropped first.
fn stored_scores(root: &Path) -> Vec<weksa::run::LanguageScores> {
    let mut cache = CultCache::new();
    cache.register_entry_type::<RunRecord>().unwrap();
    cache.register_entry_type::<ScoredRound>().unwrap();
    cache
        .add_generic_backing_store(SingleFileMessagePackBackingStore::new(root.join("run.cc")))
        .unwrap();
    cache.pull_all_backing_stores().unwrap();
    cache.get_all::<ScoredRound>().unwrap().remove(0).validation_scores
}

/// The stored scores agree with the round's readings: one entry per language
/// in the record's order, as many scores as Validation items (scored plus
/// refused), the same means, and at least the refused items as exact zeros.
fn assert_scores_match_readings(r: &RoundRecord, stored: &[weksa::run::LanguageScores]) {
    let tags: Vec<&str> = r.languages().iter().map(|l| l.language.as_str()).collect();
    let got: Vec<&str> = stored.iter().map(|l| l.language.as_str()).collect();
    assert_eq!(got, tags, "one entry per panel language, in panel order");
    for (reading, entry) in r.languages().iter().zip(stored) {
        let m = &reading.validation_mean;
        let v = reading.validation_refused;
        let refused = (v.parse + v.admit + v.lower) as usize;
        assert_eq!(entry.scores.len(), m.count, "{}", reading.language);
        assert_eq!(entry.scores.len(), reading.validation_scored as usize + refused);
        let n = entry.scores.len() as f64;
        let chrf = entry.scores.iter().map(|s| s.chrf).sum::<f64>() / n;
        assert!((chrf - m.chrf.unwrap()).abs() < 1e-9, "{}", reading.language);
        let zeros = entry
            .scores
            .iter()
            .filter(|s| s.chrf == 0.0 && s.meaning == Some(0.0))
            .count();
        assert!(zeros >= refused, "{}: refused items are stored as zeros", reading.language);
    }
}

#[test]
fn a_round_stores_one_score_per_validation_item() {
    let mut dao = dao();
    let mut model = fake(&good_answers());
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    let root = dao.root.clone();
    drop(dao.run);
    let stored = stored_scores(&root);
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].language, "en-US");
    assert_scores_match_readings(&r, &stored);
}

#[test]
fn a_two_language_round_stores_each_languages_scores_in_panel_order() {
    let mut dao = bilingual(LAOZI_JA);
    let mut model = bilingual_fake();
    // The ja-JP Validation item 13 is not an encodable draft: it scores 0.
    model.answers.insert("item:daodejing:13".into(), "not a draft".into());
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    let root = dao.root.clone();
    drop(dao.run);
    let stored = stored_scores(&root);
    assert_scores_match_readings(&r, &stored);
    let tags: Vec<&str> = stored.iter().map(|l| l.language.as_str()).collect();
    assert_eq!(tags, ["en-US", "ja-JP"]);
    // ja-JP: item 12 reads perfectly, then item 13 (the refused one), in
    // stored item order; none of en-US's items is among them.
    let ja: Vec<(f64, Option<f64>)> = stored[1].scores.iter().map(|s| (s.chrf, s.meaning)).collect();
    assert_eq!(ja, [(100.0, Some(1.0)), (0.0, Some(0.0))]);
    assert_eq!(stored[0].scores.len(), 2);
    assert!(stored[0].scores.iter().all(|s| s.chrf < 100.0), "en-US holds its own items");
}

#[test]
fn refused_validation_items_are_stored_as_zeros_at_every_stage() {
    // Lower: the ja-JP items cite an entity with no ja-JP form.
    let mut dao = bilingual(None);
    let mut model = bilingual_fake();
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    let root = dao.root.clone();
    drop(dao.run);
    let stored = stored_scores(&root);
    assert_eq!(r.language("ja-JP").unwrap().validation_refused.lower, 2);
    assert_scores_match_readings(&r, &stored);
    assert!(stored[1].scores.iter().all(|s| s.chrf == 0.0 && s.meaning == Some(0.0)));

    // Parse (item 3 is Validation) and Admit (item 2), one run each.
    for (parse, admit) in [(true, false), (false, true)] {
        let mut dao = self::dao();
        let mut answers = good_answers();
        if parse {
            answers[2].1 = "not json at all".into();
        }
        if admit {
            answers[1].1 = edited(2, unknown_concept);
        }
        let mut model = fake(&answers);
        let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
        let root = dao.root.clone();
        drop(dao.run);
        let stored = stored_scores(&root);
        let v = r.language("en-US").unwrap().validation_refused;
        assert_eq!((v.parse, v.admit), (u32::from(parse), u32::from(admit)));
        assert_scores_match_readings(&r, &stored);
    }
}

#[test]
fn an_old_run_without_scores_still_opens() {
    let mut dao = dao();
    let mut model = fake(&good_answers());
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    let rec = dao.run.record().clone();
    let root = dao.root.clone();
    drop(dao.run);
    let old = PreScoresRound {
        n: r.n(),
        inventory_rev: r.inventory_rev(),
        prompt_digest: r.prompt_digest().into(),
        corpus_digest: r.corpus_digest().into(),
        entity_digest: r.entity_digest().into(),
        model: r.model().into(),
        embedder: r.embedder().into(),
        chrf_version: r.chrf_version().into(),
        calls: r.calls().to_vec(),
        train: r.train().to_vec(),
        inventory_digest: r.inventory_digest().into(),
        cycle: r.cycle(),
        admit_rules: vec![],
        parse_reasons: vec![],
        own_family: 0,
        languages: vec![r.language("en-US").unwrap().clone()],
    };
    let _ = std::fs::remove_file(root.join("run.cc"));
    {
        let mut cache = CultCache::new();
        cache.register_entry_type::<RunRecord>().unwrap();
        cache.register_entry_type::<PreScoresRound>().unwrap();
        cache
            .add_generic_backing_store(SingleFileMessagePackBackingStore::new(root.join("run.cc")))
            .unwrap();
        cache.pull_all_backing_stores().unwrap();
        cache.put("run:dao".to_string(), &rec).unwrap();
        cache.put("run:dao:round:1".to_string(), &old).unwrap();
    }
    let reopened = RunStores::open(&root).unwrap();
    let rounds = reopened.rounds().unwrap();
    assert_eq!(rounds.len(), 1);
    assert_eq!(rounds[0].language("en-US"), r.language("en-US"));
}

#[test]
fn the_published_run_and_round_schemas_require_none_of_the_new_fields() {
    let published = weksa::schema::published();
    let schema = |needle: &str| -> serde_json::Value {
        let (_, json) = published
            .iter()
            .find(|(name, _)| name.contains(needle))
            .unwrap();
        serde_json::from_str(json).unwrap()
    };
    for (needle, fields) in [
        ("loop_round", &["parse_reasons", "own_family"][..]),
        ("loop_run", &["answerer_family"][..]),
    ] {
        let s = schema(needle);
        let required = s["required"].as_array().unwrap();
        for f in fields {
            assert!(s["properties"][f].is_object(), "{needle} {f}");
            assert!(!required.iter().any(|r| r == f), "{needle} {f} is required");
        }
    }
}

/// What a recall prompt shows of its item: the text after `opening: `.
fn shown_opening(prompt: &str) -> &str {
    prompt.rsplit_once("opening: ").expect("a recall prompt has an opening").1
}

/// The item's text after the opening the prompt showed.
fn held_back<'a>(text: &'a str, prompt: &str) -> &'a str {
    let opening = shown_opening(prompt);
    text.strip_prefix(opening)
        .unwrap_or_else(|| panic!("the opening is a prefix of the item"))
        .trim()
}

/// Answers a recall probe from the corpus: the item's own held-back half (its
/// text after the opening the prompt showed), a fixed wrong text, or junk.
struct Reciter {
    corpus: weksa::store::CorpusStore,
    /// Given the item's held-back half.
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
        Ok((self.answer)(held_back(&item.text, prompt.text())))
    }
}

fn second_half(held_back: &str) -> String {
    serde_json::json!({ "text": held_back }).to_string()
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
    let RecallOutcome::Read(reading) = dao.run.recall(&mut perfect).unwrap() else {
        panic!("every prompt was answered");
    };
    let r = reading.languages[0].clone();
    // Items 1 (Train) and 2, 3 (Validation) are live; 4 and 5 are not offered.
    assert_eq!((r.n, r.answered), (3, 3));
    assert!((r.chrf - 100.0).abs() < 1e-9, "{}", r.chrf);
    assert_eq!(perfect.seen.len(), 3);
    for (text, key) in perfect.seen.iter().zip(["item:daodejing:1", "item:daodejing:2", "item:daodejing:3"]) {
        let item = perfect.corpus.get(key).unwrap().unwrap();
        // The opening is a proper prefix of the item, cut at a word boundary
        // near its middle; the rest is held back.
        let head = shown_opening(text);
        let tail = held_back(&item.text, text);
        assert!(!head.is_empty() && !tail.is_empty(), "{key}");
        let squeeze = |t: &str| t.split_whitespace().collect::<String>();
        assert_eq!(squeeze(&item.text), squeeze(&format!("{head}{tail}")), "{key}: head and tail are the item");
        let n = item.text.len();
        assert!((n / 4..=3 * n / 4).contains(&head.len()), "{key}: near the middle");
        assert!(!text.contains(tail), "{key}: the held half is not shown");
    }
}

#[test]
fn a_recall_of_the_wrong_words_or_no_answer_reads_low_and_a_missing_answer_waits() {
    use weksa::run::RecallOutcome;
    let dao = dao();
    let mut wrong = reciter(&dao, |_| r#"{"text":"zzz qqq xxx"}"#.to_string());
    let RecallOutcome::Read(reading) = dao.run.recall(&mut wrong).unwrap() else {
        panic!("answered");
    };
    let r = reading.languages[0].clone();
    assert_eq!((r.n, r.answered), (3, 3));
    assert!(r.chrf < 10.0, "{}", r.chrf);
    let mut junk = reciter(&dao, |_| "not json".to_string());
    let RecallOutcome::Read(reading) = dao.run.recall(&mut junk).unwrap() else {
        panic!("answered");
    };
    let r = reading.languages[0].clone();
    assert_eq!((r.n, r.answered, r.chrf), (3, 0, 0.0));
    // With no answers on disk the files port waits on all three prompts.
    let mut files = AnswerFiles::new(dao.run.recall_dir(), "m".into(), vec![]);
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
        let RecallOutcome::Read(reading) = dao.run.recall(&mut m).unwrap() else {
            panic!("answered");
        };
        reading.languages[0].clone()
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

/// Answers every prompt with text that is not a draft, so a round measures only
/// what it counts before admission.
struct NotADraft;

impl weksa::model::ModelPort for NotADraft {
    fn model(&self) -> &str {
        "not-a-draft"
    }
    fn answer(&mut self, _: &weksa::context::Prompt) -> Result<String, weksa::model::PortError> {
        Ok("not a draft".to_string())
    }
}

struct OneBag;

impl weksa::score::Embedder for OneBag {
    fn model(&self) -> &str {
        "bag"
    }
    fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|_| vec![1.0]).collect())
    }
}

/// A corpus of every shape a family count must tell apart, and the own-family
/// count of a round over it by an answerer of `family`.
fn mixed_family_round(family: ModelFamily) -> u32 {
    use common::{cast, group_in_bucket, item_in};
    use weksa::corpus::Source;
    use weksa::run::{LowererReach, RoundOutcome, round};

    let mut w = world();
    let written = |family| Source::ModelWritten {
        family,
        generator: None,
        prompt_digest: None,
    };
    let public_domain = Source::PublicDomain {
        work: "w".into(),
        author: "a".into(),
        edition_url: "https://example.org/e".into(),
        licence: "PD".into(),
        jurisdiction: "US".into(),
        translator: "t".into(),
        edition_year: 1900,
    };
    let (train, validation, test) = (
        group_in_bucket(10),
        group_in_bucket(50),
        group_in_bucket(90),
    );
    let items = [
        // Offered, Anthropic: two of them count for Anthropic.
        ("item:mix:1", &train, written(ModelFamily::Anthropic)),
        ("item:mix:2", &validation, written(ModelFamily::Anthropic)),
        // Offered, other families or no model at all: never Anthropic's.
        ("item:mix:3", &train, written(ModelFamily::OpenAi)),
        ("item:mix:4", &train, written(ModelFamily::Unrecorded)),
        ("item:mix:5", &train, public_domain),
        ("item:mix:6", &train, cast()),
        // Model-written in a Test group: in no split, never offered.
        ("item:mix:7", &test, written(ModelFamily::Anthropic)),
        // Withdrawn after admission: never offered.
        ("item:mix:8", &train, written(ModelFamily::Anthropic)),
    ];
    let entities = common::entities_in(w.dir.path());
    for (key, group, source) in items {
        let text = format!("Line {key}, distinct.");
        w.corpus
            .add(item_in(key, &text, group, source), &entities)
            .unwrap();
    }
    w.corpus.withdraw("item:mix:8").unwrap();
    let dir = w.dir.path().to_path_buf();
    for name in ["c.cc", "i.cc", "entities.cc"] {
        assert!(dir.join(name).exists(), "{name}");
    }
    let mut run = RunStores::create(
        &dir.join("run"),
        "mix",
        RunPaths {
            corpus: &dir.join("c.cc"),
            inventory: &dir.join("i.cc"),
            entities: &dir.join("entities.cc"),
            ontology: &dir.join("ontology.cc"),
            speaker_state: &common::speaker_state_beside(&dir.join("ontology.cc")),
        },
        "not-a-draft",
        "bag",
        LowererReach::ToolLessRelay,
        family,
        &["en-US".to_string()],
    )
    .unwrap();
    let scorer = weksa::score::Scorer::new(OneBag);
    match round(&mut run, &mut NotADraft, &scorer).unwrap() {
        RoundOutcome::Recorded(r) => {
            // Eight items were offered: the world's two cast items and mix 1 to 6;
            // the Test-group and withdrawn items were not.
            assert_eq!(
                r.parse_reasons().iter().map(|c| c.count).sum::<u32>(),
                8,
                "offered items"
            );
            r.own_family()
        }
        RoundOutcome::Unanswered { .. } => panic!("answered"),
    }
}

#[test]
fn own_family_counts_offered_items_of_the_answerer_family() {
    // Two live Train/Validation Anthropic items; the Test-group and withdrawn
    // Anthropic items are not offered, so they are not counted.
    assert_eq!(mixed_family_round(ModelFamily::Anthropic), 2);
    assert_eq!(mixed_family_round(ModelFamily::OpenAi), 1);
    // Google wrote nothing here; an unrecorded answerer counts no item, though
    // an offered item is itself unrecorded.
    assert_eq!(mixed_family_round(ModelFamily::Google), 0);
    assert_eq!(mixed_family_round(ModelFamily::Unrecorded), 0);
}

// ---- stance ablation (stance-to-lowerer) ----

/// A lowerer by function: the text it answers for an item key and the prompt
/// text, recording every prompt it was shown.
struct Lowerer {
    f: fn(&str, &str) -> String,
    seen: Vec<(String, String, String)>,
}

impl Lowerer {
    fn new(f: fn(&str, &str) -> String) -> Self {
        Self { f, seen: vec![] }
    }
    /// The digests asked, once each, in the order first asked.
    fn digests(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (d, ..) in &self.seen {
            if !out.contains(d) {
                out.push(d.clone());
            }
        }
        out
    }
}

impl ModelPort for Lowerer {
    fn model(&self) -> &str {
        "fake-model"
    }
    fn answer(&mut self, prompt: &Prompt) -> Result<String, PortError> {
        let key = prompt.item_keys()[0].clone();
        self.seen.push((prompt.digest(), key.clone(), prompt.text().to_string()));
        Ok(serde_json::json!({ "text": (self.f)(&key, prompt.text()) }).to_string())
    }
}

const WRITTEN: &str = "item:written:1";

fn english(key: &str) -> String {
    let mut table = table_english();
    table.insert(
        WRITTEN.into(),
        "Heaven endures through the long years without end.".into(),
    );
    table[key].clone()
}

/// The packet line's stance, as the prompt prints it: the packet line is one
/// JSON object ending in the stance.
fn stance_of(text: &str) -> String {
    let line = text.lines().find(|l| l.starts_with("packet: ")).unwrap();
    line[line.find("\"stance\":").unwrap()..].to_string()
}

/// Says the same thing whatever the stance.
fn blind(key: &str, _text: &str) -> String {
    english(key)
}

/// Appends the stance it was shown.
fn reader(key: &str, text: &str) -> String {
    format!("{} {}", english(key), stance_of(text))
}

/// Does better with the item's own stance: the real sentence for it, nonsense
/// for any other, empty included.
fn helped(key: &str, text: &str) -> String {
    let r800 = r#""stance":{"features":[{"feature":"feature:18","value":{"Grade":800}}]}}"#;
    let own = match key {
        "item:daodejing:3" => r#""stance":{"features":[{"feature":"feature:18","value":{"Grade":800}},{"feature":"feature:16","value":{"Grade":300}}]}}"#,
        WRITTEN => r#""stance":{"features":[{"feature":"feature:18","value":{"Grade":500}}]}}"#,
        _ => r800,
    };
    if stance_of(text) == own { english(key) } else { "zzz qqq xxx".into() }
}

fn grade_stance(features: &[(&str, u16)]) -> weksa::packet::Stance {
    weksa::packet::Stance {
        features: features
            .iter()
            .map(|(f, v)| weksa::packet::FeatureValue {
                feature: f.parse().unwrap(),
                value: weksa::packet::FeatValue::Grade(common::grade(*v)),
            })
            .collect(),
    }
}

/// A Dao run with a third Validation item (`item:written:1`, key order after
/// items 2 and 3) and round 1 recorded over it, every item encoded to its Dao
/// packet. Stances: item 1 restraint 800; item 2 restraint 800; item 3
/// restraint 800 and affection 300; the written item restraint 500.
fn recorded_dao(edit: impl FnOnce(&mut HashMap<String, String>)) -> (Dao, RoundRecord) {
    recorded_dao_in(|_, _| {}, edit)
}

/// `recorded_dao` with `corpus_edit` run over the corpus and entities first.
fn recorded_dao_in(
    corpus_edit: impl FnOnce(&mut CorpusStore, &weksa::EntityStore),
    edit: impl FnOnce(&mut HashMap<String, String>),
) -> (Dao, RoundRecord) {
    let mut dao = dao_over(|corpus, entities| {
        let entities = weksa::EntityStore::open(entities).unwrap();
        let mut store = CorpusStore::open(corpus).unwrap();
        corpus_edit(&mut store, &entities);
        let mut written = store.get("item:daodejing:2").unwrap().unwrap();
        written.id = WRITTEN.into();
        written.text = "Heaven endures through the long years without end.".into();
        written.source = weksa::corpus::Source::OperatorWritten;
        store.add(written, &entities).unwrap();
    });
    let mut model = fake(&good_answers());
    let mut extra = draft_of(2);
    extra.referents.truncate(1);
    extra.predications.truncate(1);
    extra.stance = grade_stance(&[("feature:18", 500)]);
    model.answers.insert(WRITTEN.into(), json(&extra));
    edit(&mut model.answers);
    model.lowered.insert(WRITTEN.into(), english(WRITTEN));
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    (dao, r)
}

fn ablate(
    dao: &Dao,
    recorded: &mut dyn ModelPort,
    model: &mut dyn ModelPort,
) -> Result<weksa::run::AblationOutcome, RunError> {
    dao.run.stance_ablation(1, recorded, model, &Scorer::new(Bag))
}

fn read(
    outcome: weksa::run::AblationOutcome,
) -> Vec<(weksa::Split, weksa::run::AblationReading)> {
    match outcome {
        weksa::run::AblationOutcome::Read(r) => r.into_iter().map(|(_, split, x)| (split, x)).collect(),
        weksa::run::AblationOutcome::Unanswered { prompts } => panic!("unanswered {prompts:?}"),
    }
}

#[test]
fn recorded_answers_read_and_never_write() {
    use std::io::ErrorKind;
    use weksa::model::RecordedAnswers;
    let p = prompt();
    let dir = tempfile::tempdir().unwrap();
    let mut port = RecordedAnswers::new(dir.path().to_path_buf(), "answerer".into());
    assert_eq!(port.model(), "answerer");
    // A miss is Unanswered and the directory is as it was.
    assert_eq!(port.answer(&p), Err(PortError::Unanswered));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    // A directory that does not exist is a miss too, and is not made.
    let mut absent = RecordedAnswers::new(dir.path().join("nope"), "m".into());
    assert_eq!(absent.answer(&p), Err(PortError::Unanswered));
    assert!(!dir.path().join("nope").exists());
    // A hit is the file's text as written.
    let file = dir.path().join(format!("{}.answer", p.digest()));
    std::fs::write(&file, " {\"x\": 1}\n").unwrap();
    assert_eq!(port.answer(&p).unwrap(), " {\"x\": 1}\n");
    std::fs::write(&file, [0xff, 0xfe]).unwrap();
    assert_eq!(port.answer(&p), Err(PortError::Io(ErrorKind::InvalidData)));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn a_stance_ablation_asks_replay_drop_and_permute_for_train_and_validation_only() {
    let (dao, r) = recorded_dao(|_| {});
    let round_digests: Vec<String> = r.calls()[4..].iter().map(|c| c.prompt.clone()).collect();
    assert_eq!(round_digests.len(), 4, "the round lowered four items");
    let mut recorded_port = Lowerer::new(reader);
    let mut model = Lowerer::new(reader);
    read(ablate(&dao, &mut recorded_port, &mut model).unwrap());

    // The base prompts are the round's own, byte for byte.
    let recorded_digests = recorded_port.digests();
    assert_eq!(recorded_digests, round_digests);
    let asked = model.digests();
    for d in &round_digests {
        assert!(asked.contains(d), "the replay asks the round's prompt {d}");
    }
    // Per item: replay and drop for each; permute for each Validation item, and
    // none for the lone Train item.
    let per_key = |key: &str| model.seen.iter().filter(|s| s.1 == key).count();
    assert_eq!(per_key("item:daodejing:1"), 2);
    for key in ["item:daodejing:2", "item:daodejing:3", WRITTEN] {
        assert_eq!(per_key(key), 3, "{key}");
    }
    // Test items are never asked.
    for (_, key, text) in &model.seen {
        assert!(key != "item:daodejing:4" && key != "item:daodejing:5", "{key}");
        assert!(!text.contains("item:daodejing:4"));
    }
    // A drop prompt prints the empty stance, a replay the item's own.
    let empty = model
        .seen
        .iter()
        .filter(|s| stance_of(&s.2) == "\"stance\":{\"features\":[]}}")
        .count();
    assert_eq!(empty, 4, "one drop per item");
}

#[test]
fn a_lowerer_blind_to_stance_reads_no_movement_and_one_that_reads_it_does() {
    let (dao, _) = recorded_dao(|_| {});
    let one = |f: fn(&str, &str) -> String| {
        let (mut rec, mut model) = (Lowerer::new(f), Lowerer::new(f));
        read(ablate(&dao, &mut rec, &mut model).unwrap())
    };
    for (split, r) in one(blind) {
        assert_eq!(r.unusable, 0);
        assert_eq!((r.drop.moved, r.permute.moved), (0, 0), "{split:?}: {}", r.line());
        assert!((r.replay.chrf.unwrap() - 100.0).abs() < 1e-9);
        assert!((r.drop.chrf.unwrap() - r.replay.chrf.unwrap()).abs() < 1e-9);
        if r.permute.n > 0 {
            assert!((r.permute.chrf.unwrap() - r.replay.chrf.unwrap()).abs() < 1e-9);
        }
        assert_eq!(r.drop.ref_gain_chrf, Some(0.0));
    }
    let reading = one(reader);
    let (train, validation) = (&reading[0].1, &reading[1].1);
    assert_eq!((reading[0].0, reading[1].0), (weksa::Split::Train, weksa::Split::Validation));
    assert_eq!((train.n, validation.n), (1, 3));
    assert_eq!((train.drop.n, train.drop.moved), (1, 1));
    assert_eq!((train.permute.n, train.permute.moved), (0, 0));
    assert_eq!((validation.drop.n, validation.drop.moved), (3, 3));
    assert_eq!((validation.permute.n, validation.permute.moved), (3, 3));
    // A lone Train item has no permute: no measurement, not total disagreement.
    assert_eq!(
        train.permute,
        weksa::run::VariantReading::default(),
        "{}",
        train.line()
    );
    assert!(
        train.line().contains(
            "| permute n=0 chrf=none meaning=none moved=0 ref_gain_chrf=none ref_gain_meaning=none"
        ),
        "{}",
        train.line()
    );
    assert!((validation.replay.chrf.unwrap() - 100.0).abs() < 1e-9);
    assert!(validation.drop.chrf.unwrap() < validation.replay.chrf.unwrap());
    assert!(validation.permute.chrf.unwrap() < validation.replay.chrf.unwrap());
}

#[test]
fn the_item_s_own_stance_helps_fidelity_only_for_a_lowerer_that_uses_it() {
    let (dao, _) = recorded_dao(|_| {});
    let one = |f: fn(&str, &str) -> String| {
        let (mut rec, mut model) = (Lowerer::new(f), Lowerer::new(f));
        read(ablate(&dao, &mut rec, &mut model).unwrap())
    };
    for (split, r) in one(helped) {
        assert!(r.drop.ref_gain_chrf.unwrap() > 10.0, "{split:?}: {}", r.line());
        assert!(r.drop.ref_gain_meaning.unwrap() > 0.0, "{split:?}: {}", r.line());
        if r.permute.n > 0 {
            assert!(r.permute.ref_gain_chrf.unwrap() > 10.0, "{split:?}: {}", r.line());
        }
        assert_eq!(r.drop.moved, r.drop.n);
    }
    for (split, r) in one(blind) {
        assert_eq!((r.drop.ref_gain_chrf, r.drop.ref_gain_meaning), (Some(0.0), Some(0.0)), "{split:?}");
    }
}

#[test]
fn a_packet_without_stance_asks_no_drop_and_a_test_packet_in_the_round_is_never_asked() {
    let (dao, _) = recorded_dao(|answers| {
        let mut d = draft_of(3);
        d.stance = Default::default();
        answers.insert("item:daodejing:3".into(), json(&d));
    });
    // A packet for a Test item, put into the round's own store the way no round
    // would: the ablation reads splits from the corpus, not from the store.
    {
        let corpus = CorpusStore::open(&dao.corpus).unwrap();
        let inv = dao.run.inventory();
        let mut packet = packets().remove(3);
        packet.inventory_rev = 3;
        PacketStore::open(&dao.root.join("round-1/packets.cc"))
            .unwrap()
            .admit(packet, inv, &corpus, dao.run.entities())
            .unwrap();
    }
    let (mut rec, mut model) = (Lowerer::new(reader), Lowerer::new(reader));
    read(ablate(&dao, &mut rec, &mut model).unwrap());
    let bare = "\"stance\":{\"features\":[]}}";
    let drops_of_3 = model
        .seen
        .iter()
        .filter(|s| s.1 == "item:daodejing:3" && stance_of(&s.2) == bare)
        .count();
    // Its replay already shows the empty stance; there is no drop beside it.
    assert_eq!(drops_of_3, 1);
    assert!(model.seen.iter().all(|s| s.1 != "item:daodejing:4"), "a Test item is never asked");
    assert!(rec.seen.iter().all(|s| s.1 != "item:daodejing:4"));
}

#[test]
fn permute_takes_the_next_items_stance_in_its_split_and_skips_equal_ones() {
    let (dao, _) = recorded_dao(|_| {});
    let (mut rec, mut model) = (Lowerer::new(reader), Lowerer::new(reader));
    read(ablate(&dao, &mut rec, &mut model).unwrap());
    // The stance each Validation item's permute prompt shows: the next item's
    // in key order (2, 3, written:1), cyclic; item 1 is alone in Train.
    let stance = |text: &str| stance_of(text);
    let of = |key: &str| -> Vec<String> {
        model
            .seen
            .iter()
            .filter(|s| s.1 == key)
            .map(|s| stance(&s.2))
            .collect()
    };
    let (s2, s3, sw) = (
        stance(&rec.seen.iter().find(|s| s.1 == "item:daodejing:2").unwrap().2),
        stance(&rec.seen.iter().find(|s| s.1 == "item:daodejing:3").unwrap().2),
        stance(&rec.seen.iter().find(|s| s.1 == WRITTEN).unwrap().2),
    );
    assert!(s2 != s3 && s3 != sw && sw != s2);
    assert!(of("item:daodejing:2").contains(&s3));
    assert!(of("item:daodejing:3").contains(&sw));
    assert!(of(WRITTEN).contains(&s2), "the last wraps to the first");
    assert!(!of("item:daodejing:2").contains(&sw), "not the previous or the other");
    // Item 1 is alone in Train: it never takes a stance across splits.
    assert_eq!(of("item:daodejing:1").len(), 2, "replay and drop only");

    // Items 2 and 3 with equal stances: item 2's permute is its own prompt, so
    // it asks none; the others still do.
    let (dao, _) = recorded_dao(|answers| {
        let mut d = draft_of(3);
        d.stance = draft_of(2).stance;
        answers.insert("item:daodejing:3".into(), json(&d));
    });
    let (mut rec, mut model) = (Lowerer::new(reader), Lowerer::new(reader));
    let r = read(ablate(&dao, &mut rec, &mut model).unwrap());
    let per_key = |key: &str| model.seen.iter().filter(|s| s.1 == key).count();
    assert_eq!(per_key("item:daodejing:2"), 2, "replay and drop: its next holds the stance it has");
    assert_eq!(per_key("item:daodejing:3"), 3);
    assert_eq!(per_key(WRITTEN), 3);
    assert_eq!(r[1].1.permute.n, 2, "{}", r[1].1.line());
}

#[test]
fn a_stance_ablation_records_nothing_and_writes_nothing_in_its_round() {
    use weksa::model::RecordedAnswers;
    let (dao, r) = recorded_dao(|_| {});
    let round1 = dao.root.join("round-1");
    for c in &r.calls()[4..] {
        std::fs::write(
            round1.join(format!("{}.answer", c.prompt)),
            serde_json::json!({ "text": english(&c.items[0]) }).to_string(),
        )
        .unwrap();
    }
    let snapshot = |dir: &Path| -> Vec<(String, Vec<u8>)> {
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                let bytes = if e.path().is_dir() { vec![] } else { std::fs::read(e.path()).unwrap() };
                (e.file_name().to_string_lossy().into_owned(), bytes)
            })
            .collect();
        files.sort();
        files.retain(|f| f.0 != "ablation-1");
        files
    };
    let before = (snapshot(&round1), snapshot(&dao.root), dao.run.rounds().unwrap(), dao.run.decisions().unwrap());
    let go = || {
        let mut rec = RecordedAnswers::new(round1.clone(), "fake-model".into());
        let mut files = weksa::model::AnswerFiles::new(dao.run.ablation_dir(1), "fake-model".into(), vec![]);
        // Nothing answers yet: the replay and variant prompts are written.
        dao.run.stance_ablation(1, &mut rec, &mut files, &Scorer::new(Bag))
    };
    let weksa::run::AblationOutcome::Unanswered { prompts } = go().unwrap() else {
        panic!("nobody answered")
    };
    // Four replays (the round's own prompts), four drops, three permutes (item 1
    // is alone in Train).
    assert_eq!(prompts.len(), 4 + 4 + 3, "{prompts:?}");
    for d in &prompts {
        assert!(dao.run.ablation_dir(1).join(format!("{d}.prompt")).is_file());
    }
    // Answer every ask from a fake and read; then again over the same files.
    for d in &prompts {
        let text = std::fs::read_to_string(dao.run.ablation_dir(1).join(format!("{d}.prompt"))).unwrap();
        std::fs::write(
            dao.run.ablation_dir(1).join(format!("{d}.answer")),
            serde_json::json!({ "text": format!("answer {}", &text[..8]) }).to_string(),
        )
        .unwrap();
    }
    let first = read(go().unwrap());
    let again = read(go().unwrap());
    assert_eq!(first, again);
    let after = (snapshot(&round1), snapshot(&dao.root), dao.run.rounds().unwrap(), dao.run.decisions().unwrap());
    assert!(before.0 == after.0, "round-1 is unchanged byte for byte");
    assert!(before.1 == after.1, "the run root's files are unchanged");
    assert_eq!(before.2, after.2);
    assert_eq!(before.3, after.3);
}

#[test]
fn a_stance_ablation_refuses_a_cycle_round_and_missing_round_answers() {
    use weksa::model::RecordedAnswers;
    let (dao, _) = recorded_dao(|_| {});
    // No round-2, and round 1 holds no answer files.
    let mut model = Lowerer::new(reader);
    let mut rec = RecordedAnswers::new(dao.root.join("round-1"), "fake-model".into());
    assert!(matches!(
        dao.run.stance_ablation(2, &mut rec, &mut model, &Scorer::new(Bag)),
        Err(RunError::Malformed("round"))
    ));
    let Err(missing) = ablate(&dao, &mut rec, &mut model) else {
        panic!("a missing round answer is refused")
    };
    assert!(matches!(missing, RunError::Unrecorded));
    // It says what happened, not that a store would not read.
    let said = missing.to_string();
    assert!(said.contains("holds no answer") && said.contains("digest") && said.contains("head inventory"), "{said}");
    assert!(!said.contains("would not read"), "{said}");
    assert!(
        std::fs::read_dir(dao.root.join("round-1"))
            .unwrap()
            .all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".prompt")),
        "no prompt is written into the round"
    );
    // Drift is refused as a round refuses it.
    struct Named(Lowerer);
    impl ModelPort for Named {
        fn model(&self) -> &str {
            "other-model"
        }
        fn answer(&mut self, p: &Prompt) -> Result<String, PortError> {
            self.0.answer(p)
        }
    }
    let mut named = Named(Lowerer::new(reader));
    let mut rec2 = Lowerer::new(reader);
    assert!(matches!(
        ablate(&dao, &mut rec2, &mut named),
        Err(RunError::Drift(DriftOf::Model))
    ));
    assert!(matches!(
        dao.run.stance_ablation(1, &mut rec2, &mut Lowerer::new(reader), &Scorer::new(OtherEmbedder)),
        Err(RunError::Drift(DriftOf::Embedder))
    ));
}

#[test]
fn a_refused_stance_ablation_creates_no_folder_and_wipes_no_store() {
    let (dao, _) = recorded_dao(|_| {});
    let (mut rec, mut model) = (Lowerer::new(reader), Lowerer::new(reader));
    // No round 99: no folder appears.
    assert!(matches!(
        dao.run.stance_ablation(99, &mut rec, &mut model, &Scorer::new(Bag)),
        Err(RunError::Malformed("round"))
    ));
    assert!(!dao.run.ablation_dir(99).exists(), "a refused ablation makes no folder");
    // An earlier ablation of a refused n keeps its stores byte for byte.
    let kept = dao.run.ablation_dir(2);
    std::fs::create_dir_all(&kept).unwrap();
    std::fs::write(kept.join("base.cc"), b"earlier ablation").unwrap();
    assert!(matches!(
        dao.run.stance_ablation(2, &mut rec, &mut model, &Scorer::new(Bag)),
        Err(RunError::Malformed("round"))
    ));
    assert_eq!(std::fs::read(kept.join("base.cc")).unwrap(), b"earlier ablation");
    assert_eq!(std::fs::read_dir(&kept).unwrap().count(), 1);
}

#[test]
fn a_stance_ablation_refuses_a_cycle_round() {
    let mut dao = dao();
    two_rounds(&mut dao);
    run_cycle(&mut dao, &mut cycle_fake(ONE_CONCEPT));
    let rounds = dao.run.rounds().unwrap();
    let candidate = rounds.iter().find(|r| r.cycle().is_some()).expect("the cycle's round");
    let (mut rec, mut model) = (Lowerer::new(reader), Lowerer::new(reader));
    assert!(matches!(
        dao.run.stance_ablation(candidate.n(), &mut rec, &mut model, &Scorer::new(Bag)),
        Err(RunError::Malformed("round"))
    ));
    assert!(model.seen.is_empty() && rec.seen.is_empty());
}

/// The real sentence for the item's own stance, an empty text (not a usable
/// lowering) for any other.
fn mute_unless_own(key: &str, text: &str) -> String {
    let said = helped(key, text);
    if said == "zzz qqq xxx" { String::new() } else { said }
}

#[test]
fn unusable_variant_answers_are_counted_and_read_as_none_not_zero() {
    let (dao, _) = recorded_dao(|_| {});
    let (mut rec, mut model) = (Lowerer::new(mute_unless_own), Lowerer::new(mute_unless_own));
    let reading = read(ablate(&dao, &mut rec, &mut model).unwrap());
    let (train, validation) = (&reading[0].1, &reading[1].1);
    // Every variant answer is unusable: Train has one drop, Validation three
    // drops and three permutes. The items themselves (replay) still read.
    assert_eq!((train.n, train.unusable), (1, 1));
    assert_eq!((validation.n, validation.unusable), (3, 6));
    for r in [train, validation] {
        assert!((r.replay.chrf.unwrap() - 100.0).abs() < 1e-9);
        assert_eq!(r.drop.n, 0);
        assert_eq!(r.permute.n, 0);
        assert_eq!(r.drop, weksa::run::VariantReading::default());
        assert_eq!(r.permute, weksa::run::VariantReading::default());
    }
    assert_eq!(
        train.line(),
        "ablation n=1 unusable=1 replay chrf=100.00 meaning=1.000 | drop n=0 chrf=none meaning=none moved=0 ref_gain_chrf=none ref_gain_meaning=none | permute n=0 chrf=none meaning=none moved=0 ref_gain_chrf=none ref_gain_meaning=none"
    );
}

#[test]
fn the_line_names_each_variant_with_its_own_n_and_the_splits_n() {
    use weksa::run::{AblationReading, Agreement, VariantReading};
    let reading = AblationReading {
        n: 7,
        unusable: 2,
        replay: Agreement { chrf: Some(91.5), meaning: Some(0.9) },
        drop: VariantReading {
            n: 5,
            chrf: Some(80.25),
            meaning: Some(0.8),
            moved: 4,
            ref_gain_chrf: Some(3.5),
            ref_gain_meaning: Some(-0.125),
        },
        permute: VariantReading {
            n: 3,
            chrf: Some(70.0),
            meaning: None,
            moved: 1,
            ref_gain_chrf: None,
            ref_gain_meaning: Some(0.5),
        },
    };
    assert_eq!(
        reading.line(),
        "ablation n=7 unusable=2 replay chrf=91.50 meaning=0.900 | drop n=5 chrf=80.25 meaning=0.800 moved=4 ref_gain_chrf=3.50 ref_gain_meaning=-0.125 | permute n=3 chrf=70.00 meaning=none moved=1 ref_gain_chrf=none ref_gain_meaning=0.500"
    );
}

fn nothing_usable(_: &str, _: &str) -> String {
    String::new()
}

#[test]
fn a_split_where_no_replay_is_usable_reads_none_throughout() {
    let (dao, _) = recorded_dao(|_| {});
    let (mut rec, mut model) = (Lowerer::new(reader), Lowerer::new(nothing_usable));
    let reading = read(ablate(&dao, &mut rec, &mut model).unwrap());
    // Train: the item and its drop; Validation: three items, three drops, three permutes.
    assert_eq!((reading[0].1.n, reading[0].1.unusable), (0, 2));
    assert_eq!((reading[1].1.n, reading[1].1.unusable), (0, 9));
    for (_, r) in &reading {
        assert_eq!(r.replay, weksa::run::Agreement::default());
        assert_eq!(r.drop, weksa::run::VariantReading::default());
        assert_eq!(r.permute, weksa::run::VariantReading::default());
    }
    assert_eq!(
        reading[0].1.line(),
        "ablation n=0 unusable=2 replay chrf=none meaning=none | drop n=0 chrf=none meaning=none moved=0 ref_gain_chrf=none ref_gain_meaning=none | permute n=0 chrf=none meaning=none moved=0 ref_gain_chrf=none ref_gain_meaning=none"
    );
}

// ---- the panel: each item is lowered in its own language ----

const JA_TEXTS: [(&str, &str, usize); 3] = [
    ("item:daodejing:11", "賢者は民の心を空にし、腹を満たし、志を弱め、骨を強くする。", 1),
    ("item:daodejing:12", "天は長く、地は久しい。", 2),
    ("item:daodejing:13", "上善は水のようである。", 3),
];

/// The Dao corpus with a ja-JP twin of each live item (same group, so the same
/// split) and a pt-BR twin of each; Laozi, the speaker the packets cite, gains
/// `ja` as a ja-JP form when given.
fn add_languages(corpus: &Path, entities: &Path, ja: Option<&str>) {
    let mut entities = weksa::EntityStore::open(entities).unwrap();
    if let Some(form) = ja {
        let mut laozi = entities.get(&common::eid("laozi")).unwrap().unwrap();
        laozi.forms.push(weksa::LanguageForms {
            language: "ja-JP".into(),
            forms: vec![form.into()],
            pronunciation: None,
        });
        entities.put(laozi).unwrap();
    }
    let mut store = CorpusStore::open(corpus).unwrap();
    for (key, text, twin) in JA_TEXTS {
        let mut item = store.get(&format!("item:daodejing:{twin}")).unwrap().unwrap();
        item.id = key.into();
        item.text = text.into();
        item.language = "ja-JP".into();
        item.names = vec![];
        item.prior_turns = vec![];
        store.add(item, &entities).unwrap();
        let mut pt = store.get(&format!("item:daodejing:{twin}")).unwrap().unwrap();
        pt.id = format!("item:daodejing:2{twin}");
        pt.text = format!("PORTUGUESEMARK {twin} o sabio esvazia os coracoes");
        pt.language = "pt-BR".into();
        pt.names = vec![];
        pt.prior_turns = vec![];
        store.add(pt, &entities).unwrap();
    }
}

/// The Dao draft of `item`, its first referent citing the speaker, Laozi.
fn citing_laozi(item: usize) -> String {
    let mut v = serde_json::to_value(draft_of(item)).unwrap();
    v["referents"][0]["entity"] = serde_json::json!("entity:laozi");
    v.to_string()
}

/// Encode answers for the en-US and ja-JP items, all citing the sage.
fn bilingual_answers() -> HashMap<String, String> {
    let mut answers = HashMap::new();
    for n in 1..=3 {
        answers.insert(format!("item:daodejing:{n}"), citing_laozi(n));
        answers.insert(format!("item:daodejing:1{n}"), citing_laozi(n));
    }
    answers
}

fn bilingual_fake() -> FakeModel {
    let mut model = fake(&[]);
    model.answers = bilingual_answers();
    for (key, text, _) in JA_TEXTS {
        model.lowered.insert(key.into(), text.into());
    }
    model
}

fn bilingual(ja: Option<&'static str>) -> Dao {
    dao_panel(
        |corpus, entities| add_languages(corpus, entities, ja),
        &["ja-JP", "en-US"],
    )
}

const LAOZI_JA: Option<&str> = Some("老子");

fn lowering_texts(model: &FakeModel) -> Vec<&str> {
    model
        .seen
        .iter()
        .map(|(_, t)| t.as_str())
        .filter(|t| t.starts_with(LOWER_INSTRUCTIONS))
        .collect()
}

#[test]
fn each_item_lowers_in_its_own_language() {
    let mut dao = bilingual(LAOZI_JA);
    assert_eq!(dao.run.record().panel, ["en-US", "ja-JP"], "sorted at open");
    let mut model = bilingual_fake();
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    let prompts = lowering_texts(&model);
    assert_eq!(prompts.len(), 6, "three items in each language");
    let (mut ja, mut en) = (0, 0);
    for text in prompts {
        if text.contains("\n\nlanguage: ja-JP\n\n") {
            ja += 1;
            assert!(text.contains(r#"{"entity:laozi":["老子"]}"#), "{text}");
            assert!(!text.contains("Laozi"), "{text}");
        } else {
            en += 1;
            assert!(text.contains("\n\nlanguage: en-US\n\n"), "{text}");
            assert!(text.contains(r#"{"entity:laozi":["Laozi"]}"#), "{text}");
            assert!(!text.contains("老子"), "{text}");
        }
    }
    assert_eq!((ja, en), (3, 3));
    // No lowering prompt, in either language, holds any item's text, key or group.
    let corpus = CorpusStore::open(&dao.corpus).unwrap();
    for text in lowering_texts(&model) {
        for item in corpus.all().unwrap() {
            assert!(!text.contains(&item.text), "{}", item.id);
            assert!(!text.contains(item.id.as_str()), "{}", item.id);
            assert!(!text.contains(item.group.as_str()), "{}", item.id);
        }
    }
    // A ja-JP item is scored against its own text: the lowering that is the
    // text reads as a perfect one.
    let ja = r.language("ja-JP").unwrap();
    assert_eq!((ja.train_scored, ja.validation_scored), (1, 2));
    assert_eq!(ja.validation_mean.chrf, Some(100.0));
    assert_eq!(ja.train_mean.meaning, Some(1.0));
}

#[test]
fn a_cited_entity_without_a_form_in_the_items_language_refuses_at_lower() {
    let mut dao = bilingual(None);
    let mut model = bilingual_fake();
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    let ja = r.language("ja-JP").unwrap();
    assert_eq!(ja.validation_refused.lower, 2);
    assert_eq!((ja.train_scored, ja.validation_scored), (0, 0));
    let refused: Vec<_> = r
        .train()
        .iter()
        .filter(|row| matches!(row.outcome, Outcome::Refused(Stage::Lower)))
        .collect();
    assert_eq!(refused.len(), 1, "the ja-JP Train item");
    assert_eq!(refused[0].item, "item:daodejing:11");
    let en = r.language("en-US").unwrap();
    assert_eq!((en.train_scored, en.validation_scored), (1, 2));
    assert_eq!(en.validation_refused, weksa::run::RefusalCounts::default());
    assert_eq!(lowering_texts(&model).len(), 3, "no prompt for a form that is missing");
}

#[test]
fn the_panel_filters_every_prompt() {
    let mut dao = bilingual(LAOZI_JA);
    let mut model = bilingual_fake();
    // Two rounds, a cycle (the editor's prompt), a recall: every kind of prompt.
    let mut last = None;
    for _ in 0..2 {
        last = Some(recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap()));
    }
    let tags = |r: &RoundRecord| -> Vec<String> {
        r.languages().iter().map(|l| l.language.clone()).collect()
    };
    assert_eq!(tags(&last.unwrap()), ["en-US", "ja-JP"]);
    model.candidate = Some((4, bilingual_answers()));
    model.edit = Some(Ok(ONE_CONCEPT.to_string()));
    run_cycle(&mut dao, &mut model);
    let mut recite = reciter(&dao, second_half);
    let reading = match dao.run.recall(&mut recite).unwrap() {
        weksa::run::RecallOutcome::Read(r) => r,
        _ => panic!("answered"),
    };
    let langs: Vec<&str> = reading.languages.iter().map(|l| l.language.as_str()).collect();
    assert_eq!(langs, ["en-US", "ja-JP"]);
    let shown = model.seen.iter().map(|(_, t)| t).chain(&recite.seen);
    let mut count = 0;
    for text in shown {
        count += 1;
        assert!(!text.contains("PORTUGUESE"), "a pt-BR item reached a prompt");
        for n in 1..=3 {
            assert!(!text.contains(&format!("item:daodejing:2{n}")));
        }
    }
    assert!(count > 20, "{count}");
    assert!(!mint_texts(&model).is_empty());
    assert_eq!(dao.run.train().unwrap().len(), 2, "one Train item per panel language");
}

#[test]
fn open_refuses_a_bad_panel() {
    let train_only = |corpus: &Path, entities: &Path| {
        let entities = weksa::EntityStore::open(entities).unwrap();
        let mut store = CorpusStore::open(corpus).unwrap();
        let mut item = store.get("item:daodejing:1").unwrap().unwrap();
        item.id = "item:daodejing:31".into();
        item.text = "Un texte francais.".into();
        item.language = "fr-FR".into();
        item.names = vec![];
        item.prior_turns = vec![];
        store.add(item, &entities).unwrap();
    };
    let dao = dao_over(train_only);
    let (corpus, inventory, entities) = (&dao.corpus, &dao.inventory, &dao.entities);
    drop(dao.run);
    let tag = |s: &str| s.to_string();
    for panel in [
        vec![],
        vec![tag("en-US"), tag("en-US")],
        vec![tag("en-US"), tag("not a tag!")],
        vec![tag("en-us")],
        // No live item at all, and a Train item without a Validation one.
        vec![tag("en-US"), tag("de-DE")],
        vec![tag("en-US"), tag("fr-FR")],
    ] {
        let root = dao._dir.path().join(format!("panel-{}", panel.len()));
        let created = RunStores::create(
            &root,
            "p",
            RunPaths { corpus, inventory, entities, ontology: &dao.ontology, speaker_state: &common::speaker_state_beside(&dao.ontology) },
            "fake-model",
            "bag",
            weksa::run::LowererReach::ToolLessRelay,
            ModelFamily::Unrecorded,
            &panel,
        );
        let err = created.err();
        assert_eq!(err, Some(RunError::Panel), "{panel:?}");
        let shown = err.unwrap().to_string();
        for t in &panel {
            assert!(!shown.contains(t.as_str()), "{shown}");
        }
        assert!(!root.join("run.cc").exists(), "{panel:?}");
        assert!(!root.join("inventory.cc").exists(), "{panel:?}");
    }
}

#[test]
fn round_means_are_per_language() {
    let mut dao = bilingual(LAOZI_JA);
    let mut model = bilingual_fake();
    // One ja-JP Validation item is not an encodable draft: it counts 0.
    model.answers.insert("item:daodejing:13".into(), "not a draft".into());
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    let ja = r.language("ja-JP").unwrap();
    // Validation: item 12 reads perfectly (chrF 100, meaning 1), item 13 is 0.
    assert_eq!(ja.validation_mean.count, 2);
    assert_eq!(ja.validation_mean.chrf, Some(50.0));
    assert_eq!(ja.validation_mean.meaning, Some(0.5));
    assert_eq!((ja.validation_scored, ja.validation_refused.parse), (1, 1));
    assert_eq!(ja.train_mean.chrf, Some(100.0));

    // en-US reads exactly as it does when it is the whole panel: the ja-JP
    // items touch none of its numbers.
    let mut alone = dao_over(|_, _| {});
    let mut plain = fake(&good_answers());
    let r2 = recorded(round(&mut alone.run, &mut plain, &Scorer::new(Bag)).unwrap());
    assert_eq!(r.language("en-US"), r2.language("en-US"));
    assert_eq!(r.languages().len(), 2);
    assert_eq!(r2.languages().len(), 1);
}

#[test]
fn floor_and_feedback_are_per_language() {
    let mut dao = bilingual(LAOZI_JA);
    // Round 1: everything lowers perfectly in ja-JP. Round 2: one ja-JP
    // Validation item is refused. en-US reads the same twice.
    let mut first = bilingual_fake();
    recorded(round(&mut dao.run, &mut first, &Scorer::new(Bag)).unwrap());
    let mut second = bilingual_fake();
    second.answers.insert("item:daodejing:13".into(), "not a draft".into());
    recorded(round(&mut dao.run, &mut second, &Scorer::new(Bag)).unwrap());

    let mut model = bilingual_fake();
    model.candidate = Some((4, bilingual_answers()));
    model.edit = Some(Ok(ONE_CONCEPT.to_string()));
    let d = run_cycle(&mut dao, &mut model);
    let rows = d.panel();
    assert_eq!(
        rows.iter().map(|r| r.language.as_str()).collect::<Vec<_>>(),
        ["en-US", "ja-JP"]
    );
    let (en, ja) = (&rows[0], &rows[1]);
    assert_eq!(en.floor.composite, 0.0, "en-US read the same twice");
    assert_eq!(en.threshold, LADDER_STEP);
    assert_eq!(ja.floor.chrf, 50.0);
    assert_eq!(ja.floor.meaning, 0.5);
    assert_eq!(ja.floor.composite, 0.5);
    assert_eq!(ja.threshold, 0.5);
    assert_eq!(ja.base.composite, 1.0);

    // The editor's one held-out number is each language's head composite on
    // the ladder, in panel order.
    let rounded = |x: f64| (x / LADDER_STEP).round() * LADDER_STEP;
    let line = format!(
        "last edit: none; validation composite at head: en-US {:.3}, ja-JP {:.3}",
        rounded(en.base.composite),
        rounded(ja.base.composite)
    );
    let mint = mint_texts(&model);
    assert!(mint[0].contains(&format!("\n\n{line}\n\n")), "{}", mint[0]);
}

#[test]
fn recall_probes_unspaced_lines() {
    let dao = bilingual(LAOZI_JA);
    let mut recite = reciter(&dao, second_half);
    let reading = match dao.run.recall(&mut recite).unwrap() {
        weksa::run::RecallOutcome::Read(r) => r,
        _ => panic!("answered"),
    };
    let ja = reading.languages.iter().find(|l| l.language == "ja-JP").unwrap();
    // Three lines written without spaces were skipped before.
    assert_eq!((ja.n, ja.answered), (3, 3));
    assert!((ja.chrf - 100.0).abs() < 1e-9);
    let corpus = CorpusStore::open(&dao.corpus).unwrap();
    for (key, ja_text, _) in JA_TEXTS {
        let prompt = recite
            .seen
            .iter()
            .find(|t| shown_opening(t).chars().next() == ja_text.chars().next())
            .unwrap();
        let item = corpus.get(key).unwrap().unwrap();
        let (head, tail) = (shown_opening(prompt), held_back(&item.text, prompt));
        assert!(!head.is_empty() && !tail.is_empty());
        assert_eq!(format!("{head}{tail}"), item.text, "no spaces to lose");
        let n = item.text.len();
        assert!((n / 4..=3 * n / 4).contains(&head.len()), "{key}: near the byte middle");
        assert!(!prompt.contains(tail));
    }
    // The unrelated-text floor was measured for English only: its clause
    // prints once, on the en-US segment.
    let line = reading.line();
    assert_eq!(line.matches("unrelated English").count(), 1, "{line}");
    let (first, second) = line.split_once(" | ").unwrap();
    assert!(first.starts_with("recall en-US") && first.contains("unrelated English"), "{line}");
    assert!(second.starts_with("recall ja-JP") && !second.contains("unrelated"), "{line}");
    // en-US still splits between words.
    let en = reading.languages.iter().find(|l| l.language == "en-US").unwrap();
    assert_eq!((en.n, en.answered), (3, 3));
    for key in ["item:daodejing:1", "item:daodejing:2", "item:daodejing:3"] {
        let item = corpus.get(key).unwrap().unwrap();
        let prompt = recite
            .seen
            .iter()
            .find(|t| t.contains("opening: ") && item.text.starts_with(shown_opening(t)))
            .unwrap();
        let head = shown_opening(prompt);
        let next = item.text[head.len()..].chars().next().unwrap();
        assert!(!next.is_alphanumeric(), "{key}: cut inside a word");
    }
}

#[test]
fn offer_by_key_is_the_panels_too() {
    let dao = bilingual(LAOZI_JA);
    assert!(dao.run.offer("item:daodejing:12").unwrap().is_ok(), "ja-JP is in the panel");
    assert!(dao.run.offer("item:daodejing:2").unwrap().is_ok());
    for pt in ["item:daodejing:21", "item:daodejing:22", "item:daodejing:23"] {
        assert!(dao.run.offer(pt).is_err(), "{pt}: pt-BR is outside the panel");
    }
}

#[test]
fn a_cycle_decides_each_language_on_its_own_floor() {
    let mut dao = bilingual(LAOZI_JA);
    // en-US reads the same twice (floor 0); ja-JP loses a Validation item in
    // round 2 (a wide floor).
    let mut first = bilingual_fake();
    recorded(round(&mut dao.run, &mut first, &Scorer::new(Bag)).unwrap());
    let mut second = bilingual_fake();
    second.answers.insert("item:daodejing:13".into(), "not a draft".into());
    recorded(round(&mut dao.run, &mut second, &Scorer::new(Bag)).unwrap());
    // The candidate degrades en-US a little and leaves ja-JP as it was.
    let mut model = bilingual_fake();
    model.candidate = Some((4, {
        let mut candidate = bilingual_answers();
        candidate.extend(keyed(&degraded_answers()));
        candidate
    }));
    model.edit = Some(Ok(ONE_CONCEPT.to_string()));
    let d = run_cycle(&mut dao, &mut model);
    let (en, ja) = (&d.panel()[0], &d.panel()[1]);
    assert_eq!((en.language.as_str(), ja.language.as_str()), ("en-US", "ja-JP"));
    assert_eq!(en.floor.chrf, 0.0);
    assert_eq!(ja.floor.chrf, 50.0);
    // en-US fell by more than its own floor but by less than ja-JP's: the
    // verdict is the rejection that en-US's floor makes, not NoGain.
    let fall = en.base.chrf - en.candidate.unwrap().chrf;
    assert!(fall > 1.0 && fall < ja.floor.chrf, "{fall}");
    assert!(matches!(d.verdict(), Verdict::Rejected(Why::Regressed(_))), "{:?}", d.verdict());
}

#[test]
fn ablation_reads_each_language_apart() {
    let mut dao = bilingual(LAOZI_JA);
    let mut model = bilingual_fake();
    recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    let (mut rec, mut replay) = (bilingual_fake(), bilingual_fake());
    let readings = match ablate(&dao, &mut rec, &mut replay).unwrap() {
        weksa::run::AblationOutcome::Read(r) => r,
        _ => panic!("answered"),
    };
    let shape: Vec<(&str, weksa::Split, u32)> = readings
        .iter()
        .map(|(l, split, r)| (l.as_str(), *split, r.n))
        .collect();
    use weksa::Split::{Train, Validation};
    assert_eq!(
        shape,
        [("en-US", Train, 1), ("en-US", Validation, 2), ("ja-JP", Train, 1), ("ja-JP", Validation, 2)]
    );
}

/// A Dao ontology for en-US and ja-JP: entry 1 of each for a stance with
/// affection, and a ja-JP entry 2 for almost no restraint (which no Dao packet
/// has).
fn dao_ontology(path: &Path, inventory: &Path) {
    use weksa::packet::Grade;
    use weksa::{Condition, Layer, OntologyEntry, OntologyStore, Slot, Test};
    let inv = InventoryStore::open(inventory).unwrap();
    let feature = |gloss: &str| {
        inv.inventory()
            .unwrap()
            .entries()
            .find(|e| e.gloss == gloss)
            .unwrap()
            .id
    };
    let entry = |language: &str, n, gloss: &str, test, choice: &str| OntologyEntry {
        language: language.into(),
        n,
        inventory_rev: 1,
        layer: Layer::Discourse,
        when: vec![Condition {
            slot: Slot::Stance,
            feature: feature(gloss),
            test,
        }],
        choice: choice.into(),
        examples: vec![],
        annotator: weksa::Annotator::Operator,
    };
    let affection = || Test::AtLeast(Grade::new(200).unwrap());
    OntologyStore::open(path)
        .unwrap()
        .admit(
            vec![
                entry("en-US", 1, "affection", affection(), "ENWARM"),
                entry("ja-JP", 1, "affection", affection(), "JAWARM"),
                entry(
                    "ja-JP",
                    2,
                    "restraint",
                    Test::AtMost(Grade::new(100).unwrap()),
                    "JABLUNT",
                ),
            ],
            &inv,
        )
        .unwrap();
}

fn inv_store(path: &Path) -> InventoryStore {
    InventoryStore::open(path).unwrap()
}

#[test]
fn a_run_pins_its_ontology_and_refuses_drift() {
    let dao = dao_build(|_, _| {}, &["en-US"], dao_ontology);
    let r = dao.run.record();
    assert_eq!(r.ontology, std::fs::canonicalize(&dao.ontology).unwrap().to_str().unwrap());
    assert_eq!(r.ontology_digest, hex(&dao.ontology));
    let root = dao.root.clone();
    assert_eq!(RunStores::open(&root).unwrap().record(), dao.run.record());
    // Another entry after the pin is another ontology.
    let entry = weksa::OntologyStore::open(&dao.ontology).unwrap().all().unwrap().remove(0);
    let mut store = weksa::OntologyStore::open(&dao.ontology).unwrap();
    store
        .admit(
            vec![weksa::OntologyEntry { n: 2, ..entry }],
            &inv_store(&dao.inventory),
        )
        .unwrap();
    assert_eq!(
        RunStores::open(&root).err(),
        Some(RunError::Drift(DriftOf::Ontology))
    );
}

#[test]
fn create_refuses_an_entry_its_inventory_cannot_read() {
    use weksa::packet::Grade;
    use weksa::{Condition, Layer, OntologyEntry, OntologyStore, Slot, Test};
    // The entry reads feature 18 at revision 1; the run's head inventory is
    // another file where that feature is retired or absent, so the entry is
    // admitted against a richer inventory and refused against the run's.
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/daodejing");
    let make = |language: &str, dir: &Path, retire: bool| {
        for (from, to) in [("corpus.cc", "c.cc"), ("inventory.cc", "i.cc"), ("entity.cc", "e.cc")] {
            std::fs::copy(src.join(from), dir.join(to)).unwrap();
        }
        let ontology = dir.join("o.cc");
        // A richer inventory the entry is admitted against: the Dao one plus a feature.
        let rich = dir.join("rich.cc");
        std::fs::copy(src.join("inventory.cc"), &rich).unwrap();
        let mut store = InventoryStore::open(&rich).unwrap();
        let inv = store.inventory().unwrap();
        let head = inv.latest().unwrap().n;
        let n = head + 1;
        let next = std::num::NonZeroU32::new(
            inv.entries().filter(|e| e.id.family() == weksa::Family::Feature).map(|e| e.id.n().get()).max().unwrap() + 1,
        )
        .unwrap();
        let id = weksa::Family::Feature.id(next);
        store
            .admit_revision(
                weksa::InventoryRevision {
                    n,
                    parent: Some(head),
                    changes: vec![weksa::Change::Add(id)],
                    frozen: false,
                },
                vec![weksa::InventoryEntry {
                    id,
                    gloss: "pride".into(),
                    value_kind: Some(weksa::ValueKind::Grade),
                    applies_to: vec![Slot::Stance],
                    introduced_rev: n,
                    retired_rev: None,
                }],
            )
            .unwrap();
        OntologyStore::open(&ontology)
            .unwrap()
            .admit(
                vec![OntologyEntry {
                    language: language.into(),
                    n: 1,
                    inventory_rev: n,
                    layer: Layer::Lexicon,
                    when: vec![Condition {
                        slot: Slot::Stance,
                        feature: id,
                        test: Test::AtLeast(Grade::new(1).unwrap()),
                    }],
                    choice: "boast".into(),
                    examples: vec![],
                    annotator: weksa::Annotator::Operator,
                }],
                &store,
            )
            .unwrap();
        if retire {
            // The run's head inventory retires the feature after the entry's revision.
            store
                .admit_revision(
                    weksa::InventoryRevision {
                        n: n + 1,
                        parent: Some(n),
                        changes: vec![weksa::Change::Retire(id)],
                        frozen: false,
                    },
                    vec![],
                )
                .unwrap();
            std::fs::copy(&rich, dir.join("i.cc")).unwrap();
        }
        ontology
    };
    let create = |dir: &Path, ontology: &Path| {
        RunStores::create(
            &dir.join("run"),
            "x",
            RunPaths {
                corpus: &dir.join("c.cc"),
                inventory: &dir.join("i.cc"),
                entities: &dir.join("e.cc"),
                ontology,
                speaker_state: &common::speaker_state_beside(ontology),
            },
            "fake-model",
            "bag",
            weksa::run::LowererReach::ToolLessRelay,
            ModelFamily::Unrecorded,
            &["en-US".to_string()],
        )
    };
    let panel_dir = tempfile::tempdir().unwrap();
    let o = make("en-US", panel_dir.path(), false);
    assert_eq!(create(panel_dir.path(), &o).err(), Some(RunError::Ontology));
    assert!(!panel_dir.path().join("run/run.cc").exists());
    assert!(!panel_dir.path().join("run/inventory.cc").exists());
    // The same entry under a language outside the panel does not refuse.
    let other_dir = tempfile::tempdir().unwrap();
    let o = make("ja-JP", other_dir.path(), false);
    assert!(create(other_dir.path(), &o).is_ok());
    // An entry on a feature the head inventory has retired can never select.
    let retired_dir = tempfile::tempdir().unwrap();
    let o = make("en-US", retired_dir.path(), true);
    assert_eq!(create(retired_dir.path(), &o).err(), Some(RunError::Ontology));
    assert!(!retired_dir.path().join("run/run.cc").exists());
}

#[test]
fn round_lowering_prompts_carry_selected_entries() {
    let mut dao = dao_build(
        |corpus, entities| add_languages(corpus, entities, LAOZI_JA),
        &["ja-JP", "en-US"],
        dao_ontology,
    );
    let mut model = bilingual_fake();
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    let mut seen = std::collections::BTreeSet::new();
    for ((_, text), call) in model.seen.iter().zip(r.calls()) {
        if !text.starts_with(LOWER_INSTRUCTIONS) {
            continue;
        }
        let item = call.items[0].as_str();
        let (language, warm, other) = if text.contains("\n\nlanguage: ja-JP\n\n") {
            ("ja-JP", "JAWARM", "ENWARM")
        } else {
            ("en-US", "ENWARM", "JAWARM")
        };
        // Only the third packet of each language carries affection.
        if item.ends_with(":3") || item.ends_with(":13") {
            assert!(text.contains(&format!("\nonto:{language}:1 [discourse] {warm}\n")), "{text}");
        } else {
            assert!(text.contains("\n\nontology: none\n\n"), "{item}: {text}");
        }
        assert!(!text.contains(other) && !text.contains("JABLUNT"), "{item}: {text}");
        seen.insert(item.to_string());
    }
    assert_eq!(seen.len(), 6, "three items in each language");
}

/// Admits the speaker-state revision (rev 4) onto the rev-3 Dao inventory at
/// `inventory`, and the scene for the group of `item:daodejing:3`: Laozi's
/// relation to an unnamed party, above, intimacy 200.
fn put_dao_scene(speaker_state: &Path, inventory: &Path, corpus: &Path, entities: &Path) {
    use weksa::packet::{FeatValue, FeatureValue};
    let mut inv_store = InventoryStore::open(inventory).unwrap();
    let (rev, entries) = weksa::inventory::speaker_scene_revision(4);
    inv_store.admit_revision(rev, entries).unwrap();
    let inv = inv_store.inventory().unwrap();
    let id = |gloss: &str| inv.entries().find(|e| e.gloss == gloss).unwrap().id;
    let corpus = CorpusStore::open(corpus).unwrap();
    let item = corpus.get("item:daodejing:3").unwrap().unwrap();
    let scene = weksa::Scene {
        group: item.group.clone(),
        n: 1,
        inventory_rev: 4,
        participants: vec![item.speaker.clone()],
        relations: vec![weksa::PairRelation {
            from: item.speaker.clone(),
            to: weksa::Party::Unnamed,
            features: vec![
                FeatureValue {
                    feature: id("relative_status"),
                    value: FeatValue::Option(common::lab("above")),
                },
                FeatureValue {
                    feature: id("intimacy"),
                    value: FeatValue::Grade(common::grade(200)),
                },
            ],
        }],
        annotator: weksa::Annotator::Operator,
    };
    weksa::SpeakerStateStore::open(speaker_state)
        .unwrap()
        .put_scene(scene, &inv_store, &corpus, &weksa::EntityStore::open(entities).unwrap())
        .unwrap();
}

/// A Dao run whose store holds a scene for item 3's group and whose ontology
/// holds en-US:1, selected when the speaker is above the addressee.
fn scene_dao(extra: impl FnOnce(&Path, &Path)) -> Dao {
    dao_build(
        |_, _| {},
        &["en-US"],
        |ontology, inventory| {
            use weksa::{Condition, Layer, OntologyEntry, OntologyStore, Slot, Test};
            let dir = ontology.parent().unwrap();
            put_dao_scene(
                &common::speaker_state_beside(ontology),
                inventory,
                &dir.join("c.cc"),
                &dir.join("e.cc"),
            );
            let inv_store = InventoryStore::open(inventory).unwrap();
            let status = inv_store
                .inventory()
                .unwrap()
                .entries()
                .find(|e| e.gloss == "relative_status")
                .unwrap()
                .id;
            OntologyStore::open(ontology)
                .unwrap()
                .admit(
                    vec![OntologyEntry {
                        language: "en-US".into(),
                        n: 1,
                        inventory_rev: 4,
                        layer: Layer::Discourse,
                        when: vec![Condition {
                            slot: Slot::Scene,
                            feature: status,
                            test: Test::Is(weksa::packet::FeatValue::Option(common::lab("above"))),
                        }],
                        choice: "ENABOVE".into(),
                        examples: vec![],
                        annotator: weksa::Annotator::Operator,
                    }],
                    &inv_store,
                )
                .unwrap();
            extra(&dir.join("c.cc"), &dir.join("e.cc"));
        },
    )
}

/// Answers for items 1 to 3 where item 3 names referent n2 (no entity) as its
/// addressee, or nobody.
fn addressing(addressee: Option<&str>) -> Vec<(usize, String)> {
    (1..=3)
        .map(|n| {
            let mut v = serde_json::to_value(draft_of(n)).unwrap();
            if n == 3 {
                v["discourse"]["addressee"] = serde_json::json!(addressee);
            }
            (n, v.to_string())
        })
        .collect()
}

fn lowering_of(model: &FakeModel, marker: &str) -> Vec<String> {
    lowering_texts(model)
        .into_iter()
        .filter(|t| t.contains(marker))
        .map(String::from)
        .collect()
}

#[test]
fn round_prompts_carry_scene_selected_entries() {
    // Above an unnamed addressee: item 3's prompt holds the entry, the other
    // two (no scene for their groups) read NoScene and print without it.
    let mut dao = scene_dao(|_, _| {});
    let mut model = fake(&addressing(Some("n2")));
    recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    assert_eq!(lowering_texts(&model).len(), 3);
    let with = lowering_of(&model, "\nonto:en-US:1 [discourse] ENABOVE");
    assert_eq!(with.len(), 1, "only item 3 has a scene and an addressee");
    assert_eq!(
        lowering_texts(&model).iter().filter(|t| t.contains("ontology: none")).count(),
        2
    );
    // The same item with no addressee selects no Scene entry.
    let mut dao = scene_dao(|_, _| {});
    let mut model = fake(&addressing(None));
    recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    assert!(lowering_of(&model, "ENABOVE").is_empty());
    assert_eq!(lowering_texts(&model).len(), 3);
}

#[test]
fn a_scene_read_failure_is_a_lower_refusal() {
    // A live item of item 3's group, in a language outside the panel, spoken by
    // someone the scene does not cover: V16 for that group.
    let mut dao = scene_dao(|corpus, entities| {
        let mut entity_store = weksa::EntityStore::open(entities).unwrap();
        entity_store.put(ent("stranger", "en-US", &["Stranger"])).unwrap();
        let mut store = CorpusStore::open(corpus).unwrap();
        let mut twin = store.get("item:daodejing:3").unwrap().unwrap();
        twin.id = "item:daodejing:93".into();
        twin.language = "pt-BR".into();
        twin.text = "PORTUGUESEMARK 93 o sabio".into();
        twin.speaker = common::eid("stranger");
        twin.names = vec![];
        twin.prior_turns = vec![];
        store.add(twin, &entity_store).unwrap();
    });
    let mut model = fake(&addressing(Some("n2")));
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    // Item 3 never reached the model; items 1 and 2 did, and the round recorded.
    assert_eq!(lowering_texts(&model).len(), 2);
    assert_eq!(r.n(), 1);
}

#[test]
fn a_run_pins_its_speaker_state_and_refuses_drift() {
    let dao = scene_dao(|_, _| {});
    let r = dao.run.record();
    let path = dao.ontology.with_file_name("speaker-state.cc");
    assert_eq!(r.speaker_state, std::fs::canonicalize(&path).unwrap().to_str().unwrap());
    assert_eq!(r.speaker_state_digest, hex(&path));
    let root = dao.root.clone();
    assert_eq!(RunStores::open(&root).unwrap().record(), dao.run.record());
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.push(0xc0);
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(
        RunStores::open(&root).err(),
        Some(RunError::Drift(DriftOf::SpeakerState))
    );
}

#[test]
fn create_refuses_a_scene_its_inventory_cannot_read() {
    // The scene is admitted against a copy that gained rev 4; the run's head
    // inventory stops at rev 3.
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/daodejing");
    let dir = tempfile::tempdir().unwrap();
    for (from, to) in [("corpus.cc", "c.cc"), ("entity.cc", "e.cc")] {
        std::fs::copy(src.join(from), dir.path().join(to)).unwrap();
    }
    common::dao_rev3_at(&dir.path().join("i.cc"));
    common::dao_rev3_at(&dir.path().join("rich.cc"));
    let state = common::speaker_state_in(dir.path());
    put_dao_scene(&state, &dir.path().join("rich.cc"), &dir.path().join("c.cc"), &dir.path().join("e.cc"));
    let ontology = common::ontology_in(dir.path());
    let root = dir.path().join("run");
    let create = || {
        RunStores::create(
            &root,
            "x",
            RunPaths {
                corpus: &dir.path().join("c.cc"),
                inventory: &dir.path().join("i.cc"),
                entities: &dir.path().join("e.cc"),
                ontology: &ontology,
                speaker_state: &state,
            },
            "fake-model",
            "bag",
            weksa::run::LowererReach::ToolLessRelay,
            ModelFamily::Unrecorded,
            &["en-US".to_string()],
        )
    };
    assert_eq!(create().err(), Some(RunError::SpeakerState));
    assert!(!root.join("run.cc").exists());
    assert!(!root.join("inventory.cc").exists());
}

#[test]
fn a_run_reads_entries_and_scenes_at_its_own_head() {
    use weksa::{Condition, Layer, OntologyEntry, OntologyStore, Slot, Test};
    // An entry and a scene admitted at rev 4 through a Dao-shaped inventory open
    // a run whose own inventory is seed, proof and the scene revision as rev 3:
    // beds number revisions independently, so a run reads features at its head.
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/daodejing");
    let dir = tempfile::tempdir().unwrap();
    for (from, to) in [("corpus.cc", "c.cc"), ("entity.cc", "e.cc")] {
        std::fs::copy(src.join(from), dir.path().join(to)).unwrap();
    }
    let rich = dir.path().join("rich.cc");
    common::dao_rev3_at(&rich);
    let state = common::speaker_state_in(dir.path());
    put_dao_scene(&state, &rich, &dir.path().join("c.cc"), &dir.path().join("e.cc"));
    let rich_store = InventoryStore::open(&rich).unwrap();
    let status = rich_store.inventory().unwrap().entries().find(|e| e.gloss == "relative_status").unwrap().id;
    let ontology = common::ontology_in(dir.path());
    OntologyStore::open(&ontology)
        .unwrap()
        .admit(
            vec![OntologyEntry {
                language: "en-US".into(),
                n: 1,
                inventory_rev: 4,
                layer: Layer::Discourse,
                when: vec![Condition {
                    slot: Slot::Scene,
                    feature: status,
                    test: Test::Is(weksa::packet::FeatValue::Option(common::lab("above"))),
                }],
                choice: "ENABOVE".into(),
                examples: vec![],
                annotator: weksa::Annotator::Operator,
            }],
            &rich_store,
        )
        .unwrap();
    let head = dir.path().join("i.cc");
    let mut own = InventoryStore::open(&head).unwrap();
    for (rev, entries) in [
        weksa::inventory::seed(),
        weksa::inventory::proof_revision(),
        weksa::inventory::speaker_scene_revision(3),
    ] {
        own.admit_revision(rev, entries).unwrap();
    }
    drop(own);
    let run = RunStores::create(
        &dir.path().join("run"),
        "x",
        RunPaths {
            corpus: &dir.path().join("c.cc"),
            inventory: &head,
            entities: &dir.path().join("e.cc"),
            ontology: &ontology,
            speaker_state: &state,
        },
        "fake-model",
        "bag",
        weksa::run::LowererReach::ToolLessRelay,
        ModelFamily::Unrecorded,
        &["en-US".to_string()],
    );
    assert!(run.is_ok(), "{:?}", run.err());
}

// ---- test-text-guard: no prompt file or answer for a prompt holding Test text ----

#[test]
fn answer_files_withhold_prompts_that_hold_test_text() {
    use weksa::model::{AnswerFiles, ModelPort, PortError};
    let p = prompt();
    let dir = tempfile::tempdir().unwrap();
    let round = dir.path().join("round-1");
    let refused = |t: &str| {
        AnswerFiles::new(round.clone(), "m".into(), vec![t.to_string()]).answer(&p)
            == Err(PortError::Withheld)
    };
    // A text of 12 normalised characters or more counts anywhere in the prompt, in
    // any case, spacing, wrapping or width: the corpus door's own sameness.
    assert!(refused("Nibu says mind the hatch."));
    assert!(refused("NIBU SAYS MIND THE HATCH"));
    assert!(refused("nibu  says\n mind the\n\n hatch"));
    assert!(refused("\u{ff2e}ibu says mind the hatch."));
    assert!(!round.exists(), "a refused prompt creates nothing");
    // The threshold is 12 normalised characters: "the hatch" (8) says nothing about a
    // prompt, "mind the hatch" (12) does.
    assert!(!refused("the hatch"));
    assert!(refused("mind the hatch"));
    assert!(!refused("   "), "a blank withheld text matches nothing");
    // Unrelated text passes and the prompt is then written for its answerer.
    let mut clean = AnswerFiles::new(round.clone(), "m".into(), vec!["The ferry leaves at noon.".into()]);
    assert_eq!(clean.answer(&p), Err(PortError::Unanswered));
    assert!(round.join(format!("{}.prompt", p.digest())).exists());
    // The refusal comes before a recorded answer is read.
    std::fs::write(round.join(format!("{}.answer", p.digest())), "x").unwrap();
    assert!(refused("Nibu says mind the hatch."));
}

#[test]
fn a_round_stops_on_a_prompt_that_holds_test_text() {
    use common::{cast, entities_in, group_in_bucket, item_in};
    use weksa::model::{ModelPort, PortError};
    use weksa::run::{RunError, round};
    let mut w = world();
    let entities = entities_in(w.dir.path());
    // Test sentences (one withdrawn), and Train lines that quote them: the corpus
    // door refuses an identical twin, not a longer line that holds one.
    let held = "Hold this sentence back.";
    let gone = "A withdrawn Test sentence.";
    w.corpus
        .add(item_in("item:held:3", gone, &group_in_bucket(91), cast()), &entities)
        .unwrap();
    w.corpus.withdraw("item:held:3").unwrap();
    for (key, line, bucket) in [
        ("item:held:1", held.to_string(), 90),
        ("item:held:2", format!("{held} And then some."), 10),
        ("item:held:4", format!("{gone} And then some."), 11),
    ] {
        w.corpus
            .add(item_in(key, &line, &group_in_bucket(bucket), cast()), &entities)
            .unwrap();
    }
    let mut run = run_over(w.dir.path());
    let dir = w.dir.path().join("answers");
    let mut files = run.answer_files(dir.clone()).unwrap();
    let inv = run.inventory().inventory().unwrap();
    let train = run.train().unwrap();
    let encode = |key: &str| {
        let offer = run.offer(key).unwrap().unwrap();
        weksa::context::Prompt::encode(&offer, &train, &inv, 4)
    };
    assert_eq!(files.answer(&encode("item:nibu:1")), Err(PortError::Unanswered));
    for key in ["item:held:2", "item:held:4"] {
        assert_eq!(files.answer(&encode(key)), Err(PortError::Withheld), "{key}");
    }
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "only the clean prompt");
    // The round stops on it rather than skipping the item. A model-written line in a
    // Test group is withheld too: it is in no split, and its text is still held out.
    // (Covered by test_texts taking every Test-group item, below.)
    let scorer = weksa::score::Scorer::new(OneBag);
    let mut round_files = run.answer_files(w.dir.path().join("round")).unwrap();
    assert!(matches!(
        round(&mut run, &mut round_files, &scorer),
        Err(RunError::Withheld)
    ));
}

#[test]
fn the_encode_step_and_ontology_mint_stop_on_a_withheld_prompt() {
    use weksa::run::{RunError, round};
    let w = world();
    let mut run = run_over(w.dir.path());
    let scorer = weksa::score::Scorer::new(OneBag);
    let r = round(&mut run, &mut Withholds, &scorer);
    assert!(matches!(r, Err(RunError::Withheld)), "{:?}", r.map(|_| ()));
    assert!(matches!(run.mint_ontology(&mut Withholds, "en-US"), Err(RunError::Withheld)));
}

#[test]
fn model_written_test_group_text_is_withheld_too() {
    use common::{entities_in, group_in_bucket, item_in, model};
    let mut w = world();
    let entities = entities_in(w.dir.path());
    w.corpus
        .add(item_in("item:held:5", "A model line of a Test meaning.", &group_in_bucket(92), model()), &entities)
        .unwrap();
    assert!(
        w.corpus
            .test_texts()
            .unwrap()
            .iter()
            .any(|t| t == "A model line of a Test meaning."),
        "a model-written Test-group item is in no split, and its text is still held out"
    );
}

// ---- test-text-withheld-arms: a refused prompt stops every model call, none skips it ----

/// Refuses every prompt as holding Test text, as the answer door does.
struct Withholds;

impl ModelPort for Withholds {
    fn model(&self) -> &str {
        "fake-model"
    }
    fn answer(&mut self, _: &Prompt) -> Result<String, PortError> {
        Err(PortError::Withheld)
    }
}

#[test]
fn recall_stops_on_a_withheld_prompt() {
    let dao = dao();
    assert!(matches!(dao.run.recall(&mut Withholds), Err(RunError::Withheld)));
}

#[test]
fn the_lowering_step_of_a_round_stops_on_a_withheld_prompt() {
    // Encoding is answered; every lowering prompt is refused.
    let mut dao = dao();
    let mut model = fake(&good_answers());
    model.raw_lowering = Some(Err(PortError::Withheld));
    let r = round(&mut dao.run, &mut model, &Scorer::new(Bag));
    assert!(matches!(r, Err(RunError::Withheld)), "{:?}", r.map(|_| ()));
    assert!(
        !dao.root.join("round-1/packets.cc").exists(),
        "the refused attempt's packets are discarded"
    );
}

#[test]
fn the_cycle_editor_stops_on_a_withheld_prompt() {
    let mut dao = dao();
    two_rounds(&mut dao);
    let mut model = cycle_fake(ONE_CONCEPT);
    model.edit = Some(Err(PortError::Withheld));
    let r = cycle(&mut dao.run, &mut model, &Scorer::new(Bag));
    assert!(matches!(r, Err(RunError::Withheld)), "{:?}", r.map(|_| ()));
}

#[test]
fn a_stance_ablation_stops_on_a_withheld_prompt() {
    let (dao, _) = recorded_dao(|_| {});
    let mut rec = Lowerer::new(blind);
    let r = ablate(&dao, &mut rec, &mut Withholds);
    assert!(matches!(r, Err(RunError::Withheld)), "{:?}", r.map(|_| ()));
    // A recorded answer that is refused stops it as well.
    let r = ablate(&dao, &mut Withholds, &mut Lowerer::new(blind));
    assert!(matches!(r, Err(RunError::Withheld)), "{:?}", r.map(|_| ()));
}

#[test]
fn the_cycles_candidate_round_stops_on_a_withheld_lowering_prompt() {
    // The edit is answered and the candidate round encodes at rev 4; every lowering
    // prompt is then refused. No decision is recorded and no packets are left.
    let mut dao = dao();
    two_rounds(&mut dao);
    let mut model = cycle_fake(ONE_CONCEPT);
    model.raw_lowering = Some(Err(PortError::Withheld));
    let r = cycle(&mut dao.run, &mut model, &Scorer::new(Bag));
    assert!(matches!(r, Err(RunError::Withheld)), "{:?}", r.map(|_| ()));
    assert!(dao.run.decisions().unwrap().is_empty(), "a refused cycle decides nothing");
    assert!(
        !dao.root.join("cycle-1/packets.cc").exists(),
        "the refused attempt's packets are discarded"
    );
}

// ---- distil-slice: relower, a fixed-packet re-lowering measure ----

use weksa::run::{RelowerOutcome, RelowerReading};

/// `relower` of round 1 with the Bag scorer; `floors` are the floor arms (none, or three or more).
fn relower(
    dao: &Dao,
    recorded: &mut dyn ModelPort,
    arm: &mut dyn ModelPort,
    floors: &mut [&mut dyn ModelPort],
) -> Result<RelowerOutcome, RunError> {
    dao.run.relower(1, "arm", recorded, arm, floors, &Scorer::new(Bag))
}

fn relowered(outcome: Result<RelowerOutcome, RunError>) -> (Vec<RelowerReading>, Option<weksa::run::Verdict>) {
    match outcome.unwrap() {
        RelowerOutcome::Read { languages, verdict } => (languages, verdict),
        RelowerOutcome::Unanswered { prompts } => panic!("unanswered {prompts:?}"),
    }
}

#[test]
fn relower_asks_only_validation_lowering_prompts_of_the_round() {
    let (dao, r) = recorded_dao(|_| {});
    let round_digests: Vec<String> = r.calls()[4..].iter().map(|c| c.prompt.clone()).collect();
    // The reference pass: a lowerer shown every packet of the round, to learn
    // which digest is which item's.
    let (mut every, mut rec, mut arm) = (Lowerer::new(reader), Lowerer::new(reader), Lowerer::new(reader));
    read(ablate(&dao, &mut every, &mut Lowerer::new(reader)).unwrap());
    relowered(relower(&dao, &mut rec, &mut arm, &mut []));
    let of = |keys: &[&str]| -> Vec<String> {
        every
            .seen
            .iter()
            .filter(|s| keys.contains(&s.1.as_str()))
            .map(|s| s.0.clone())
            .fold(Vec::new(), |mut v, d| {
                if !v.contains(&d) {
                    v.push(d);
                }
                v
            })
    };
    let mut want = of(&["item:daodejing:2", "item:daodejing:3", WRITTEN]);
    want.sort();
    let mut got = arm.digests();
    got.sort();
    assert_eq!(got, want, "exactly the Validation lowering prompts");
    assert_eq!(want.len(), 3);
    for d in &want {
        assert!(round_digests.contains(d), "byte-identical to the round's prompt {d}");
    }
    // Neither the lone Train item, a Test item nor an encode prompt.
    for (_, key, text) in &arm.seen {
        assert!(!["item:daodejing:1", "item:daodejing:4", "item:daodejing:5"].contains(&key.as_str()));
        assert!(text.starts_with(LOWER_INSTRUCTIONS));
    }
    assert_eq!(rec.digests().len(), 3, "the recorded port is asked the same prompts");
}

#[test]
fn relower_reads_recorded_and_arm_against_the_item() {
    let (dao, _) = recorded_dao(|_| {});
    // An arm that says the item's own sentence beats a recorded lowering that
    // adds the stance to it.
    let (langs, verdict) = relowered(relower(
        &dao,
        &mut Lowerer::new(reader),
        &mut Lowerer::new(blind),
        &mut [],
    ));
    let l = &langs[0];
    assert_eq!((l.language.as_str(), l.n, l.unusable), ("en-US", 3, 0));
    assert!(l.recorded.chrf < l.arm.chrf - 5.0, "{}", l.line());
    assert!(l.arm.composite > l.recorded.composite);
    assert!(l.floor.is_none() && verdict.is_none(), "no floor arm, no verdict");
    // An arm equal to the recorded answers reads equal.
    let (langs, _) = relowered(relower(&dao, &mut Lowerer::new(reader), &mut Lowerer::new(reader), &mut []));
    assert_eq!(langs[0].recorded, langs[0].arm);
}

/// Answers `text` for every item but `bad`, which gets something that is no lowering.
struct Breaks(&'static str);

impl ModelPort for Breaks {
    fn model(&self) -> &str {
        "fake-model"
    }
    fn answer(&mut self, prompt: &Prompt) -> Result<String, PortError> {
        let key = prompt.item_keys()[0].clone();
        if key == self.0 {
            return Ok("this is not a lowering draft".into());
        }
        Ok(serde_json::json!({ "text": english(&key) }).to_string())
    }
}

#[test]
fn relower_counts_an_unusable_arm_answer_as_zero() {
    let (dao, _) = recorded_dao(|_| {});
    let keys = ["item:daodejing:2", "item:daodejing:3", WRITTEN];
    let (whole, _) = relowered(relower(&dao, &mut Lowerer::new(blind), &mut Lowerer::new(blind), &mut []));
    let whole = whole[0].arm;
    let (mut chrf, mut meaning) = (0.0, 0.0);
    for key in keys {
        let (langs, _) = relowered(relower(&dao, &mut Lowerer::new(blind), &mut Breaks(key), &mut []));
        let l = &langs[0];
        assert_eq!((l.n, l.unusable), (3, 1), "{}", l.line());
        assert!(l.arm.chrf < whole.chrf && l.arm.meaning < whole.meaning, "{}", l.line());
        chrf += l.arm.chrf;
        meaning += l.arm.meaning;
    }
    // Each item is refused once: the three means hold every item twice at its
    // score and once at 0, so the refused item entered its mean as 0 and n stayed 3.
    assert!((chrf - 2.0 * whole.chrf).abs() < 1e-6, "{chrf} {}", whole.chrf);
    assert!((meaning - 2.0 * whole.meaning).abs() < 1e-6, "{meaning} {}", whole.meaning);
}

#[test]
fn relower_scores_an_unusable_recorded_answer_as_zero_without_counting_it_against_the_arm() {
    let (dao, _) = recorded_dao(|_| {});
    let (whole, _) = relowered(relower(&dao, &mut Lowerer::new(blind), &mut Lowerer::new(blind), &mut []));
    let (langs, _) = relowered(relower(&dao, &mut Breaks("item:daodejing:3"), &mut Lowerer::new(blind), &mut []));
    let l = &langs[0];
    assert_eq!(l.unusable, 0, "{}", l.line());
    assert_eq!(l.arm, whole[0].arm);
    assert!(l.recorded.chrf < whole[0].recorded.chrf && l.recorded.meaning < whole[0].recorded.meaning);
}

#[test]
fn relower_decides_by_decide_panel_against_the_floor_arm() {
    use weksa::run::decide_panel;
    let (dao, _) = recorded_dao(|_| {});
    let run = |recorded: fn(&str, &str) -> String, arm: fn(&str, &str) -> String| {
        let (mut f1, mut f2, mut f3) = (Lowerer::new(recorded), Lowerer::new(recorded), Lowerer::new(recorded));
        let (langs, verdict) = relowered(relower(
            &dao,
            &mut Lowerer::new(recorded),
            &mut Lowerer::new(arm),
            &mut [&mut f1, &mut f2, &mut f3],
        ));
        let l = &langs[0];
        let floor_reading = l.floor.expect("a floor arm gives a floor");
        // Floor arms that equal the recorded answers have no spread.
        assert_eq!((floor_reading.chrf, floor_reading.meaning, floor_reading.composite), (0.0, 0.0, 0.0));
        assert_eq!(verdict, Some(decide_panel(&[(l.recorded, l.arm, floor_reading)])));
        verdict.unwrap()
    };
    assert_eq!(run(reader, reader), Verdict::Rejected(Why::NoGain));
    assert_eq!(run(reader, blind), Verdict::Accepted);
    assert!(matches!(run(blind, reader), Verdict::Rejected(Why::Regressed(_))));
}

#[test]
fn relower_floor_is_the_paired_noise_of_recorded_against_the_floor_arms() {
    let (dao, _) = recorded_dao(|_| {});
    let (langs, _) = relowered(relower(
        &dao,
        &mut Lowerer::new(blind),
        &mut Lowerer::new(blind),
        &mut [&mut Lowerer::new(blind), &mut Lowerer::new(blind), &mut Breaks("item:daodejing:3")],
    ));
    let l = &langs[0];
    let f = l.floor.unwrap();
    assert!(f.chrf > 0.0 && f.meaning > 0.0 && f.composite > 0.0, "{}", l.line());
    assert!(l.line().contains(&format!("| floor chrf={:.2}", f.chrf)), "{}", l.line());
    assert_eq!(l.unusable, 0, "unusable counts the arm, not the floor");
    assert_eq!(l.floor_unusable, 1, "a floor arm's refusal is counted apart");
    assert!(l.line().contains(" unusable=0 recorded "), "{}", l.line());
    assert!(l.line().ends_with(" unusable=1"), "the floor's refusal is on the line: {}", l.line());
    // A floor needs three arms: fewer measure no noise, and nothing is asked of the arm.
    for arms in [1usize, 2] {
        let mut fs: Vec<Lowerer> = (0..arms).map(|_| Lowerer::new(blind)).collect();
        let mut dyns: Vec<&mut dyn ModelPort> = fs.iter_mut().map(|f| f as &mut dyn ModelPort).collect();
        let mut arm = Lowerer::new(blind);
        let r = relower(&dao, &mut Lowerer::new(blind), &mut arm, &mut dyns);
        assert!(matches!(r, Err(RunError::FloorArms)));
        assert!(arm.seen.is_empty());
    }
    // Without floor arms there is none to count.
    let (none, _) = relowered(relower(&dao, &mut Lowerer::new(blind), &mut Lowerer::new(blind), &mut []));
    assert_eq!(none[0].floor_unusable, 0);
}

#[test]
fn relower_skips_and_counts_an_item_the_round_refused_before_admission() {
    // Validation item 3 is not an encodable draft in the round: it has no packet.
    let (dao, _) = recorded_dao(|answers| {
        answers.insert("item:daodejing:3".into(), "not a draft".into());
    });
    let (mut rec, mut arm) = (Lowerer::new(blind), Lowerer::new(blind));
    let (langs, _) = relowered(relower(&dao, &mut rec, &mut arm, &mut []));
    let l = &langs[0];
    assert_eq!((l.n, l.skipped, l.unusable), (2, 1, 0), "{}", l.line());
    assert!(l.line().starts_with("relower en-US n=2 skipped=1 unusable=0 "), "{}", l.line());
    assert_eq!(l.recorded, l.arm, "both readings are over the same two items");
    assert!(arm.seen.iter().all(|(_, key, _)| key != "item:daodejing:3"), "nothing is asked for it");
}

#[test]
fn relower_skips_and_counts_an_item_whose_lowering_prompt_the_round_refused() {
    // ja-JP item 12 cites an entity with no ja form: the round admitted it and refused it
    // at Lower. Item 13 cites none and lowers.
    let mut dao = bilingual(None);
    let mut model = bilingual_fake();
    model.answers.insert("item:daodejing:12".into(), citing_laozi(2));
    model.answers.insert("item:daodejing:13".into(), json(&draft_of(3)));
    let r = recorded(round(&mut dao.run, &mut model, &Scorer::new(Bag)).unwrap());
    assert_eq!(r.language("ja-JP").unwrap().validation_refused.lower, 1);
    let (mut rec, mut arm) = (Lowerer::new(bi_blind), Lowerer::new(bi_blind));
    let (langs, _) = relowered(relower(&dao, &mut rec, &mut arm, &mut []));
    let ja = langs.iter().find(|l| l.language == "ja-JP").unwrap();
    assert_eq!((ja.n, ja.skipped), (1, 1), "{}", ja.line());
    assert_eq!(ja.recorded, ja.arm);
    let en = langs.iter().find(|l| l.language == "en-US").unwrap();
    assert_eq!((en.n, en.skipped), (2, 0), "{}", en.line());
    assert!(arm.seen.iter().all(|(_, key, _)| key != "item:daodejing:12"));
}

/// The ja-JP item's own text, else the English one: `f` for en-US.
fn bi(key: &str, text: &str, f: fn(&str, &str) -> String) -> String {
    match JA_TEXTS.iter().find(|(k, ..)| *k == key) {
        Some((_, ja, _)) => ja.to_string(),
        None => f(key, text),
    }
}

fn bi_blind(key: &str, text: &str) -> String {
    bi(key, text, blind)
}

fn bi_reader(key: &str, text: &str) -> String {
    bi(key, text, reader)
}

/// Fine in English, nonsense in ja-JP.
fn bi_ja_lost(key: &str, text: &str) -> String {
    match JA_TEXTS.iter().any(|(k, ..)| *k == key) {
        true => "zzzz".into(),
        false => blind(key, text),
    }
}

#[test]
fn relower_decides_over_the_whole_panel_not_its_first_language() {
    use weksa::run::decide_panel;
    let mut dao = bilingual(LAOZI_JA);
    recorded(round(&mut dao.run, &mut bilingual_fake(), &Scorer::new(Bag)).unwrap());
    // en-US gains (the arm drops the stance the recorded lowering repeats); ja-JP is lost.
    let (langs, verdict) = relowered(relower(
        &dao,
        &mut Lowerer::new(bi_reader),
        &mut Lowerer::new(bi_ja_lost),
        &mut [&mut Lowerer::new(bi_reader), &mut Lowerer::new(bi_reader), &mut Lowerer::new(bi_reader)],
    ));
    assert_eq!(
        langs.iter().map(|l| (l.language.as_str(), l.n)).collect::<Vec<_>>(),
        [("en-US", 2), ("ja-JP", 2)],
        "panel order, Validation only"
    );
    assert!(langs[0].arm.composite > langs[0].recorded.composite, "en-US gains");
    assert!(langs[1].arm.composite < langs[1].recorded.composite, "ja-JP is lost");
    let panel: Vec<_> = langs.iter().map(|l| (l.recorded, l.arm, l.floor.unwrap())).collect();
    assert_eq!(verdict, Some(decide_panel(&panel)));
    assert!(matches!(verdict, Some(Verdict::Rejected(Why::Regressed(_)))), "{verdict:?}");
    // The first language alone would have accepted.
    assert_eq!(decide_panel(&panel[..1]), Verdict::Accepted);
}

#[test]
fn relower_refuses_a_cycle_round_and_a_missing_recorded_answer() {
    use weksa::model::RecordedAnswers;
    let mut dao = dao();
    two_rounds(&mut dao);
    run_cycle(&mut dao, &mut cycle_fake(ONE_CONCEPT));
    let candidate = dao.run.rounds().unwrap().into_iter().find(|r| r.cycle().is_some()).unwrap().n();
    let mut arm = Lowerer::new(reader);
    let r = dao.run.relower(candidate, "arm", &mut Lowerer::new(reader), &mut arm, &mut [], &Scorer::new(Bag));
    assert!(matches!(r, Err(RunError::Malformed("round"))));
    let r = dao.run.relower(99, "arm", &mut Lowerer::new(reader), &mut arm, &mut [], &Scorer::new(Bag));
    assert!(matches!(r, Err(RunError::Malformed("round"))));
    assert!(arm.seen.is_empty());

    let (dao, _) = recorded_dao(|_| {});
    let mut empty = RecordedAnswers::new(dao.root.join("round-1/nowhere"), "fake-model".into());
    let mut arm = Lowerer::new(reader);
    let r = relower(&dao, &mut empty, &mut arm, &mut []);
    assert!(matches!(r, Err(RunError::Unrecorded)));
    assert!(arm.seen.is_empty(), "nothing is asked of the arm before the round's answer exists");
}

#[test]
fn relower_floor_arm_with_an_unanswered_prompt_is_refused() {
    use weksa::model::RecordedAnswers;
    let (dao, _) = recorded_dao(|_| {});
    let mut floor = RecordedAnswers::new(dao.root.join("relower-1-floor"), "fake-model".into());
    let r = relower(
        &dao,
        &mut Lowerer::new(reader),
        &mut Lowerer::new(reader),
        &mut [&mut floor, &mut Lowerer::new(reader), &mut Lowerer::new(reader)],
    );
    assert!(matches!(r, Err(RunError::FloorUnanswered)));
    assert!(!dao.root.join("relower-1-floor").exists(), "a floor arm is never written to");
}

#[test]
fn relower_checks_the_embedder_not_the_model() {
    struct Named(Lowerer);
    impl ModelPort for Named {
        fn model(&self) -> &str {
            "another-model"
        }
        fn answer(&mut self, p: &Prompt) -> Result<String, PortError> {
            self.0.answer(p)
        }
    }
    let (dao, _) = recorded_dao(|_| {});
    let mut other = Named(Lowerer::new(blind));
    relowered(relower(&dao, &mut Lowerer::new(reader), &mut other, &mut []));
    let mut arm = Lowerer::new(reader);
    let r = dao.run.relower(1, "arm", &mut Lowerer::new(reader), &mut arm, &mut [], &Scorer::new(OtherEmbedder));
    assert!(matches!(r, Err(RunError::Drift(DriftOf::Embedder))));
    assert!(arm.seen.is_empty());
}

/// Every file under `dir` with its bytes, by relative path.
fn tree(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.insert(
                    p.strip_prefix(dir).unwrap().to_string_lossy().into_owned(),
                    std::fs::read(&p).unwrap(),
                );
            }
        }
    }
    out
}

#[test]
fn relower_records_nothing() {
    use weksa::model::RecordedAnswers;
    let (dao, _) = recorded_dao(|_| {});
    let before = tree(&dao.root);
    let mut recorded = RecordedAnswers::new(dao.root.join("round-1"), "fake-model".into());
    // An unanswered arm writes its prompts in its own directory only.
    let mut files = dao.run.answer_files(dao.run.relower_dir(1, "arm")).unwrap();
    // The recorded answers are files here, as the relay leaves them.
    let r = relower(&dao, &mut recorded, &mut files, &mut []);
    assert!(matches!(r, Err(RunError::Unrecorded)), "no round answer files yet");
    let after = tree(&dao.root);
    for (path, bytes) in &before {
        assert_eq!(after.get(path), Some(bytes), "{path} changed");
    }
    assert!(
        after.keys().all(|p| before.contains_key(p) || p.starts_with("relower-1-arm/")),
        "only relower-1-arm gains files"
    );

    // With the round's answers present, the arm is asked and its prompts land in relower-1-arm.
    let (dao, r) = recorded_dao(|_| {});
    for call in &r.calls()[4..] {
        std::fs::write(
            dao.root.join(format!("round-1/{}.answer", call.prompt)),
            r#"{"text":"The highest excellence is like the water."}"#,
        )
        .unwrap();
    }
    let before = tree(&dao.root);
    let mut recorded = RecordedAnswers::new(dao.root.join("round-1"), "fake-model".into());
    let mut files = dao.run.answer_files(dao.run.relower_dir(1, "arm")).unwrap();
    let RelowerOutcome::Unanswered { prompts } = relower(&dao, &mut recorded, &mut files, &mut []).unwrap() else {
        panic!("the arm is unanswered")
    };
    assert_eq!(prompts.len(), 3);
    let after = tree(&dao.root);
    for (path, bytes) in &before {
        assert_eq!(after.get(path), Some(bytes), "{path} changed");
    }
    let added: Vec<&String> = after.keys().filter(|p| !before.contains_key(*p)).collect();
    assert!(added.iter().all(|p| p.starts_with("relower-1-arm/")), "{added:?}");
    assert_eq!(added.iter().filter(|p| p.ends_with(".prompt")).count(), 3);
}

#[test]
fn relower_arm_prompt_holding_test_text_is_withheld() {
    // A Test sentence that a Validation lowering prompt holds: a line of the
    // lowering instructions, which every lowering prompt carries.
    let line = LOWER_INSTRUCTIONS.lines().find(|l| l.trim().chars().count() > 30).unwrap().trim().to_string();
    let (dao, _) = recorded_dao_in(
        |corpus, entities| {
            let mut held = corpus.get("item:daodejing:4").unwrap().unwrap();
            held.id = "item:held:9".into();
            held.text = line.clone();
            corpus.add(held, entities).unwrap();
        },
        |_| {},
    );
    let mut files = dao.run.answer_files(dao.run.relower_dir(1, "arm")).unwrap();
    let r = relower(&dao, &mut Lowerer::new(reader), &mut files, &mut []);
    assert!(matches!(r, Err(RunError::Withheld)), "{:?}", r.map(|_| ()));
    let dir = dao.run.relower_dir(1, "arm");
    let prompts = std::fs::read_dir(&dir)
        .map(|d| d.filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().ends_with(".prompt")).count())
        .unwrap_or(0);
    assert_eq!(prompts, 0, "no prompt file is written");
}

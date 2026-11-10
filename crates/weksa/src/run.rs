//! One golf-loop run: the stores it pinned at open, the rounds and decisions it
//! has recorded, and the one writer of all of them (`RunStores::create`, `round`
//! and `cycle`). A cycle asks an editor for one inventory edit, scores a
//! candidate round under it and accepts or rejects on Validation (`decide`); the
//! latest Accepted decision names the run's head inventory.
//!
//! A run pins its corpus and entity store files by canonical path and FNV-1a 64
//! digest of their bytes. It works on its own copy of the inventory:
//! `RunStores::create` copies the caller's inventory file to
//! `<root>/inventory.cc` and pins the digest of the copy; the caller's path is
//! kept as provenance and never opened again, and `open` refuses a head file
//! whose digest differs. Every round reopens them only from that record and
//! refuses on drift, and every round record carries the digests. Nothing inside
//! a process whose writer controls the code can stop an item faked into a
//! scratch store; what this makes true is that a run over a scratch store is a
//! different run with a different corpus digest, visible in its record, and a
//! later reader compares the rounds of one run only. `RunStores` is the one
//! public producer of [`TrainItems`] and [`EncoderOffer`]: the CLI paths that
//! take caller stores (`validate`, `packet proof|dao`, `lower`) are unchanged.
//!
//! A round encodes, admits, then lowers each admitted view through the
//! `ModelPort` (`Prompt::lower`), in two answer phases; the CLI's `lower` still
//! uses the template table. The prompt source pin covers `context.rs`,
//! `split.rs` and `lower.rs`.
//!
//! `RunStores::stance_ablation`, like `recall`, is a diagnostic in its own
//! directory: it records nothing and never writes its round's directory.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use cultcache_rs::{
    CacheBackingStore, CultCache, DatabaseEntry, SingleFileMessagePackBackingStore,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::context::{EditFeedback, EncoderOffer, Prompt, TrainItems};
use crate::corpus::{CorpusItem, CorpusItemId, ModelFamily, Source};
use crate::encode::{EditDraft, LoweringDraft, PacketDraft, encoder_id};
use crate::form::{is_attestation, is_slug};
use crate::model::{ModelPort, PortError};
use crate::packet::{Packet, Stance};
use crate::score::{CHRF_VERSION, chrf, Embedder, ItemScore, ScoreInput, Scorer, SplitMean, aggregate};
use crate::split::{Split, SplitScheme, fnv1a64, fnv1a64_of, hex16};
use crate::store::{
    CorpusStore, EntityStore, InventoryStore, PacketStore, StoreError, open_cache_over,
};
use crate::validate::LoweringInput;

/// The digest of the prompt builder's source: `context.rs`, then `split.rs`,
/// whose scheme and digest function decide which lines the prompt holds as
/// Train and name every prompt, then `lower.rs`, whose `TARGET_LANGUAGE` and
/// `EntityForms` port the lowering prompt reads, then `packet.rs`, whose serde
/// shapes the lowering prompt prints as the packet. Each file enters the digest
/// behind its length, so the parts cannot be re-cut into other parts with the
/// same digest. Any edit to any of them, comments included, ends a run; code
/// outside those files is not covered.
pub const PROMPT_SOURCE_DIGEST: u64 = fnv1a64_of(&[
    include_bytes!("context.rs"),
    include_bytes!("split.rs"),
    include_bytes!("lower.rs"),
    include_bytes!("packet.rs"),
]);

/// The least composite gain an edit must show over item noise to be kept, and
/// the grid the editor's one held-out number is rounded to. A run releases at
/// most `1 / LADDER_STEP` = 200 distinct head composites (the Ladder, Blum and
/// Hardt 2015), because the head changes only on an accept and an accept needs
/// a gain above one step.
pub const LADDER_STEP: f64 = 0.005;

/// What a run pinned at open. Key `run:<label>`.
#[derive(Clone, Debug, PartialEq, Eq, JsonSchema, DatabaseEntry)]
#[cultcache(type = "weksa.loop_run")]
pub struct RunRecord {
    #[cultcache(key = 0)]
    pub label: String,
    /// Canonical absolute path of the corpus store.
    #[cultcache(key = 1)]
    pub corpus: String,
    /// Canonical absolute path of the inventory store the run was opened over:
    /// provenance only. `create` copies that file to `<root>/inventory.cc` and
    /// the run reads nothing at this path afterwards.
    #[cultcache(key = 2)]
    pub inventory: String,
    #[cultcache(key = 3)]
    pub entities: String,
    /// 16 hex of the corpus file's bytes.
    #[cultcache(key = 4)]
    pub corpus_digest: String,
    #[cultcache(key = 5)]
    pub entity_digest: String,
    #[cultcache(key = 6)]
    pub prompt_digest: String,
    // Key 7 is retired and never reused: a run.cc from before is a scratch run
    // off git and does not open.
    /// The answering model.
    #[cultcache(key = 8)]
    pub model: String,
    /// The embedder's model.
    #[cultcache(key = 9)]
    pub embedder: String,
    #[cultcache(key = 10)]
    pub chrf_version: String,
    /// 16 hex of the bytes `create` copied to `<root>/inventory.cc`.
    #[cultcache(key = 11)]
    pub inventory_digest: String,
    /// How the run's lowering prompts are answered, set once at open.
    #[cultcache(key = 12)]
    pub lowerer_reach: LowererReach,
    /// The family of the model answering the run's encode and lowering
    /// prompts, set once at open; `Unrecorded` when the opener did not say
    /// and for a run opened before this was kept.
    #[cultcache(key = 13, default)]
    #[schemars(default)]
    pub answerer_family: ModelFamily,
}

/// How a run's lowering prompts reach the model that answers them (rulings
/// `lowerer-call-reach-toolless-relay`, `dao-round-seed-inventory`). The
/// record says which, so a round lowered by a briefed subagent cannot pass as
/// a tool-less one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum LowererReach {
    /// A subagent briefed to read one prompt file: acceptable for smoke rounds
    /// only.
    BriefedSubagent,
    /// The prompt passed inline to an agent with no file tools.
    ToolLessRelay,
}

impl LowererReach {
    /// The spelling `weksa loop open --lowerer-reach` takes and lines print.
    pub fn label(self) -> &'static str {
        match self {
            LowererReach::BriefedSubagent => "briefed-subagent",
            LowererReach::ToolLessRelay => "tool-less-relay",
        }
    }
}

impl std::str::FromStr for LowererReach {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        [LowererReach::BriefedSubagent, LowererReach::ToolLessRelay]
            .into_iter()
            .find(|r| r.label() == s)
            .ok_or_else(|| "expected briefed-subagent or tool-less-relay".to_string())
    }
}

/// Where a round refused: the first stage that did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Stage {
    Parse,
    Admit,
    /// The lowering prompt could not be built, or its answer was not UTF-8, not
    /// a `LoweringDraft`, or text that is blank, over `MAX_LOWERING_BYTES` or
    /// holds a control character.
    Lower,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Outcome {
    Scored { chrf: f64, meaning: Option<f64> },
    Refused(Stage),
}

/// One model call: the prompt's digest and the item keys it held.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Call {
    pub prompt: String,
    pub items: Vec<String>,
}

/// How many items one admission rule refused, over both splits: a rule code
/// (`V5`, `UnknownEntity`) and a count, never an identifier or text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuleCount {
    pub rule: String,
    pub count: u32,
}

/// A Train item's row. Validation has no rows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TrainRow {
    pub item: String,
    pub packet: Option<String>,
    pub outcome: Outcome,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RefusalCounts {
    pub parse: u32,
    pub admit: u32,
    pub lower: u32,
}

/// One scored round. It holds no text and no Test reading: per-item rows for
/// Train only, Validation only as means and refusal counts by stage. Only
/// `run::round` builds one.
///
/// ```
/// fn f(r: &weksa::run::RoundRecord) -> u32 { r.n() }
/// ```
///
/// Every field is given, so the literal below fails only on privacy; its twin
/// is the same literal in the unit test `a_round_record_literal_compiles_in_the_lib`.
///
/// ```compile_fail,E0451
/// use weksa::run::{RefusalCounts, RoundRecord};
/// use weksa::score::SplitMean;
/// fn f() -> RoundRecord {
///     RoundRecord {
///         n: 1,
///         inventory_rev: 1,
///         prompt_digest: String::new(),
///         corpus_digest: String::new(),
///         entity_digest: String::new(),
///         model: String::new(),
///         embedder: String::new(),
///         chrf_version: String::new(),
///         calls: vec![],
///         train: vec![],
///         train_mean: SplitMean::default(),
///         train_scored: 0,
///         validation_mean: SplitMean::default(),
///         validation_scored: 0,
///         validation_refused: RefusalCounts::default(),
///         inventory_digest: String::new(),
///         cycle: None,
///         admit_rules: vec![],
///         parse_reasons: vec![],
///         own_family: 0,
///     }
/// }
/// ```
#[derive(Clone, Debug, PartialEq, JsonSchema, DatabaseEntry)]
#[cultcache(type = "weksa.loop_round")]
pub struct RoundRecord {
    #[cultcache(key = 0)]
    n: u32,
    #[cultcache(key = 1)]
    inventory_rev: u32,
    #[cultcache(key = 2)]
    prompt_digest: String,
    // Key 3 is retired and never reused.
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
    /// The encode calls, then the lowering calls, each with its prompt digest
    /// and one item key.
    #[cultcache(key = 9)]
    calls: Vec<Call>,
    #[cultcache(key = 10)]
    train: Vec<TrainRow>,
    #[cultcache(key = 11)]
    train_mean: SplitMean,
    #[cultcache(key = 12)]
    validation_mean: SplitMean,
    #[cultcache(key = 13)]
    validation_refused: RefusalCounts,
    /// Train items that lowered and scored; `train_mean` is over all of them
    /// and counts a refused item as 0.
    #[cultcache(key = 14)]
    train_scored: u32,
    /// Validation items that lowered and scored; `validation_mean` is over all
    /// of them and counts a refused item as 0, so refusing a weak item cannot
    /// raise it.
    #[cultcache(key = 15)]
    validation_scored: u32,
    /// 16 hex of the inventory file the round was scored under.
    #[cultcache(key = 16)]
    inventory_digest: String,
    /// `Some(k)` for the candidate round of cycle `k`.
    #[cultcache(key = 17)]
    cycle: Option<u32>,
    /// The rules admission refused items under, in rule order; every Train
    /// and Validation item refused at `Stage::Admit` is counted once per rule.
    #[cultcache(key = 18, default)]
    #[schemars(default)]
    admit_rules: Vec<RuleCount>,
    /// Why items were refused at `Stage::Parse`, in code order, over Train and
    /// Validation: a [`DraftClass`] code (`rule`), or `unreadable` for an
    /// item with no answer that read as text. Never a token of an answer.
    #[cultcache(key = 19, default)]
    #[schemars(default)]
    parse_reasons: Vec<RuleCount>,
    /// Offered (live Train and Validation) `ModelWritten` items whose writer
    /// family is the run's answerer family. The corpus rule binds
    /// model judges; the scorer is deterministic, so nothing is refused for
    /// this: the round labels it.
    #[cultcache(key = 20, default)]
    #[schemars(default)]
    own_family: u32,
}

impl RoundRecord {
    pub fn n(&self) -> u32 {
        self.n
    }
    pub fn inventory_rev(&self) -> u32 {
        self.inventory_rev
    }
    pub fn prompt_digest(&self) -> &str {
        &self.prompt_digest
    }
    pub fn corpus_digest(&self) -> &str {
        &self.corpus_digest
    }
    pub fn entity_digest(&self) -> &str {
        &self.entity_digest
    }
    pub fn model(&self) -> &str {
        &self.model
    }
    pub fn embedder(&self) -> &str {
        &self.embedder
    }
    pub fn chrf_version(&self) -> &str {
        &self.chrf_version
    }
    pub fn calls(&self) -> &[Call] {
        &self.calls
    }
    pub fn train(&self) -> &[TrainRow] {
        &self.train
    }
    pub fn train_mean(&self) -> &SplitMean {
        &self.train_mean
    }
    pub fn validation_mean(&self) -> &SplitMean {
        &self.validation_mean
    }
    pub fn validation_refused(&self) -> RefusalCounts {
        self.validation_refused
    }
    pub fn train_scored(&self) -> u32 {
        self.train_scored
    }
    pub fn validation_scored(&self) -> u32 {
        self.validation_scored
    }
    pub fn inventory_digest(&self) -> &str {
        &self.inventory_digest
    }
    pub fn cycle(&self) -> Option<u32> {
        self.cycle
    }
    pub fn admit_rules(&self) -> &[RuleCount] {
        &self.admit_rules
    }
    pub fn parse_reasons(&self) -> &[RuleCount] {
        &self.parse_reasons
    }
    pub fn own_family(&self) -> u32 {
        self.own_family
    }

    /// The round line, unlabelled: [`RunStores::round_line`] adds how the run was lowered. `n` is every item of the split,
    /// `scored` those that lowered; a mean counts a refused item as 0.
    fn line(&self) -> String {
        let mean = |value: Option<f64>, places: usize| {
            value.map_or("none".to_string(), |v| format!("{v:.places$}"))
        };
        let refused = self
            .train
            .iter()
            .filter(|row| matches!(row.outcome, Outcome::Refused(_)))
            .count();
        let (t, v, x) = (
            &self.train_mean,
            &self.validation_mean,
            self.validation_refused,
        );
        let line = format!(
            "round {} rev {} train n={} scored={} chrf={} meaning={} refused={refused} validation n={} scored={} chrf={} meaning={} refused parse={} admit={} lower={}",
            self.n,
            self.inventory_rev,
            t.count,
            self.train_scored,
            mean(t.chrf, 2),
            mean(t.meaning, 4),
            v.count,
            self.validation_scored,
            mean(v.chrf, 2),
            mean(v.meaning, 4),
            x.parse,
            x.admit,
            x.lower,
        );
        let codes = |counts: &[RuleCount]| {
            counts
                .iter()
                .map(|r| format!("{}x{}", r.rule, r.count))
                .collect::<Vec<_>>()
                .join(",")
        };
        let mut line = line;
        if !self.parse_reasons.is_empty() {
            line = format!("{line} parse-reasons={}", codes(&self.parse_reasons));
        }
        if !self.admit_rules.is_empty() {
            line = format!("{line} admit-rules={}", codes(&self.admit_rules));
        }
        if self.own_family > 0 {
            line = format!("{line} own-family {}", self.own_family);
        }
        line
    }
}

/// What a pinned store or id drifted from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriftOf {
    Corpus,
    Entities,
    Prompts,
    Model,
    Embedder,
    /// The run's own head inventory file.
    Inventory,
}

/// A run's failure. Display and Debug name a kind, never a path, key, label or
/// any other value the caller supplied.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunError {
    /// `run.cc` already holds a run.
    Exists,
    /// `run.cc` holds no run.
    NoRun,
    /// A field that cannot be a run's: `label`, `model`, `embedder`, `path`.
    Malformed(&'static str),
    Drift(DriftOf),
    /// The inventory store holds no revision.
    NoInventory,
    Io(ErrorKind),
    /// A store or the run file would not read or write.
    Store,
    /// The embedder failed.
    Embed,
    /// Rounds 1 and 2 are not two plain replicates under the run's first inventory.
    NoFloor,
    /// A round has no Validation reading.
    NoValidation,
    /// The round holds no answer to a lowering prompt the ablation rebuilt: the
    /// prompt's digest drifted or the head inventory moved since the round.
    Unrecorded,
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Exists => f.write_str("the root already holds a run"),
            RunError::NoRun => f.write_str("the root holds no run"),
            RunError::Malformed(what) => write!(f, "malformed {what}"),
            RunError::Drift(DriftOf::Prompts) => f.write_str(
                "drift in Prompts (the prompt source digest covers context.rs, split.rs, lower.rs and packet.rs)",
            ),
            RunError::Drift(of) => write!(f, "drift in {of:?}"),
            RunError::NoInventory => f.write_str("the inventory holds no revision"),
            RunError::Io(kind) => write!(f, "io error {kind:?}"),
            RunError::Store => f.write_str("a store would not read or write"),
            RunError::Unrecorded => f.write_str(
                "the round holds no answer to a lowering prompt rebuilt now: the prompt digest drifted or the head inventory moved since the round",
            ),
            RunError::Embed => f.write_str("the embedder failed"),
            RunError::NoFloor => f.write_str("the run has no noise floor: record two rounds first"),
            RunError::NoValidation => f.write_str("a reading has no validation item"),
        }
    }
}

impl fmt::Debug for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for RunError {}

/// The three stores a run pins.
#[derive(Clone, Copy)]
pub struct RunPaths<'a> {
    pub corpus: &'a Path,
    pub inventory: &'a Path,
    pub entities: &'a Path,
}

/// A run's open stores, reopened only from its record. The one public producer
/// of [`TrainItems`] and [`EncoderOffer`], and the only writer of `run.cc`.
pub struct RunStores {
    root: PathBuf,
    cache: CultCache,
    record: RunRecord,
    corpus: CorpusStore,
    inventory: InventoryStore,
    entities: EntityStore,
}

/// The run's own copy of the inventory, made at `create`.
const INVENTORY_FILE: &str = "inventory.cc";

/// The corpus, entity and head inventory files against the record's pins.
fn check_pins(cache: &CultCache, root: &Path, record: &RunRecord) -> Result<(), RunError> {
    if file_digest(&record.corpus).as_ref() != Some(&record.corpus_digest) {
        return Err(RunError::Drift(DriftOf::Corpus));
    }
    if file_digest(&record.entities).as_ref() != Some(&record.entity_digest) {
        return Err(RunError::Drift(DriftOf::Entities));
    }
    let (path, digest, _) = head_of(cache, root, record)?;
    if file_digest_at(&path).as_ref() != Some(&digest) {
        return Err(RunError::Drift(DriftOf::Inventory));
    }
    Ok(())
}

fn file_digest_at(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|b| hex16(fnv1a64(&b)))
}

fn file_digest(path: &str) -> Option<String> {
    file_digest_at(Path::new(path))
}

fn canonical(path: &Path) -> Result<String, RunError> {
    std::fs::canonicalize(path)
        .map_err(|e| RunError::Io(e.kind()))?
        .into_os_string()
        .into_string()
        .map_err(|_| RunError::Malformed("path"))
}

fn run_cache(root: &Path) -> Result<CultCache, RunError> {
    std::fs::create_dir_all(root).map_err(|_| RunError::Store)?;
    run_cache_over(SingleFileMessagePackBackingStore::new(root.join("run.cc")))
}

/// The run's cache over `store`; a test passes one that fails a chosen write.
fn run_cache_over(store: impl CacheBackingStore + 'static) -> Result<CultCache, RunError> {
    open_cache_over(store, |c| {
        c.register_entry_type::<RunRecord>()?;
        c.register_entry_type::<RoundRecord>()?;
        c.register_entry_type::<Decision>()
    })
    .map_err(|_| RunError::Store)
}

fn one_run(cache: &CultCache) -> Result<Option<RunRecord>, RunError> {
    Ok(cache
        .get_all::<RunRecord>()
        .map_err(|_| RunError::Store)?
        .into_iter()
        .next())
}

/// Copies `from` onto a `to` that must not exist: a root that already holds a
/// file of that name (the source itself, say) refuses before anything is written.
fn copy_new(from: &Path, to: &Path) -> Result<(), RunError> {
    let io = |e: std::io::Error| RunError::Io(e.kind());
    let mut source = std::fs::File::open(from).map_err(io)?;
    let mut dest = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)
        .map_err(io)?;
    if let Err(e) = std::io::copy(&mut source, &mut dest) {
        // The file is ours (create_new made it): a half copy must not block a retry.
        let _ = std::fs::remove_file(to);
        return Err(io(e));
    }
    Ok(())
}

impl RunStores {
    /// Pins the three stores at `root`, which must hold no run. A store file
    /// that does not exist is `Io(NotFound)`.
    pub fn create(
        root: &Path,
        label: &str,
        paths: RunPaths<'_>,
        model: &str,
        embedder: &str,
        reach: LowererReach,
        answerer_family: ModelFamily,
    ) -> Result<Self, RunError> {
        Self::create_between(
            root,
            label,
            paths,
            model,
            embedder,
            reach,
            answerer_family,
            || {},
        )
    }

    /// `create` with `between` run after the digests are taken and before the
    /// stores are opened: the seam that lets a test change a store in that window.
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)] // `create`'s six plus the test seam
    pub fn create_between(
        root: &Path,
        label: &str,
        paths: RunPaths<'_>,
        model: &str,
        embedder: &str,
        reach: LowererReach,
        answerer_family: ModelFamily,
        between: impl FnOnce(),
    ) -> Result<Self, RunError> {
        if !is_slug(label) {
            return Err(RunError::Malformed("label"));
        }
        if !is_attestation(model) {
            return Err(RunError::Malformed("model"));
        }
        if !is_attestation(embedder) {
            return Err(RunError::Malformed("embedder"));
        }
        let cache = run_cache(root)?;
        if one_run(&cache)?.is_some() {
            return Err(RunError::Exists);
        }
        let record = RunRecord {
            label: label.to_string(),
            corpus: canonical(paths.corpus)?,
            inventory: canonical(paths.inventory)?,
            entities: canonical(paths.entities)?,
            corpus_digest: String::new(),
            entity_digest: String::new(),
            prompt_digest: hex16(PROMPT_SOURCE_DIGEST),
            model: model.to_string(),
            embedder: embedder.to_string(),
            chrf_version: CHRF_VERSION.to_string(),
            inventory_digest: String::new(),
            lowerer_reach: reach,
            answerer_family,
        };
        // The run works on its own copy; the caller's store is never opened again.
        let head = root.join(INVENTORY_FILE);
        copy_new(Path::new(&record.inventory), &head)?;
        // The copy is ours from here: any later failure takes it away again, or
        // it would refuse every retry, and so with the lock the open left beside it.
        let pinned = (|| {
            let record = RunRecord {
                corpus_digest: file_digest(&record.corpus).ok_or(RunError::Store)?,
                entity_digest: file_digest(&record.entities).ok_or(RunError::Store)?,
                inventory_digest: file_digest_at(&head).ok_or(RunError::Store)?,
                ..record
            };
            between();
            let mut run = Self::with(root, cache, record)?;
            // The stores were read between the digests: unless they still match
            // the pins, what was opened is not what was pinned. Nothing is
            // recorded for a run that did not pin.
            check_pins(&run.cache, root, &run.record)?;
            run.cache
                .put(format!("run:{label}"), &run.record)
                .map_err(|_| RunError::Store)?;
            Ok(run)
        })();
        if pinned.is_err() {
            discard_inventory(root);
        }
        pinned
    }

    /// Reopens the run at `root` from its record, refusing when a pinned store
    /// or the prompt source (`PROMPT_SOURCE_DIGEST`) no longer matches.
    pub fn open(root: &Path) -> Result<Self, RunError> {
        Self::open_between(root, || {})
    }

    /// `open` with `between` run after the pins are checked and before the
    /// stores are opened: the seam that lets a test change a store in that window.
    #[doc(hidden)]
    pub fn open_between(root: &Path, between: impl FnOnce()) -> Result<Self, RunError> {
        if !root.join("run.cc").is_file() {
            return Err(RunError::NoRun);
        }
        let cache = run_cache(root)?;
        let record = one_run(&cache)?.ok_or(RunError::NoRun)?;
        if record.prompt_digest != hex16(PROMPT_SOURCE_DIGEST) {
            return Err(RunError::Drift(DriftOf::Prompts));
        }
        check_pins(&cache, root, &record)?;
        between();
        let run = Self::with(root, cache, record)?;
        // Checking and opening are two reads of each file: the opened stores
        // are the pinned ones only if the pins still hold after the open.
        check_pins(&run.cache, root, &run.record)?;
        Ok(run)
    }

    fn with(root: &Path, cache: CultCache, record: RunRecord) -> Result<Self, RunError> {
        let store = |_| RunError::Store;
        let (head, _, _) = head_of(&cache, root, &record)?;
        Ok(Self {
            root: root.to_path_buf(),
            corpus: CorpusStore::open(Path::new(&record.corpus)).map_err(store)?,
            inventory: InventoryStore::open(&head).map_err(store)?,
            entities: EntityStore::open(Path::new(&record.entities)).map_err(store)?,
            cache,
            record,
        })
    }

    pub fn record(&self) -> &RunRecord {
        &self.record
    }

    /// `line` (a round or cycle summary) with how this run's lowerings were
    /// answered; a briefed-subagent run says its numbers are smoke-round only.
    pub fn labelled(&self, line: &str) -> String {
        let reach = self.record.lowerer_reach;
        match reach {
            LowererReach::BriefedSubagent => {
                format!("{line} lowering={} (smoke-round only)", reach.label())
            }
            LowererReach::ToolLessRelay => format!("{line} lowering={}", reach.label()),
        }
    }

    /// The line a recorded round prints, labelled with how the run was lowered.
    pub fn round_line(&self, round: &RoundRecord) -> String {
        self.labelled(&round.line())
    }

    /// The line a closed cycle prints, labelled the same way.
    pub fn cycle_line(&self, decision: &Decision) -> String {
        self.labelled(&decision.line())
    }

    pub fn inventory(&self) -> &InventoryStore {
        &self.inventory
    }

    /// The pinned entity store, for lowering and admission.
    pub fn entities(&self) -> &EntityStore {
        &self.entities
    }

    /// The live Train items of the pinned corpus.
    pub fn train(&self) -> anyhow::Result<TrainItems> {
        TrainItems::from_corpus(&self.corpus)
    }

    /// The offer for `key`; the inner error is the gate's refusal. An absent
    /// key is an error that names no key.
    pub fn offer(
        &self,
        key: &str,
    ) -> anyhow::Result<Result<EncoderOffer<'static>, crate::context::OfferRefusal>> {
        let item = self
            .corpus
            .get(key)?
            .ok_or_else(|| anyhow::anyhow!("no such item"))?;
        Ok(EncoderOffer::for_round(&item))
    }

    /// The live Train and Validation offers, in key order. Items the gate
    /// refuses are skipped and read no further.
    pub fn offers(&self) -> anyhow::Result<Vec<EncoderOffer<'static>>> {
        Ok(self
            .corpus
            .all()?
            .iter()
            .filter_map(|item| EncoderOffer::for_round(item).ok())
            .collect())
    }

    /// The recorded rounds, in order.
    pub fn rounds(&self) -> anyhow::Result<Vec<RoundRecord>> {
        let mut rounds = self.cache.get_all::<RoundRecord>()?;
        rounds.sort_by_key(RoundRecord::n);
        Ok(rounds)
    }

    /// The recorded decisions, by cycle.
    pub fn decisions(&self) -> anyhow::Result<Vec<Decision>> {
        decisions_of(&self.cache)
    }

    /// The head inventory file, its digest and its establishing round.
    fn head(&self) -> Result<(PathBuf, String, u32), RunError> {
        head_of(&self.cache, &self.root, &self.record)
    }

    /// `<root>/cycle-<k>` for the next undecided cycle `k`.
    pub fn next_cycle_dir(&self) -> anyhow::Result<PathBuf> {
        Ok(self
            .root
            .join(format!("cycle-{}", self.decisions()?.len() + 1)))
    }

    /// `<root>/round-<n>` for the next unrecorded round `n`.
    pub fn next_round_dir(&self) -> anyhow::Result<PathBuf> {
        Ok(self
            .root
            .join(format!("round-{}", self.rounds()?.len() + 1)))
    }
}

fn decisions_of(cache: &CultCache) -> anyhow::Result<Vec<Decision>> {
    let mut decisions = cache.get_all::<Decision>()?;
    decisions.sort_by_key(Decision::k);
    Ok(decisions)
}

/// The head is derived, never stored: the latest Accepted decision names it
/// (`<root>/cycle-<k>/inventory.cc`, its candidate digest, its candidate round);
/// with none it is the copy `create` made and round 1.
fn head_of(
    cache: &CultCache,
    root: &Path,
    record: &RunRecord,
) -> Result<(PathBuf, String, u32), RunError> {
    let decisions = decisions_of(cache).map_err(|_| RunError::Store)?;
    let accepted = decisions.iter().rfind(|d| d.verdict == Verdict::Accepted);
    match accepted {
        None => Ok((
            root.join(INVENTORY_FILE),
            record.inventory_digest.clone(),
            1,
        )),
        Some(d) => Ok((
            root.join(format!("cycle-{}", d.k)).join(INVENTORY_FILE),
            d.candidate_inventory_digest
                .clone()
                .ok_or(RunError::Store)?,
            d.candidate_round.ok_or(RunError::Store)?,
        )),
    }
}

/// What a round came to. One is built per call and matched at once, so the
/// size of the record is not worth an indirection.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum RoundOutcome {
    /// Prompts nobody has answered; nothing was written to the run or admitted.
    Unanswered {
        prompts: Vec<String>,
    },
    Recorded(RoundRecord),
}

/// A recall reading (`lowering-recall-diagnostic`): how closely a fresh model
/// completes each live Train and Validation item from its opening alone, chrF against the
/// item's own second half. Recorded nowhere: the answers stay in `<root>/recall`.
/// It is never a gate; a high reading says a round's scores on these items are
/// recitation.
#[derive(Clone, Debug, PartialEq)]
pub struct RecallReading {
    /// Items probed, Train and Validation.
    pub n: u32,
    /// Items whose completion was a usable answer; a refused one counts 0.
    pub answered: u32,
    pub chrf: f64,
}

impl RecallReading {
    pub fn line(&self) -> String {
        format!(
            "recall n={} answered={} chrf={:.2} (completion from the opening; unrelated English already scores chrf 14 to 20, so a reading near that floor is no recitation and one well above it is)",
            self.n, self.answered, self.chrf
        )
    }
}

#[derive(Debug)]
pub enum RecallOutcome {
    Unanswered { prompts: Vec<String> },
    Read(RecallReading),
}

/// The opening and the rest of `text`, split between words at about the
/// middle; `None` when it has fewer than four words.
fn recall_split(text: &str) -> Option<(String, String)> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() < 4 {
        return None;
    }
    let (head, tail) = words.split_at(words.len() / 2);
    Some((head.join(" "), tail.join(" ")))
}

impl RunStores {
    /// Probes every live Train and Validation item of the pinned corpus once,
    /// through `model`. A public-domain item names its work, author and
    /// translator; any other says `work: not published`.
    pub fn recall(&self, model: &mut dyn ModelPort) -> Result<RecallOutcome, RunError> {
        let store = |_| RunError::Store;
        let mut probes = Vec::new();
        for item in self.corpus.all().map_err(store)? {
            let provenance = match &item.source {
                Source::PublicDomain {
                    work,
                    author,
                    translator,
                    ..
                } => format!("work: {work}\nauthor: {author}\ntranslator: {translator}"),
                _ => "work: not published".to_string(),
            };
            if EncoderOffer::for_round(&item).is_err() {
                continue;
            }
            if let Some((opening, rest)) = recall_split(&item.text) {
                let prompt = Prompt::recall(&item.id, &provenance, &opening);
                probes.push((prompt, rest));
            }
        }
        let mut unanswered = Vec::new();
        let mut scores = Vec::new();
        for (prompt, rest) in &probes {
            match model.answer(prompt) {
                Ok(answer) => {
                    let text = LoweringDraft::parse(&answer)
                        .ok()
                        .filter(LoweringDraft::usable)
                        .map(|d| d.text);
                    scores.push(text.map(|t| chrf(&t, rest)));
                }
                Err(PortError::Unanswered) => unanswered.push(prompt.digest()),
                Err(PortError::Io(ErrorKind::InvalidData)) => scores.push(None),
                Err(PortError::Io(kind)) => return Err(RunError::Io(kind)),
            }
        }
        if !unanswered.is_empty() {
            return Ok(RecallOutcome::Unanswered { prompts: unanswered });
        }
        let n = scores.len() as u32;
        let answered = scores.iter().flatten().count() as u32;
        let chrf = if n == 0 {
            0.0
        } else {
            scores.iter().map(|s| s.unwrap_or(0.0)).sum::<f64>() / f64::from(n)
        };
        Ok(RecallOutcome::Read(RecallReading { n, answered, chrf }))
    }

    /// `<root>/recall`, where a recall's prompts and answers live.
    pub fn recall_dir(&self) -> PathBuf {
        self.root.join("recall")
    }
}

/// What the lowerer's answer did to each variant of a packet's stance, per
/// split. Means only; no key, text or stance value.
#[derive(Clone, Debug, PartialEq)]
pub struct AblationReading {
    /// Items whose recorded and replayed lowerings were both usable.
    pub n: u32,
    /// Answers dropped as unusable, replay or variant.
    pub unusable: u32,
    /// Mean agreement of the replay with the recorded lowering: the noise
    /// floor a variant is read against.
    pub replay: Agreement,
    pub drop: VariantReading,
    pub permute: VariantReading,
}

/// Means over an empty set are `None`, printed `none`: no measurement, never
/// a zero.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Agreement {
    pub chrf: Option<f64>,
    pub meaning: Option<f64>,
}

/// One variant's reading: agreement with the recorded lowering, pairs whose
/// chrF fell below the replay's, and the mean fidelity the item's own stance
/// adds (`score(item, replay) - score(item, variant)`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct VariantReading {
    pub n: u32,
    pub chrf: Option<f64>,
    pub meaning: Option<f64>,
    pub moved: u32,
    pub ref_gain_chrf: Option<f64>,
    pub ref_gain_meaning: Option<f64>,
}

fn show(x: Option<f64>, places: usize) -> String {
    x.map_or("none".to_string(), |x| format!("{x:.places$}"))
}

impl AblationReading {
    pub fn line(&self) -> String {
        let variant = |name: &str, v: &VariantReading| {
            format!(
                "{name} n={} chrf={} meaning={} moved={} ref_gain_chrf={} ref_gain_meaning={}",
                v.n,
                show(v.chrf, 2),
                show(v.meaning, 3),
                v.moved,
                show(v.ref_gain_chrf, 2),
                show(v.ref_gain_meaning, 3)
            )
        };
        format!(
            "ablation n={} unusable={} replay chrf={} meaning={} | {} | {}",
            self.n,
            self.unusable,
            show(self.replay.chrf, 2),
            show(self.replay.meaning, 3),
            variant("drop", &self.drop),
            variant("permute", &self.permute),
        )
    }
}

#[derive(Debug)]
pub enum AblationOutcome {
    Unanswered { prompts: Vec<String> },
    /// Train then Validation.
    Read(Vec<(Split, AblationReading)>),
}

/// What the port made of one lowering prompt.
enum Asked {
    Text(String),
    Unusable,
    Waiting,
}

fn asked(port: &mut dyn ModelPort, prompt: &Prompt) -> Result<Asked, RunError> {
    match port.answer(prompt) {
        Ok(a) => Ok(LoweringDraft::parse(&a)
            .ok()
            .filter(LoweringDraft::usable)
            .map_or(Asked::Unusable, |d| Asked::Text(d.text))),
        Err(PortError::Io(ErrorKind::InvalidData)) => Ok(Asked::Unusable),
        Err(PortError::Unanswered) => Ok(Asked::Waiting),
        Err(PortError::Io(kind)) => Err(RunError::Io(kind)),
    }
}

fn mean(xs: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, n) = xs.fold((0.0, 0u32), |(s, n), x| (s + x, n + 1));
    (n > 0).then(|| sum / f64::from(n))
}

type Scored = (ItemScore, ItemScore);

/// One item's reference text, recorded lowering, replay, and drop and permute
/// lowerings.
type Lowered = (String, String, String, [Option<String>; 2]);

impl RunStores {
    /// `<root>/ablation-<n>`, where a stance ablation's prompts, answers and
    /// variant packet stores live.
    pub fn ablation_dir(&self, n: u32) -> PathBuf {
        self.root.join(format!("ablation-{n}"))
    }

    /// A diagnostic like [`recall`](Self::recall), never a gate: for each live
    /// Train and Validation packet of plain round `n`, asks the lowerer again
    /// (`model`, the replay), with the packet's stance emptied (drop) and with
    /// the next item's stance in its split (permute), and reads how far the
    /// lowering moves against the round's own answer (`recorded`). Test and
    /// withdrawn items are never asked. Records nothing and writes nothing in
    /// `round-<n>`; the variant stores are rebuilt each call. An answer that
    /// is not a usable lowering drops its pair and is counted in `unusable`.
    pub fn stance_ablation<E: Embedder>(
        &self,
        n: u32,
        recorded: &mut dyn ModelPort,
        model: &mut dyn ModelPort,
        scorer: &Scorer<E>,
    ) -> Result<AblationOutcome, RunError> {
        let store = |_| RunError::Store;
        check_drift(self, model, scorer)?;
        self.rounds()
            .map_err(store)?
            .into_iter()
            .find(|r| r.n() == n && r.cycle().is_none())
            .ok_or(RunError::Malformed("round"))?;
        let inventory = self.inventory.inventory().map_err(store)?;

        let dir = self.ablation_dir(n);
        std::fs::create_dir_all(&dir).map_err(|e| RunError::Io(e.kind()))?;
        for name in ["base", "drop", "permute"] {
            for suffix in ["cc", "cc.lock"] {
                let _ = std::fs::remove_file(dir.join(format!("{name}.{suffix}")));
            }
        }
        let mut stores = [
            PacketStore::open(&dir.join("base.cc")).map_err(store)?,
            PacketStore::open(&dir.join("drop.cc")).map_err(store)?,
            PacketStore::open(&dir.join("permute.cc")).map_err(store)?,
        ];

        // Live Train and Validation packets, in item key order within a split.
        let round_packets = PacketStore::open(&self.root.join(format!("round-{n}")).join("packets.cc"))
            .map_err(store)?
            .all()
            .map_err(store)?;
        let mut splits: [(Split, Vec<(CorpusItem, Packet)>); 2] =
            [(Split::Train, Vec::new()), (Split::Validation, Vec::new())];
        for packet in round_packets {
            let item = self
                .corpus
                .get(packet.provenance.corpus_item.as_str())
                .map_err(store)?
                .ok_or(RunError::Store)?;
            for (split, held) in &mut splits {
                if SplitScheme::V1.live(&item) == Some(*split) {
                    held.push((item.clone(), packet.clone()));
                }
            }
        }

        let mut waiting: Vec<String> = Vec::new();
        let mut readings = Vec::new();
        // Texts per split: (item text, recorded, replay, [drop, permute]).
        let mut texts: Vec<Vec<Lowered>> = Vec::new();
        let mut unusable = [0u32; 2];
        for (at, (_, held)) in splits.iter_mut().enumerate() {
            held.sort_by(|a, b| a.0.id.as_str().cmp(b.0.id.as_str()));
            let mut rows = Vec::new();
            for (i, (item, packet)) in held.iter().enumerate() {
                let packet_item = packet.provenance.corpus_item.clone();
                let mut prompt_of = |slot: usize, packet: Packet| -> Result<Prompt, RunError> {
                    let view = stores[slot]
                        .admit(packet, &self.inventory, &self.corpus, &self.entities)
                        .map_err(|_| RunError::Store)?;
                    Prompt::lower(&view, &packet_item, &inventory, &self.entities)
                        .map_err(|_| RunError::Store)
                };
                let base = prompt_of(0, packet.clone())?;
                let mut variants: [Option<Prompt>; 2] = [None, None];
                if !packet.stance.features.is_empty() {
                    let mut p = packet.clone();
                    p.stance = Stance::default();
                    variants[0] = Some(prompt_of(1, p)?);
                }
                if held.len() > 1 {
                    let mut p = packet.clone();
                    p.stance = held[(i + 1) % held.len()].1.stance.clone();
                    let prompt = prompt_of(2, p)?;
                    if prompt.digest() != base.digest() {
                        variants[1] = Some(prompt);
                    }
                }
                // The round's own answer must exist; a missing one is not a gap to ask for.
                let recorded_text = match asked(recorded, &base)? {
                    Asked::Waiting => return Err(RunError::Unrecorded),
                    other => other,
                };
                let replay = asked(model, &base)?;
                let mut got: [Option<String>; 2] = [None, None];
                for (slot, prompt) in got.iter_mut().zip(&variants) {
                    let Some(prompt) = prompt else { continue };
                    match asked(model, prompt)? {
                        Asked::Text(t) => *slot = Some(t),
                        Asked::Unusable => unusable[at] += 1,
                        Asked::Waiting => {
                            let d = prompt.digest();
                            if !waiting.contains(&d) {
                                waiting.push(d);
                            }
                        }
                    }
                }
                match (recorded_text, replay) {
                    (Asked::Text(b), Asked::Text(r)) => {
                        rows.push((item.text.clone(), b, r, got));
                    }
                    (_, Asked::Waiting) => {
                        let d = base.digest();
                        if !waiting.contains(&d) {
                            waiting.push(d);
                        }
                    }
                    _ => unusable[at] += 1,
                }
            }
            texts.push(rows);
        }
        if !waiting.is_empty() {
            return Ok(AblationOutcome::Unanswered { prompts: waiting });
        }

        // One score call: for each item, the replay then each variant, each
        // against the recorded lowering (agreement) and the item (fidelity).
        let mut inputs: Vec<ScoreInput> = Vec::new();
        for (reference, base, replay, got) in texts.iter().flatten() {
            for output in std::iter::once(replay).chain(got.iter().flatten()) {
                inputs.push(ScoreInput { reference: base, output });
                inputs.push(ScoreInput { reference, output });
            }
        }
        let mut scores = if inputs.is_empty() {
            Vec::new()
        } else {
            scorer.score(&inputs).map_err(|_| RunError::Embed)?
        }
        .into_iter();
        let mut pair = || -> Result<Scored, RunError> {
            Ok((scores.next().ok_or(RunError::Store)?, scores.next().ok_or(RunError::Store)?))
        };
        for (at, ((split, _), rows)) in splits.iter().zip(&texts).enumerate() {
            let mut replay: Vec<Scored> = Vec::new();
            let mut variants: [Vec<(Scored, Scored)>; 2] = [Vec::new(), Vec::new()];
            for (_, _, _, got) in rows {
                let r = pair()?;
                for (k, g) in got.iter().enumerate() {
                    if g.is_some() {
                        variants[k].push((r.clone(), pair()?));
                    }
                }
                replay.push(r);
            }
            let variant = |v: &[(Scored, Scored)]| VariantReading {
                n: v.len() as u32,
                chrf: mean(v.iter().map(|(_, x)| x.0.chrf)),
                meaning: mean(v.iter().filter_map(|(_, x)| x.0.meaning)),
                moved: v.iter().filter(|(r, x)| x.0.chrf < r.0.chrf).count() as u32,
                ref_gain_chrf: mean(v.iter().map(|(r, x)| r.1.chrf - x.1.chrf)),
                ref_gain_meaning: mean(
                    v.iter().filter_map(|(r, x)| Some(r.1.meaning? - x.1.meaning?)),
                ),
            };
            readings.push((
                *split,
                AblationReading {
                    n: rows.len() as u32,
                    unusable: unusable[at],
                    replay: Agreement {
                        chrf: mean(replay.iter().map(|r| r.0.chrf)),
                        meaning: mean(replay.iter().filter_map(|r| r.0.meaning)),
                    },
                    drop: variant(&variants[0]),
                    permute: variant(&variants[1]),
                },
            ));
        }
        Ok(AblationOutcome::Read(readings))
    }
}

/// The distinct rule codes an admission refusal broke: validator rules by
/// code, a store refusal by its variant name. Nothing from the packet or the
/// stores is kept; an I/O failure is `io`.
fn admit_rule_codes(e: &StoreError) -> BTreeSet<String> {
    match e {
        StoreError::Invalid(rs) => rs.iter().map(|r| r.rule().to_string()).collect(),
        StoreError::Refused(r) => [r
            .to_string()
            .split([' ', ':'])
            .next()
            .unwrap_or_default()
            .to_string()]
        .into(),
        StoreError::Io(_) => ["io".to_string()].into(),
    }
}

/// How one item fared: where it was refused, or the text it lowered to, with
/// the key of the packet that was admitted for it.
enum Fate {
    Refused(Stage, Option<String>),
    Lowered(String, String),
}

/// One round: encodes every live Train and Validation item of the run's pinned
/// corpus through `model`, admits each packet into the round's own
/// `<root>/round-<n>/packets.cc`, lowers each admitted view through the same
/// port, scores, and records the round. The round has two answer phases, all
/// or nothing: while any encode or lowering prompt is unanswered, nothing
/// reaches `run.cc` and the round directory keeps no `packets.cc`. A round that
/// fails before its record is put (the embedder down) leaves no `packets.cc`
/// behind, so a rerun with revised answers starts clean. A mean
/// counts an item that was refused or did not lower as 0, so refusing a weak
/// item cannot raise it; `scored` counts beside it say how many were real.
pub fn round<M: ModelPort, E: Embedder>(
    run: &mut RunStores,
    model: &mut M,
    scorer: &Scorer<E>,
) -> Result<RoundOutcome, RunError> {
    check_drift(run, model, scorer)?;
    let store = |_| RunError::Store;
    let dir = run.next_round_dir().map_err(store)?;
    let (_, digest, _) = run.head()?;
    match score_round(run, &run.inventory, &digest, &dir, None, model, scorer)? {
        RoundOutcome::Recorded(record) => {
            run.cache
                .put(
                    format!("run:{}:round:{}", run.record.label, record.n),
                    &record,
                )
                .map_err(store)?;
            Ok(RoundOutcome::Recorded(record))
        }
        unanswered => Ok(unanswered),
    }
}

/// The model and embedder the run pinned are the ones answering.
fn check_drift<M: ModelPort + ?Sized, E: Embedder>(
    run: &RunStores,
    model: &M,
    scorer: &Scorer<E>,
) -> Result<(), RunError> {
    if model.model() != run.record.model {
        return Err(RunError::Drift(DriftOf::Model));
    }
    if scorer.embedder().model() != run.record.embedder {
        return Err(RunError::Drift(DriftOf::Embedder));
    }
    Ok(())
}

/// Encodes, admits, lowers and scores every offered item under `inventory`
/// (whose file digests to `inventory_digest`), with packets in `dir`, and
/// returns the record unput. `Recorded` here means built, not stored.
fn score_round<M: ModelPort, E: Embedder>(
    run: &RunStores,
    inventory_store: &InventoryStore,
    inventory_digest: &str,
    dir: &Path,
    cycle: Option<u32>,
    model: &mut M,
    scorer: &Scorer<E>,
) -> Result<RoundOutcome, RunError> {
    let store = |_| RunError::Store;
    let inventory = inventory_store.inventory().map_err(store)?;
    let rev = inventory.latest().ok_or(RunError::NoInventory)?.n;

    let train = run.train().map_err(store)?;
    let mut calls = Vec::new();
    let mut answers = Vec::new();
    let mut unanswered = Vec::new();
    for offer in run.offers().map_err(store)? {
        let prompt = Prompt::encode(&offer, &train, &inventory, rev);
        match model.answer(&prompt) {
            Ok(answer) => answers.push((offer.key().to_string(), Some(answer))),
            // An answer that is not UTF-8 is that item's parse refusal.
            Err(PortError::Io(ErrorKind::InvalidData)) => {
                answers.push((offer.key().to_string(), None))
            }
            Err(PortError::Unanswered) => unanswered.push(prompt.digest()),
            Err(PortError::Io(kind)) => return Err(RunError::Io(kind)),
        }
        calls.push(Call {
            prompt: prompt.digest(),
            items: prompt.item_keys().to_vec(),
        });
    }
    if !unanswered.is_empty() {
        return Ok(RoundOutcome::Unanswered {
            prompts: unanswered,
        });
    }

    std::fs::create_dir_all(dir).map_err(|e| RunError::Io(e.kind()))?;
    // The directory of an unrecorded round holds only a failed attempt's packets.
    discard_packets(dir);
    let mut packets = PacketStore::open(&dir.join("packets.cc")).map_err(store)?;
    let encoder = encoder_id(model.model(), &run.record.prompt_digest);
    let mut items: Vec<CorpusItem> = Vec::new();
    let mut fates: Vec<Fate> = Vec::new();
    let mut admit_rules: BTreeMap<String, u32> = BTreeMap::new();
    let mut parse_reasons: BTreeMap<String, u32> = BTreeMap::new();
    // Phase 1 ends at admission: each admitted view waits for its lowering.
    let mut admitted: Vec<(usize, CorpusItemId, String, LoweringInput)> = Vec::new();
    for (key, answer) in &answers {
        let item = run.corpus.get(key).map_err(store)?.ok_or(RunError::Store)?;
        let id = CorpusItemId::parse(key).map_err(|_| RunError::Store)?;
        let fate = match answer.as_deref().map(PacketDraft::parse) {
            None => {
                *parse_reasons.entry("unreadable".to_string()).or_default() += 1;
                Fate::Refused(Stage::Parse, None)
            }
            Some(Err(e)) => {
                *parse_reasons.entry(e.class.code().to_string()).or_default() += 1;
                Fate::Refused(Stage::Parse, None)
            }
            Some(Ok(draft)) => {
                let packet = draft.into_packet(&id, rev, &encoder);
                let packet_key = packet.id.to_string();
                match packets.admit(packet, inventory_store, &run.corpus, &run.entities) {
                    Err(e) => {
                        for rule in admit_rule_codes(&e) {
                            *admit_rules.entry(rule).or_default() += 1;
                        }
                        Fate::Refused(Stage::Admit, None)
                    }
                    Ok(view) => {
                        admitted.push((fates.len(), id, packet_key, view));
                        // Replaced below by what the lowering answers.
                        Fate::Refused(Stage::Lower, None)
                    }
                }
            }
        };
        items.push(item);
        fates.push(fate);
    }

    // Phase 2: one lowering prompt per admitted view, answered through the
    // port. Nothing is kept unless every one is answered.
    let mut unanswered = Vec::new();
    for (at, id, packet_key, view) in &admitted {
        let Ok(prompt) = Prompt::lower(view, id, &inventory, &run.entities) else {
            fates[*at] = Fate::Refused(Stage::Lower, Some(packet_key.clone()));
            continue;
        };
        calls.push(Call {
            prompt: prompt.digest(),
            items: prompt.item_keys().to_vec(),
        });
        fates[*at] = match model.answer(&prompt) {
            Ok(answer) => match LoweringDraft::parse(&answer) {
                Ok(draft) if draft.usable() => {
                    Fate::Lowered(draft.text, packet_key.clone())
                }
                _ => Fate::Refused(Stage::Lower, Some(packet_key.clone())),
            },
            Err(PortError::Io(ErrorKind::InvalidData)) => {
                Fate::Refused(Stage::Lower, Some(packet_key.clone()))
            }
            Err(PortError::Unanswered) => {
                // Two views with one meaning share one prompt: name it once.
                if !unanswered.contains(&prompt.digest()) {
                    unanswered.push(prompt.digest());
                }
                continue;
            }
            Err(PortError::Io(kind)) => {
                discard_packets(dir);
                return Err(RunError::Io(kind));
            }
        };
    }
    if !unanswered.is_empty() {
        discard_packets(dir);
        return Ok(RoundOutcome::Unanswered {
            prompts: unanswered,
        });
    }

    // One score call over what lowered; none when nothing did.
    let lowered: Vec<(usize, &str)> = fates
        .iter()
        .enumerate()
        .filter_map(|(i, fate)| match fate {
            Fate::Lowered(text, _) => Some((i, text.as_str())),
            Fate::Refused(..) => None,
        })
        .collect();
    let inputs: Vec<ScoreInput> = lowered
        .iter()
        .map(|(i, text)| ScoreInput {
            reference: &items[*i].text,
            output: text,
        })
        .collect();
    let scores = if inputs.is_empty() {
        Vec::new()
    } else {
        match scorer.score(&inputs) {
            Ok(scores) => scores,
            Err(_) => {
                discard_packets(dir);
                return Err(RunError::Embed);
            }
        }
    };
    let mut score_at: Vec<Option<ItemScore>> = vec![None; items.len()];
    for ((i, _), score) in lowered.iter().zip(scores) {
        score_at[*i] = Some(score);
    }
    // An item that did not lower scores 0 in every mean.
    let nothing = ItemScore {
        chrf: 0.0,
        meaning: Some(0.0),
    };
    let scored: Vec<(&CorpusItem, ItemScore)> = items
        .iter()
        .zip(&score_at)
        .map(|(item, score)| (item, score.clone().unwrap_or_else(|| nothing.clone())))
        .collect();
    let means = aggregate(&scored);

    let mut train_rows = Vec::new();
    let mut refused = RefusalCounts::default();
    let (mut train_scored, mut validation_scored) = (0, 0);
    for ((item, fate), score) in items.iter().zip(&fates).zip(&score_at) {
        let (outcome, packet) = match (fate, score) {
            (Fate::Lowered(_, key), Some(score)) => (
                Outcome::Scored {
                    chrf: score.chrf,
                    meaning: score.meaning,
                },
                Some(key.clone()),
            ),
            (Fate::Refused(stage, key), _) => (Outcome::Refused(*stage), key.clone()),
            (Fate::Lowered(..), None) => return Err(RunError::Store),
        };
        match (SplitScheme::V1.live(item), fate) {
            (Some(Split::Train), fate) => {
                train_scored += u32::from(matches!(fate, Fate::Lowered(..)));
                train_rows.push(TrainRow {
                    item: item.id.clone(),
                    packet,
                    outcome,
                });
            }
            (Some(Split::Validation), Fate::Lowered(..)) => validation_scored += 1,
            (Some(Split::Validation), Fate::Refused(stage, _)) => match stage {
                Stage::Parse => refused.parse += 1,
                Stage::Admit => refused.admit += 1,
                Stage::Lower => refused.lower += 1,
            },
            _ => {}
        }
    }

    let own_family = match run.record.answerer_family {
        ModelFamily::Unrecorded => 0,
        family => items
            .iter()
            .filter(|item| {
                matches!(&item.source, Source::ModelWritten { family: f, .. } if *f == family)
            })
            .count() as u32,
    };
    let n = run.rounds().map_err(store)?.len() as u32 + 1;
    let record = RoundRecord {
        n,
        inventory_rev: rev,
        prompt_digest: run.record.prompt_digest.clone(),
        corpus_digest: run.record.corpus_digest.clone(),
        entity_digest: run.record.entity_digest.clone(),
        model: run.record.model.clone(),
        embedder: run.record.embedder.clone(),
        chrf_version: run.record.chrf_version.clone(),
        calls,
        train: train_rows,
        train_mean: means.train,
        validation_mean: means.validation,
        validation_refused: refused,
        train_scored,
        validation_scored,
        inventory_digest: inventory_digest.to_string(),
        cycle,
        admit_rules: admit_rules
            .into_iter()
            .map(|(rule, count)| RuleCount { rule, count })
            .collect(),
        parse_reasons: parse_reasons
            .into_iter()
            .map(|(rule, count)| RuleCount { rule, count })
            .collect(),
        own_family,
    };
    Ok(RoundOutcome::Recorded(record))
}

/// One validation reading: chrF 0 to 100, meaning 0 to 1, and their composite.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Reading {
    pub chrf: f64,
    pub meaning: f64,
    pub composite: f64,
}

impl Reading {
    /// `None` when the split has no items or no chrF; a missing meaning reads as
    /// 0. The composite is the mean of chrF as a fraction and meaning.
    pub fn of(mean: &SplitMean) -> Option<Self> {
        if mean.count == 0 {
            return None;
        }
        let chrf = mean.chrf?;
        let meaning = mean.meaning.unwrap_or(0.0);
        Some(Self {
            chrf,
            meaning,
            composite: (chrf / 100.0 + meaning) / 2.0,
        })
    }
}

/// Which metric an edit regressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Metric {
    Chrf,
    Meaning,
}

/// Why an edit was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Why {
    /// The answer was not an edit draft.
    EditParse,
    /// The draft asked for no change or too many, or the inventory door refused it.
    EditAdmit,
    NoGain,
    Regressed(Metric),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Verdict {
    Accepted,
    Rejected(Why),
}

/// The smallest composite gain that accepts an edit: the measured floor, but
/// never under one [`LADDER_STEP`], so a zero floor from identical replicates
/// does not let a rounding-error gain through.
pub fn threshold(floor: &Reading) -> f64 {
    floor.composite.max(LADDER_STEP)
}

/// A reading difference in billionths. [`decide`] compares whole numbers, so a
/// gain of exactly one [`LADDER_STEP`] is a tie on every baseline of the grid
/// instead of a coin the last bits of `0.505 - 0.5` toss; differences under a
/// billionth are not told apart.
fn nano(x: f64) -> i64 {
    (x * 1e9).round() as i64
}

/// The accept rule. A metric that falls by more than its tolerance (the floor,
/// at least one step) rejects; otherwise the composite must gain strictly more
/// than [`threshold`]. Means count a refused Validation item as 0, so refusing a
/// weak item cannot raise a reading. Differences are compared in billionths.
pub fn decide(base: &Reading, candidate: &Reading, floor: &Reading) -> Verdict {
    if nano(candidate.chrf - base.chrf) < -nano(floor.chrf.max(100.0 * LADDER_STEP)) {
        Verdict::Rejected(Why::Regressed(Metric::Chrf))
    } else if nano(candidate.meaning - base.meaning) < -nano(floor.meaning.max(LADDER_STEP)) {
        Verdict::Rejected(Why::Regressed(Metric::Meaning))
    } else if nano(candidate.composite - base.composite) > nano(threshold(floor)) {
        Verdict::Accepted
    } else {
        Verdict::Rejected(Why::NoGain)
    }
}

/// One closed edit-accept cycle: digests and numbers, no text. Key
/// `run:<label>:cycle:<k>`. Only [`cycle`] builds one.
///
/// ```
/// fn f(d: &weksa::run::Decision) -> u32 { d.k() }
/// ```
///
/// Every field is given, so the literal below fails only on privacy; its twin is
/// the same literal in the unit test `a_decision_literal_compiles_in_the_lib`.
///
/// ```compile_fail,E0451
/// use weksa::run::{Call, Decision, Reading, Verdict};
/// fn f() -> Decision {
///     let r = Reading { chrf: 0.0, meaning: 0.0, composite: 0.0 };
///     Decision {
///         k: 1,
///         base_round: 1,
///         base_rev: 1,
///         base_inventory_digest: String::new(),
///         edit: Call { prompt: String::new(), items: vec![] },
///         edit_answer_digest: None,
///         candidate_rev: None,
///         candidate_inventory_digest: None,
///         candidate_round: None,
///         base: r,
///         candidate: None,
///         floor: r,
///         threshold: 0.0,
///         verdict: Verdict::Accepted,
///     }
/// }
/// ```
#[derive(Clone, Debug, PartialEq, JsonSchema, DatabaseEntry)]
#[cultcache(type = "weksa.loop_decision")]
pub struct Decision {
    #[cultcache(key = 0)]
    k: u32,
    /// The head's establishing round: the base of this cycle.
    #[cultcache(key = 1)]
    base_round: u32,
    #[cultcache(key = 2)]
    base_rev: u32,
    #[cultcache(key = 3)]
    base_inventory_digest: String,
    /// The edit prompt's digest and the Train keys it held.
    #[cultcache(key = 4)]
    edit: Call,
    /// 16 hex of the editor's answer; `None` when it was unreadable.
    #[cultcache(key = 5)]
    edit_answer_digest: Option<String>,
    #[cultcache(key = 6)]
    candidate_rev: Option<u32>,
    #[cultcache(key = 7)]
    candidate_inventory_digest: Option<String>,
    #[cultcache(key = 8)]
    candidate_round: Option<u32>,
    #[cultcache(key = 9)]
    base: Reading,
    #[cultcache(key = 10)]
    candidate: Option<Reading>,
    #[cultcache(key = 11)]
    floor: Reading,
    #[cultcache(key = 12)]
    threshold: f64,
    #[cultcache(key = 13)]
    verdict: Verdict,
}

impl Decision {
    pub fn k(&self) -> u32 {
        self.k
    }
    pub fn base_round(&self) -> u32 {
        self.base_round
    }
    pub fn base_rev(&self) -> u32 {
        self.base_rev
    }
    pub fn base_inventory_digest(&self) -> &str {
        &self.base_inventory_digest
    }
    pub fn edit(&self) -> &Call {
        &self.edit
    }
    pub fn edit_answer_digest(&self) -> Option<&str> {
        self.edit_answer_digest.as_deref()
    }
    pub fn candidate_rev(&self) -> Option<u32> {
        self.candidate_rev
    }
    pub fn candidate_inventory_digest(&self) -> Option<&str> {
        self.candidate_inventory_digest.as_deref()
    }
    pub fn candidate_round(&self) -> Option<u32> {
        self.candidate_round
    }
    pub fn base(&self) -> &Reading {
        &self.base
    }
    pub fn candidate(&self) -> Option<&Reading> {
        self.candidate.as_ref()
    }
    pub fn floor(&self) -> &Reading {
        &self.floor
    }
    pub fn threshold(&self) -> f64 {
        self.threshold
    }
    pub fn verdict(&self) -> Verdict {
        self.verdict
    }

    /// The cycle line, unlabelled: [`RunStores::cycle_line`] adds how the run was lowered.
    fn line(&self) -> String {
        let candidate_rev = self
            .candidate_rev
            .map_or("none".to_string(), |n| n.to_string());
        let candidate = self
            .candidate
            .map_or("none".to_string(), |c| format!("{:.4}", c.composite));
        let verdict = match self.verdict {
            Verdict::Accepted => "accepted".to_string(),
            Verdict::Rejected(why) => format!(
                "rejected {}",
                match why {
                    Why::EditParse => "edit-parse",
                    Why::EditAdmit => "edit-admit",
                    Why::NoGain => "no-gain",
                    Why::Regressed(Metric::Chrf) => "regressed-chrf",
                    Why::Regressed(Metric::Meaning) => "regressed-meaning",
                }
            ),
        };
        format!(
            "cycle {} rev {}->{candidate_rev} base={:.4} candidate={candidate} floor={:.4} threshold={:.4} {verdict}",
            self.k, self.base_rev, self.base.composite, self.floor.composite, self.threshold
        )
    }
}

/// What a cycle came to; built once per call, as [`RoundOutcome`] is.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum CycleOutcome {
    /// Prompts nobody has answered; nothing was written to `run.cc`.
    Unanswered {
        prompts: Vec<String>,
    },
    Decided(Decision),
}

/// The spread of the Validation reading between rounds 1 and 2: two independent
/// encodings under the run's first inventory and one spec, measured once per run.
fn floor_of(rounds: &[RoundRecord], record: &RunRecord) -> Result<Reading, RunError> {
    let replicate = |n: u32| {
        let round = rounds
            .iter()
            .find(|r| {
                r.n == n && r.cycle.is_none() && r.inventory_digest == record.inventory_digest
            })
            .ok_or(RunError::NoFloor)?;
        Reading::of(&round.validation_mean).ok_or(RunError::NoValidation)
    };
    let (a, b) = (replicate(1)?, replicate(2)?);
    Ok(Reading {
        chrf: (a.chrf - b.chrf).abs(),
        meaning: (a.meaning - b.meaning).abs(),
        composite: (a.composite - b.composite).abs(),
    })
}

/// One edit-accept cycle. The editor is asked for one inventory edit from a
/// prompt built over Train items, the head inventory and an [`EditFeedback`];
/// the edit is admitted as a candidate revision in `<root>/cycle-<k>/inventory.cc`;
/// a candidate round scores it; [`decide`] accepts or rejects on Validation
/// against the head's establishing round. Accepting is one write: the candidate
/// round and the decision go into `run.cc` in one batch, and the decision names
/// the new head. While any prompt is unanswered, or the candidate round fails,
/// `run.cc` is untouched. An edit that fails is a rejected decision, not an error.
pub fn cycle<M: ModelPort, E: Embedder>(
    run: &mut RunStores,
    model: &mut M,
    scorer: &Scorer<E>,
) -> Result<CycleOutcome, RunError> {
    let store = |_| RunError::Store;
    check_drift(run, model, scorer)?;
    let rounds = run.rounds().map_err(store)?;
    let floor = floor_of(&rounds, &run.record)?;
    let (head_path, head_digest, base_round) = run.head()?;
    let base = rounds
        .iter()
        .find(|r| r.n == base_round)
        .ok_or(RunError::Store)?;
    let base = Reading::of(&base.validation_mean).ok_or(RunError::NoValidation)?;
    let decisions = run.decisions().map_err(store)?;
    let k = decisions.len() as u32 + 1;
    let dir = run.next_cycle_dir().map_err(store)?;
    let inventory = run.inventory.inventory().map_err(store)?;
    let base_rev = inventory.latest().ok_or(RunError::NoInventory)?.n;

    let feedback = EditFeedback::new(
        decisions.last().map(|d| d.verdict == Verdict::Accepted),
        Some(base.composite),
    );
    let train = run.train().map_err(store)?;
    let prompt = Prompt::mint(&train, &inventory, base_rev, &feedback);
    let answer = match model.answer(&prompt) {
        Ok(answer) => Some(answer),
        Err(PortError::Io(ErrorKind::InvalidData)) => None,
        Err(PortError::Unanswered) => {
            return Ok(CycleOutcome::Unanswered {
                prompts: vec![prompt.digest()],
            });
        }
        Err(PortError::Io(kind)) => return Err(RunError::Io(kind)),
    };

    let blank = |verdict: Verdict| Decision {
        k,
        base_round,
        base_rev,
        base_inventory_digest: head_digest.clone(),
        edit: Call {
            prompt: prompt.digest(),
            items: prompt.item_keys().to_vec(),
        },
        edit_answer_digest: answer.as_ref().map(|a| hex16(fnv1a64(a.as_bytes()))),
        candidate_rev: None,
        candidate_inventory_digest: None,
        candidate_round: None,
        base,
        candidate: None,
        floor,
        threshold: threshold(&floor),
        verdict,
    };
    let decision_key = format!("run:{}:cycle:{k}", run.record.label);
    // A rejection leaves no candidate inventory behind, whichever attempt built it.
    let reject = |run: &mut RunStores, why: Why| -> Result<CycleOutcome, RunError> {
        discard_inventory(&dir);
        let d = blank(Verdict::Rejected(why));
        run.cache.put(decision_key.clone(), &d).map_err(store)?;
        Ok(CycleOutcome::Decided(d))
    };

    let Some(draft) = answer.as_deref().and_then(|a| EditDraft::parse(a).ok()) else {
        return reject(run, Why::EditParse);
    };
    let Some((revision, entries)) = draft.into_revision(&inventory, base_rev) else {
        return reject(run, Why::EditAdmit);
    };
    let candidate_rev = revision.n;

    // The cycle's directory holds prompts and answers; its inventory and packets
    // are rebuilt from the head on every attempt.
    let candidate_file = dir.join(INVENTORY_FILE);
    std::fs::create_dir_all(&dir).map_err(|e| RunError::Io(e.kind()))?;
    discard_inventory(&dir);
    discard_packets(&dir);
    std::fs::copy(&head_path, &candidate_file).map_err(|e| RunError::Io(e.kind()))?;
    let mut candidate_store = InventoryStore::open(&candidate_file).map_err(store)?;
    if candidate_store.admit_revision(revision, entries).is_err() {
        drop(candidate_store);
        return reject(run, Why::EditAdmit);
    }
    let candidate_digest = file_digest_at(&candidate_file).ok_or(RunError::Store)?;

    let record = match score_round(
        run,
        &candidate_store,
        &candidate_digest,
        &dir,
        Some(k),
        model,
        scorer,
    )? {
        RoundOutcome::Unanswered { prompts } => return Ok(CycleOutcome::Unanswered { prompts }),
        RoundOutcome::Recorded(record) => record,
    };
    let candidate = Reading::of(&record.validation_mean).ok_or(RunError::NoValidation)?;
    let verdict = decide(&base, &candidate, &floor);
    let decided = Decision {
        candidate_rev: Some(candidate_rev),
        candidate_inventory_digest: Some(candidate_digest),
        candidate_round: Some(record.n),
        candidate: Some(candidate),
        ..blank(verdict)
    };
    let staged = vec![
        run.cache
            .prepare_entry_named(
                format!("run:{}:round:{}", run.record.label, record.n),
                &record,
            )
            .map_err(store)?
            .0,
        run.cache
            .prepare_entry_named(decision_key.clone(), &decided)
            .map_err(store)?
            .0,
    ];
    run.cache.put_prepared_batch(staged).map_err(store)?;
    if verdict == Verdict::Accepted {
        run.inventory = candidate_store;
    }
    Ok(CycleOutcome::Decided(decided))
}

/// Removes a round directory's packet store and its lock.
fn discard_packets(dir: &Path) {
    for name in ["packets.cc", "packets.cc.lock"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
}

/// Removes a cycle directory's candidate inventory and its lock.
fn discard_inventory(dir: &Path) {
    for name in [INVENTORY_FILE, "inventory.cc.lock"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The twin of the `RoundRecord` literal in its compile_fail doctest: the
    /// same fields, compiling here because the fields are private to this module.
    #[test]
    fn a_round_record_literal_compiles_in_the_lib() {
        let r = RoundRecord {
            n: 1,
            inventory_rev: 1,
            prompt_digest: String::new(),
            corpus_digest: String::new(),
            entity_digest: String::new(),
            model: String::new(),
            embedder: String::new(),
            chrf_version: String::new(),
            calls: vec![],
            train: vec![],
            train_mean: SplitMean::default(),
            train_scored: 0,
            validation_mean: SplitMean::default(),
            validation_scored: 0,
            validation_refused: RefusalCounts::default(),
            inventory_digest: String::new(),
            cycle: None,
            admit_rules: vec![],
            parse_reasons: vec![],
            own_family: 0,
        };
        assert_eq!(r.n(), 1);
    }

    /// The twin of the `Decision` literal in its compile_fail doctest.
    #[test]
    fn a_decision_literal_compiles_in_the_lib() {
        let r = Reading {
            chrf: 0.0,
            meaning: 0.0,
            composite: 0.0,
        };
        let d = Decision {
            k: 1,
            base_round: 1,
            base_rev: 1,
            base_inventory_digest: String::new(),
            edit: Call {
                prompt: String::new(),
                items: vec![],
            },
            edit_answer_digest: None,
            candidate_rev: None,
            candidate_inventory_digest: None,
            candidate_round: None,
            base: r,
            candidate: None,
            floor: r,
            threshold: 0.0,
            verdict: Verdict::Accepted,
        };
        assert_eq!(d.k(), 1);
    }

    fn reading(chrf: f64, meaning: f64, composite: f64) -> Reading {
        Reading {
            chrf,
            meaning,
            composite,
        }
    }

    #[test]
    fn reading_of_a_mean() {
        let mean = |count, chrf, meaning| SplitMean {
            count,
            chrf,
            meaning,
        };
        assert_eq!(Reading::of(&mean(0, Some(50.0), Some(0.5))), None);
        assert_eq!(Reading::of(&mean(2, None, Some(0.5))), None);
        assert_eq!(
            Reading::of(&mean(2, Some(50.0), Some(0.5))),
            Some(reading(50.0, 0.5, 0.5))
        );
        // A missing meaning reads as 0.
        assert_eq!(
            Reading::of(&mean(2, Some(80.0), None)),
            Some(reading(80.0, 0.0, 0.4))
        );
        assert_eq!(
            Reading::of(&mean(1, Some(0.0), Some(1.0)))
                .unwrap()
                .composite,
            0.5
        );
    }

    #[test]
    fn the_threshold_is_the_floor_but_never_under_a_step() {
        assert_eq!(threshold(&reading(0.0, 0.0, 0.0)), LADDER_STEP);
        assert_eq!(threshold(&reading(0.0, 0.0, 0.001)), LADDER_STEP);
        assert_eq!(threshold(&reading(0.0, 0.0, LADDER_STEP)), LADDER_STEP);
        assert_eq!(threshold(&reading(0.0, 0.0, 0.25)), 0.25);
    }

    #[test]
    fn decide_pins_each_branch() {
        let base = reading(50.0, 0.5, 0.5);
        let zero = reading(0.0, 0.0, 0.0);
        let rejected = Verdict::Rejected;

        // Floor 0: a chrF drop beyond 100 * LADDER_STEP regresses, inside it does not.
        let down = |chrf| reading(chrf, 0.5, 0.5);
        assert_eq!(
            decide(&base, &down(49.4), &zero),
            rejected(Why::Regressed(Metric::Chrf))
        );
        assert_eq!(decide(&base, &down(49.6), &zero), rejected(Why::NoGain));
        // The tolerance is the floor when that is larger: exactly at it stands.
        let floor = reading(8.0, 0.0, 0.0);
        assert_eq!(decide(&base, &down(42.0), &floor), rejected(Why::NoGain));
        assert_eq!(
            decide(&base, &down(41.99), &floor),
            rejected(Why::Regressed(Metric::Chrf))
        );
        // A floor under a step does not narrow it.
        let small = reading(0.1, 0.001, 0.0);
        assert_eq!(decide(&base, &down(49.6), &small), rejected(Why::NoGain));
        assert_eq!(
            decide(&base, &reading(50.0, 0.498, 0.5), &small),
            rejected(Why::NoGain)
        );
        // Meaning likewise: one step by default, the floor when larger.
        let meaning = |m| reading(50.0, m, 0.5);
        assert_eq!(
            decide(&base, &meaning(0.49), &zero),
            rejected(Why::Regressed(Metric::Meaning))
        );
        assert_eq!(decide(&base, &meaning(0.498), &zero), rejected(Why::NoGain));
        let floor = reading(0.0, 0.25, 0.0);
        assert_eq!(decide(&base, &meaning(0.25), &floor), rejected(Why::NoGain));
        assert_eq!(
            decide(&base, &meaning(0.2499), &floor),
            rejected(Why::Regressed(Metric::Meaning))
        );
        // A regression rejects whatever the composite gained; chrF is judged first.
        assert_eq!(
            decide(&base, &reading(40.0, 0.9, 0.9), &zero),
            rejected(Why::Regressed(Metric::Chrf))
        );
        assert_eq!(
            decide(&base, &reading(50.0, 0.4, 0.9), &zero),
            rejected(Why::Regressed(Metric::Meaning))
        );
        assert_eq!(
            decide(&base, &reading(40.0, 0.4, 0.1), &zero),
            rejected(Why::Regressed(Metric::Chrf))
        );

        // The gain must exceed the threshold strictly.
        let gain = |c| reading(50.0, 0.5, c);
        assert_eq!(decide(&base, &gain(0.504), &zero), rejected(Why::NoGain));
        assert_eq!(decide(&base, &gain(0.506), &zero), Verdict::Accepted);
        assert_eq!(decide(&base, &gain(0.5), &zero), rejected(Why::NoGain));
        assert_eq!(decide(&base, &gain(0.4), &zero), rejected(Why::NoGain));
        // A floor above the step sets the threshold: a gain equal to it is no gain.
        let floor = reading(0.0, 0.0, 0.25);
        assert_eq!(decide(&base, &gain(0.75), &floor), rejected(Why::NoGain));
        assert_eq!(decide(&base, &gain(0.7501), &floor), Verdict::Accepted);
        assert_eq!(decide(&base, &gain(0.506), &floor), rejected(Why::NoGain));
    }

    #[test]
    fn a_gain_of_exactly_one_step_is_no_gain_on_every_baseline_of_the_grid() {
        let zero = reading(0.0, 0.0, 0.0);
        let at = |i: u32| reading(50.0, 0.5, f64::from(i) * LADDER_STEP);
        for i in 0..400 {
            assert_eq!(
                decide(&at(i), &at(i + 1), &zero),
                Verdict::Rejected(Why::NoGain),
                "{i}"
            );
            // Two steps clears one; a step and a hair over clears it too.
            assert_eq!(decide(&at(i), &at(i + 2), &zero), Verdict::Accepted, "{i}");
            let hair = reading(50.0, 0.5, f64::from(i + 1) * LADDER_STEP + 1e-6);
            assert_eq!(decide(&at(i), &hair, &zero), Verdict::Accepted, "{i}");
        }
        // The floor is a tie at its own value on the grid, as the step is.
        let floor = reading(0.0, 0.0, 3.0 * LADDER_STEP);
        for i in 0..400 {
            assert_eq!(
                decide(&at(i), &at(i + 3), &floor),
                Verdict::Rejected(Why::NoGain),
                "{i}"
            );
            assert_eq!(decide(&at(i), &at(i + 4), &floor), Verdict::Accepted, "{i}");
        }
        // The regression tolerance is a tie at its own value, on both metrics.
        for i in 0..400 {
            let chrf = 20.0 + f64::from(i) * 0.5;
            let base = reading(chrf, 0.5, 0.5);
            let drop = reading(chrf - 100.0 * LADDER_STEP, 0.5, 0.5);
            assert_eq!(decide(&base, &drop, &zero), Verdict::Rejected(Why::NoGain));
            let meaning = f64::from(i + 1) * LADDER_STEP;
            let base = reading(50.0, meaning, 0.5);
            let drop = reading(50.0, meaning - LADDER_STEP, 0.5);
            assert_eq!(decide(&base, &drop, &zero), Verdict::Rejected(Why::NoGain));
        }
        // Two-decimal chrF readings are not binary-exact: in float, 84 of these
        // pairs would read as a regression of more than 0.5.
        for i in 60..10_000 {
            let base = reading(f64::from(i) / 100.0, 0.5, 0.5);
            let drop = reading(f64::from(i - 50) / 100.0, 0.5, 0.5);
            assert_eq!(
                decide(&base, &drop, &zero),
                Verdict::Rejected(Why::NoGain),
                "{i}"
            );
        }
    }

    fn replicate(n: u32, digest: &str, chrf: f64, meaning: f64) -> RoundRecord {
        RoundRecord {
            n,
            inventory_rev: 1,
            prompt_digest: String::new(),
            corpus_digest: String::new(),
            entity_digest: String::new(),
            model: String::new(),
            embedder: String::new(),
            chrf_version: String::new(),
            calls: vec![],
            train: vec![],
            train_mean: SplitMean::default(),
            train_scored: 0,
            validation_mean: SplitMean {
                count: 2,
                chrf: Some(chrf),
                meaning: Some(meaning),
            },
            validation_scored: 2,
            validation_refused: RefusalCounts::default(),
            inventory_digest: digest.to_string(),
            cycle: None,
            admit_rules: vec![],
            parse_reasons: vec![],
            own_family: 0,
        }
    }

    fn record_of(digest: &str) -> RunRecord {
        RunRecord {
            label: "t".into(),
            corpus: String::new(),
            inventory: String::new(),
            entities: String::new(),
            corpus_digest: String::new(),
            entity_digest: String::new(),
            prompt_digest: String::new(),
            model: String::new(),
            embedder: String::new(),
            chrf_version: String::new(),
            inventory_digest: digest.to_string(),
            lowerer_reach: LowererReach::ToolLessRelay,
            answerer_family: ModelFamily::Unrecorded,
        }
    }

    /// The floor is the spread of each field of the two replicates, the composite
    /// as the spread of the composites: with chrF up and meaning down it is not
    /// a function of the other two spreads' magnitudes.
    #[test]
    fn the_floor_is_the_spread_of_each_field_of_the_two_replicates() {
        let record = record_of("d1");
        let rounds = [replicate(1, "d1", 60.0, 0.5), replicate(2, "d1", 50.0, 0.7)];
        let floor = floor_of(&rounds, &record).unwrap();
        // composites 0.55 and 0.6.
        assert_eq!(floor.chrf, 10.0);
        assert!((floor.meaning - 0.2).abs() < 1e-12);
        assert!(
            (floor.composite - 0.05).abs() < 1e-12,
            "{}",
            floor.composite
        );

        // The order of the replicates does not matter, and a third round, or a
        // round under another inventory or from a cycle, is not a replicate.
        let mut other = replicate(2, "d2", 0.0, 0.0);
        let mut cycle = replicate(2, "d1", 0.0, 0.0);
        cycle.cycle = Some(1);
        other.n = 2;
        let rounds = [
            replicate(3, "d1", 0.0, 0.0),
            other,
            cycle,
            replicate(2, "d1", 50.0, 0.7),
            replicate(1, "d1", 60.0, 0.5),
        ];
        assert_eq!(floor_of(&rounds, &record).unwrap(), floor);
        assert_eq!(
            floor_of(&rounds[..3], &record).err(),
            Some(RunError::NoFloor)
        );
    }

    // ---- accepting is one write, held by behaviour ----

    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use cultcache_rs::{CultCacheEnvelope, PushAllOptions};

    /// The single-file store, but the write numbered `fail_at` (counting from 1;
    /// `push`, `push_all` and `delete` are each one write) is refused.
    struct FailsAt {
        inner: SingleFileMessagePackBackingStore,
        armed: Arc<AtomicUsize>,
        fail_at: usize,
    }

    impl FailsAt {
        fn write(&self) -> anyhow::Result<()> {
            let n = self.armed.fetch_add(1, Ordering::SeqCst) + 1;
            if n == self.fail_at {
                anyhow::bail!("write {n} refused");
            }
            Ok(())
        }
    }

    impl CacheBackingStore for FailsAt {
        fn pull_all(&self) -> anyhow::Result<Vec<CultCacheEnvelope>> {
            self.inner.pull_all()
        }
        fn push(&mut self, entry: &CultCacheEnvelope) -> anyhow::Result<()> {
            self.write()?;
            self.inner.push(entry)
        }
        fn delete(&mut self, entry: &CultCacheEnvelope) -> anyhow::Result<()> {
            self.write()?;
            self.inner.delete(entry)
        }
        fn push_all(
            &mut self,
            entries: &[CultCacheEnvelope],
            options: PushAllOptions,
        ) -> anyhow::Result<()> {
            self.write()?;
            self.inner.push_all(entries, options)
        }
    }

    /// Answers the Dao encode prompts from `answers` by item, the candidate
    /// inventory's (rev 4) from the good drafts, and the editor with one concept.
    struct Fake {
        degraded: HashMap<String, String>,
        good: HashMap<String, String>,
    }

    impl ModelPort for Fake {
        fn model(&self) -> &str {
            "fake-model"
        }
        fn answer(&mut self, prompt: &Prompt) -> Result<String, PortError> {
            if prompt.text().starts_with(crate::context::MINT_INSTRUCTIONS) {
                return Ok(
                    r#"{"add":[{"family":"Concept","gloss":"a general meaning the Dao names"}]}"#
                        .to_string(),
                );
            }
            if prompt.text().starts_with(crate::context::LOWER_INSTRUCTIONS) {
                // The table's English for the Dao items, the lowerer's answer.
                let text = match prompt.item_keys()[0].as_str() {
                    "item:daodejing:1" => "The sage empties the minds. The sage fills the bellies. The sage weakens the wills. The sage strengthens the bones.",
                    "item:daodejing:2" => "The heaven is long-enduring. The earth continues long.",
                    "item:daodejing:3" => "The highest excellence is like the water.",
                    other => panic!("lowering asked about {other}"),
                };
                return Ok(format!(r#"{{"text":"{text}"}}"#));
            }
            let answers = if prompt.text().contains("\n\ninventory rev:4\n") {
                &self.good
            } else {
                &self.degraded
            };
            Ok(answers[&prompt.item_keys()[0]].clone())
        }
    }

    struct Bag;

    impl Embedder for Bag {
        fn model(&self) -> &str {
            "bag"
        }
        fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
            Ok(texts
                .iter()
                .map(|t| {
                    let mut v = vec![0.0; 27];
                    v[26] = 1.0;
                    for c in t.chars().filter(char::is_ascii_alphabetic) {
                        v[(c.to_ascii_lowercase() as u8 - b'a') as usize] += 1.0;
                    }
                    v
                })
                .collect())
        }
    }

    fn fake() -> Fake {
        let drafts = |degrade: bool| -> HashMap<String, String> {
            crate::daodejing::packets()
                .iter()
                .take(3)
                .enumerate()
                .map(|(i, p)| {
                    let mut d = PacketDraft::of(p);
                    if degrade && i == 1 {
                        d.referents[0].concept = "concept:999".parse().unwrap();
                    }
                    (
                        format!("item:daodejing:{}", i + 1),
                        serde_json::to_string(&d).unwrap(),
                    )
                })
                .collect()
        };
        Fake {
            degraded: drafts(true),
            good: drafts(false),
        }
    }

    /// Accepting a cycle writes the candidate round and the decision to `run.cc`
    /// as one write: a store that refuses its first or its second write after the
    /// cycle starts leaves both or neither, never the round alone.
    #[test]
    fn accepting_is_one_write_the_round_and_the_decision_land_together() {
        for fail_at in [1, 2] {
            use std::fs;
            let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/daodejing");
            let dir = tempfile::tempdir().unwrap();
            let copy = |from: &str, to: &str| {
                fs::copy(src.join(from), dir.path().join(to)).unwrap();
                dir.path().join(to)
            };
            let (corpus, inventory, entities) = (
                copy("corpus.cc", "c.cc"),
                copy("inventory.cc", "i.cc"),
                copy("entity.cc", "e.cc"),
            );
            let root = dir.path().join("run");
            let mut run = RunStores::create(
                &root,
                "dao",
                RunPaths {
                    corpus: &corpus,
                    inventory: &inventory,
                    entities: &entities,
                },
                "fake-model",
                "bag",
                crate::run::LowererReach::ToolLessRelay,
                ModelFamily::Unrecorded,
            )
            .unwrap();
            let mut model = fake();
            for _ in 0..2 {
                let out = round(&mut run, &mut model, &Scorer::new(Bag)).unwrap();
                assert!(matches!(out, RoundOutcome::Recorded(_)));
            }
            run.cache = run_cache_over(FailsAt {
                inner: SingleFileMessagePackBackingStore::new(root.join("run.cc")),
                armed: Arc::new(AtomicUsize::new(0)),
                fail_at,
            })
            .unwrap();

            let out = cycle(&mut run, &mut model, &Scorer::new(Bag));

            let reloaded = run_cache(&root).unwrap();
            let rounds = reloaded.get_all::<RoundRecord>().unwrap().len();
            let decisions = reloaded.get_all::<Decision>().unwrap().len();
            assert_eq!(
                (rounds, decisions),
                if fail_at == 1 { (2, 0) } else { (3, 1) },
                "write {fail_at} refused"
            );
            assert_eq!(out.is_ok(), fail_at == 2);
            if fail_at == 2 {
                // The fixture accepts: a drifted literal that made it reject
                // would write one round and one decision all the same.
                match &out {
                    Ok(CycleOutcome::Decided(d)) => assert_eq!(d.verdict(), Verdict::Accepted),
                    _ => panic!("the second write was not a decided cycle"),
                }
            }
        }
    }
}

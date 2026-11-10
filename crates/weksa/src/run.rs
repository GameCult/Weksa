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

use std::fmt;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use cultcache_rs::{CultCache, DatabaseEntry};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::context::{EditFeedback, EncoderOffer, Prompt, TrainItems};
use crate::corpus::{CorpusItem, CorpusItemId};
use crate::encode::{EditDraft, PacketDraft, encoder_id};
use crate::form::{is_attestation, is_slug};
use crate::lower::lower;
use crate::model::{ModelPort, PortError};
use crate::score::{CHRF_VERSION, Embedder, ItemScore, ScoreInput, Scorer, SplitMean, aggregate};
use crate::split::{Split, SplitScheme, fnv1a64, hex16};
use crate::store::{CorpusStore, EntityStore, InventoryStore, PacketStore, open_cache};

/// The digest of the prompt builder's source. Any edit to `context.rs`,
/// comments included, ends a run; code outside that file is not covered.
pub const PROMPT_SOURCE_DIGEST: u64 = fnv1a64(include_bytes!("context.rs"));

/// The digest of the lowerer's source. Any edit to `lower.rs`, comments
/// included, ends a run; code outside that file is not covered.
pub const LOWERER_SOURCE_DIGEST: u64 = fnv1a64(include_bytes!("lower.rs"));

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
    #[cultcache(key = 7)]
    pub lowerer_digest: String,
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
}

/// Where a round refused: the first stage that did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Stage {
    Parse,
    Admit,
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
///         lowerer_digest: String::new(),
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
    #[cultcache(key = 3)]
    lowerer_digest: String,
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
    pub fn lowerer_digest(&self) -> &str {
        &self.lowerer_digest
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

    /// The one line a recorded round prints. `n` is every item of the split,
    /// `scored` those that lowered; a mean counts a refused item as 0.
    pub fn summary_line(&self) -> String {
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
        format!(
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
        )
    }
}

/// What a pinned store or id drifted from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriftOf {
    Corpus,
    Entities,
    Prompts,
    Lowerer,
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
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Exists => f.write_str("the root already holds a run"),
            RunError::NoRun => f.write_str("the root holds no run"),
            RunError::Malformed(what) => write!(f, "malformed {what}"),
            RunError::Drift(of) => write!(f, "drift in {of:?}"),
            RunError::NoInventory => f.write_str("the inventory holds no revision"),
            RunError::Io(kind) => write!(f, "io error {kind:?}"),
            RunError::Store => f.write_str("a store would not read or write"),
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
    open_cache(&root.join("run.cc"), |c| {
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

impl RunStores {
    /// Pins the three stores at `root`, which must hold no run. A store file
    /// that does not exist is `Io(NotFound)`.
    pub fn create(
        root: &Path,
        label: &str,
        paths: RunPaths<'_>,
        model: &str,
        embedder: &str,
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
        let mut cache = run_cache(root)?;
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
            lowerer_digest: hex16(LOWERER_SOURCE_DIGEST),
            model: model.to_string(),
            embedder: embedder.to_string(),
            chrf_version: CHRF_VERSION.to_string(),
            inventory_digest: String::new(),
        };
        // The run works on its own copy; the caller's store is never opened again.
        let head = root.join(INVENTORY_FILE);
        std::fs::copy(&record.inventory, &head).map_err(|e| RunError::Io(e.kind()))?;
        let record = RunRecord {
            corpus_digest: file_digest(&record.corpus).ok_or(RunError::Store)?,
            entity_digest: file_digest(&record.entities).ok_or(RunError::Store)?,
            inventory_digest: file_digest_at(&head).ok_or(RunError::Store)?,
            ..record
        };
        cache
            .put(format!("run:{label}"), &record)
            .map_err(|_| RunError::Store)?;
        Self::with(root, cache, record)
    }

    /// Reopens the run at `root` from its record, refusing when a pinned store
    /// or the prompt or lowerer source no longer matches.
    pub fn open(root: &Path) -> Result<Self, RunError> {
        if !root.join("run.cc").is_file() {
            return Err(RunError::NoRun);
        }
        let cache = run_cache(root)?;
        let record = one_run(&cache)?.ok_or(RunError::NoRun)?;
        if record.prompt_digest != hex16(PROMPT_SOURCE_DIGEST) {
            return Err(RunError::Drift(DriftOf::Prompts));
        }
        if record.lowerer_digest != hex16(LOWERER_SOURCE_DIGEST) {
            return Err(RunError::Drift(DriftOf::Lowerer));
        }
        if file_digest(&record.corpus).as_ref() != Some(&record.corpus_digest) {
            return Err(RunError::Drift(DriftOf::Corpus));
        }
        if file_digest(&record.entities).as_ref() != Some(&record.entity_digest) {
            return Err(RunError::Drift(DriftOf::Entities));
        }
        let (path, digest, _) = head_of(&cache, root, &record)?;
        if file_digest_at(&path).as_ref() != Some(&digest) {
            return Err(RunError::Drift(DriftOf::Inventory));
        }
        Self::with(root, cache, record)
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

/// How one item fared: where it was refused, or the text it lowered to, with
/// the key of the packet that was admitted for it.
enum Fate {
    Refused(Stage, Option<String>),
    Lowered(String, String),
}

/// One round: encodes every live Train and Validation item of the run's pinned
/// corpus through `model`, admits each packet into the round's own
/// `<root>/round-<n>/packets.cc`, lowers, scores, and records the round. The
/// round is all or nothing: while any prompt is unanswered, nothing is written,
/// and a round that fails before its record is put (the embedder down) leaves no
/// `packets.cc` behind, so a rerun with revised answers starts clean. A mean
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
fn check_drift<M: ModelPort, E: Embedder>(
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
    for (key, answer) in &answers {
        let item = run.corpus.get(key).map_err(store)?.ok_or(RunError::Store)?;
        let id = CorpusItemId::parse(key).map_err(|_| RunError::Store)?;
        let fate = match answer.as_deref().map(PacketDraft::parse) {
            None | Some(Err(_)) => Fate::Refused(Stage::Parse, None),
            Some(Ok(draft)) => {
                let packet = draft.into_packet(&id, rev, &encoder);
                let packet_key = packet.id.to_string();
                match packets.admit(packet, inventory_store, &run.corpus, &run.entities) {
                    Err(_) => Fate::Refused(Stage::Admit, None),
                    Ok(view) => match lower(&view, &inventory, &run.entities) {
                        Err(_) => Fate::Refused(Stage::Lower, Some(packet_key)),
                        Ok(text) => Fate::Lowered(text, packet_key),
                    },
                }
            }
        };
        items.push(item);
        fates.push(fate);
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

    let n = run.rounds().map_err(store)?.len() as u32 + 1;
    let record = RoundRecord {
        n,
        inventory_rev: rev,
        prompt_digest: run.record.prompt_digest.clone(),
        lowerer_digest: run.record.lowerer_digest.clone(),
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

/// The accept rule. A metric that falls by more than its tolerance (the floor,
/// at least one step) rejects; otherwise the composite must gain strictly more
/// than [`threshold`]. Means count a refused Validation item as 0, so refusing a
/// weak item cannot raise a reading.
pub fn decide(base: &Reading, candidate: &Reading, floor: &Reading) -> Verdict {
    if candidate.chrf - base.chrf < -floor.chrf.max(100.0 * LADDER_STEP) {
        Verdict::Rejected(Why::Regressed(Metric::Chrf))
    } else if candidate.meaning - base.meaning < -floor.meaning.max(LADDER_STEP) {
        Verdict::Rejected(Why::Regressed(Metric::Meaning))
    } else if candidate.composite - base.composite > threshold(floor) {
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

    /// The one line a closed cycle prints.
    pub fn summary_line(&self) -> String {
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
    let reject = |run: &mut RunStores, why: Why| -> Result<CycleOutcome, RunError> {
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
        discard_inventory(&dir);
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
        verdict,
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
            lowerer_digest: String::new(),
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
}

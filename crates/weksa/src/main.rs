//! The `weksa` command line: the same validator and store admission the library
//! exposes, plus text projections (`dump`) that are for review and are not stores.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{Args, Parser, Subcommand, ValueEnum};
use unicode_normalization::UnicodeNormalization;
use weksa::corpus::{CorpusItem, ModelFamily, NameSpan, Source, Turn};
use weksa::encode::{EditDraft, MarkedLine, OntologyDraft, WorkDraft};
use weksa::entity::{Entity, EntityOrigin, LanguageForms, Pronunciation};
use weksa::inventory::{proof_revision, seed, speaker_scene_revision};
use weksa::packet::{FeatValue, FeatureValue, Grade};
use weksa::packet::proof_packet;
use weksa::model::{ModelPort, RecordedAnswers};
use weksa::run::{
    AblationOutcome, CalibrateOutcome, CycleOutcome, FLOOR_ARMS_MIN, LowererReach, OntologyMintOutcome, RecallOutcome, RelowerOutcome, RoundOutcome, RunPaths, RunStores, cycle, round,
};
use weksa::score::ScoreInput;
use weksa::{
    Annotator, CorpusStore, EntityId, EntityStore, Family, InventoryStore, OllamaEmbedder, OntologyStore, OptionLabel,
    PacketStore, PairRelation, Party, Scene, Slot, SpeakerStateStore, ValueKind, Scorer, Split, SplitScheme, StoreError, StoreRefusal, pronounce, validate,
};

#[derive(Parser)]
#[command(
    name = "weksa",
    about = "Weksa interlingua packet, inventory and corpus tools"
)]
struct Cli {
    /// The corpus store.
    #[arg(long, global = true, default_value = "data/corpus/corpus.cc")]
    corpus: PathBuf,
    /// The inventory store.
    #[arg(long, global = true, default_value = "data/interlingua/inventory.cc")]
    inventory: PathBuf,
    /// The entity store.
    #[arg(long, global = true, default_value = "data/entity/entity.cc")]
    entities: PathBuf,
    /// The language ontology store.
    #[arg(long, global = true, default_value = "data/ontology/ontology.cc")]
    ontology: PathBuf,
    /// The speaker-state store (scenes).
    #[arg(long, global = true, default_value = "data/corpus/speaker-state.cc")]
    speaker_state: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate the packets in a store (or one packet) against the inventory.
    Validate { store: PathBuf, key: Option<String> },
    #[command(subcommand)]
    Corpus(CorpusCommand),
    #[command(subcommand)]
    Inventory(InventoryCommand),
    #[command(subcommand)]
    Entity(EntityCommand),
    #[command(subcommand)]
    Ontology(OntologyCommand),
    #[command(subcommand)]
    Scene(SceneCommand),
    #[command(subcommand)]
    Packet(PacketCommand),
    /// Print the ruled IPA of each entity a stored packet cites, one per line;
    /// refused for an entity with no ruled pronunciation.
    Pronounce {
        store: PathBuf,
        key: String,
        /// Print the phones line of each entity instead, one JSON object per
        /// line ({language, source, phones: [{ipa, stress}]}); refused for a
        /// phone the inventory lacks.
        #[arg(long)]
        phones: bool,
    },
    /// Score output text (stdin, or --output) against corpus items, one embed call.
    Score(ScoreArgs),
    /// Print the split of a corpus item.
    Split { item_key: String },
    #[command(subcommand)]
    Loop(LoopCommand),
    /// Write the published JSON Schemas.
    Schema {
        #[arg(long, default_value = "schemas/cultnet")]
        out: PathBuf,
    },
}

#[derive(Subcommand)]
enum LoopCommand {
    /// Pin the global --corpus, --inventory, --entities, --ontology and --speaker-state as a run at ROOT.
    Open {
        root: PathBuf,
        #[arg(long)]
        label: String,
        /// The id of the model that answers prompts.
        #[arg(long)]
        answerer: String,
        #[arg(long, default_value = "qwen3-embedding:0.6b")]
        embed_model: String,
        /// How lowering prompts will be answered: briefed-subagent (smoke
        /// rounds only) or tool-less-relay. Pinned in the run's record.
        #[arg(long)]
        lowerer_reach: LowererReach,
        /// The family of the answering model; rounds count the corpus items
        /// that family wrote. Defaults to unrecorded.
        #[arg(long, default_value = "unrecorded")]
        answerer_family: ModelFamilyArg,
        /// The languages the run offers, reads and decides on: BCP 47 tags,
        /// comma-separated. Every one needs a live Train and Validation item.
        #[arg(long, value_delimiter = ',', required = true)]
        panel: Vec<String>,
    },
    /// Run one round of the run at ROOT. The stores are read only from the
    /// run's record: the global --corpus, --inventory, --entities and --ontology
    /// are not read. Encode prompts are written first; once they are answered a rerun
    /// writes the lowering prompts. The command exits 3 each time until every
    /// prompt has an answer.
    Round {
        root: PathBuf,
        #[arg(long, default_value = "http://127.0.0.1:11434")]
        ollama_url: String,
    },
    /// Probe how closely a fresh model recites the run's items from their
    /// openings (chrF of its completion against the item's second
    /// half). Prompts and answers live in `<ROOT>/recall`; the command exits 3
    /// until every prompt has an answer. A diagnostic, never a gate.
    Recall { root: PathBuf },
    /// Draft ontology entries from the run's Train items of one language.
    /// Prompts and answers live in `<ROOT>/ontology-mint`; the command exits 3
    /// until the prompt has an answer. Admission is `weksa ontology admit`
    /// into a copy, never into the run's pinned file.
    MintOntology {
        root: PathBuf,
        #[arg(long)]
        language: String,
    },
    /// Ask whether the lowerer uses a packet's stance: for each live Train and
    /// Validation packet of round ROUND, ask again with the stance as it is,
    /// emptied, and swapped for the next item's, and read how far the lowering
    /// moves from the round's own answer. Prompts and answers live in
    /// `<ROOT>/ablation-<ROUND>`; the command exits 3 until every prompt has an
    /// answer. A diagnostic, never a gate; Test is never touched.
    Ablate {
        root: PathBuf,
        #[arg(long)]
        round: u32,
        #[arg(long, default_value = "http://127.0.0.1:11434")]
        ollama_url: String,
    },
    /// Re-lower round ROUND's Validation packets through arm ARM, score them
    /// against the items, and, with --floor ARM given two or more times (prompted
    /// repeats of the lowering), decide against the paired noise floor of those
    /// repeats by the panel accept rule. Prompts and answers live in
    /// `<ROOT>/relower-<ROUND>-<ARM>`; the command exits 3 until the arm is
    /// answered. --lowerer names the arm's model in the printed line. A
    /// diagnostic, never a gate; records nothing; Test and Train are never
    /// touched. Labels match [a-z0-9-]{1,32} and differ.
    Relower {
        root: PathBuf,
        #[arg(long)]
        round: u32,
        #[arg(long)]
        arm: String,
        #[arg(long)]
        lowerer: String,
        #[arg(long)]
        floor: Vec<String>,
        #[arg(long, default_value = "http://127.0.0.1:11434")]
        ollama_url: String,
    },
    /// Replay the accept rule over answered relower draws of round ROUND, to
    /// measure how often it accepts, rejects or regresses what it should. Reads
    /// the recorded lowering and each draw (--prompted: repeats of it, at least
    /// four; --degraded: arms that should regress; --candidate: arms to rate),
    /// answered through `weksa loop relower ROOT --round N --arm LABEL`, and
    /// prints counts over all Validation items and over each --class
    /// NAME=PREFIX[,PREFIX...] (items whose key starts with a prefix). Writes
    /// no prompts; exits 1 naming the draws still unanswered. Procedure:
    /// docs/calibration.md. A diagnostic, never a gate; records nothing; Test
    /// and Train are never touched. Labels match [a-z0-9-]{1,32} and differ.
    Calibrate {
        root: PathBuf,
        #[arg(long)]
        round: u32,
        #[arg(long)]
        prompted: Vec<String>,
        #[arg(long)]
        degraded: Vec<String>,
        #[arg(long)]
        candidate: Vec<String>,
        #[arg(long)]
        class: Vec<String>,
        #[arg(long, default_value = "http://127.0.0.1:11434")]
        ollama_url: String,
    },
    /// Run one edit-accept cycle of the run at ROOT. The stores are read only
    /// from the run's record. The editor's prompt, then the candidate round's
    /// encode prompts, then its lowering prompts are written in turn; the
    /// command exits 3 each time until every prompt has an answer. The noise
    /// floor needs four recorded rounds under the first inventory (the recorded
    /// round and three prompted repeats).
    Cycle {
        root: PathBuf,
        #[arg(long, default_value = "http://127.0.0.1:11434")]
        ollama_url: String,
    },
}

#[derive(Subcommand)]
enum PacketCommand {
    /// Admit the nibu-proof-001 packet through `PacketStore::admit`.
    Proof { store: PathBuf },
    /// Admit the Dao De Jing slice's packets through `PacketStore::admit`.
    Dao { store: PathBuf },
}

#[derive(Args)]
struct ScoreArgs {
    /// Corpus item keys, the references.
    #[arg(required = true)]
    items: Vec<String>,
    /// The text to score; stdin when absent.
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, default_value = "http://127.0.0.1:11434")]
    ollama_url: String,
    #[arg(long, default_value = "qwen3-embedding:0.6b")]
    model: String,
}

#[derive(Subcommand)]
enum CorpusCommand {
    Add(Box<AddArgs>),
    /// Text projection of the corpus for review; not a store.
    Dump,
    Withdraw {
        key: String,
    },
    /// Import a transcribed public-domain work: mint its cast as entities and
    /// add each line a name-span answer covers (`docs/name-span-annotation.md`).
    Import {
        /// The work file: provenance, cast, lines.
        work: PathBuf,
        /// The annotator's answer: one `{"line": N, "marked": "..."}` per line.
        names: PathBuf,
        #[arg(long, requires = "generator", conflicts_with = "operator")]
        model_family: Option<ModelFamilyArg>,
        /// The attestation string naming the annotating model.
        #[arg(long, requires = "model_family")]
        generator: Option<String>,
        /// Annotated by the operator, not a model.
        #[arg(long)]
        operator: bool,
    },
}

#[derive(Subcommand)]
enum InventoryCommand {
    /// Text projection of the inventory for review; not a store.
    Dump,
    /// Admit revision 1, the standard's enumerated values and the nuance features.
    Seed,
    /// Admit revision 2, the concepts and frames the nibu-proof-001 packet names.
    Proof,
    /// Admit revision 3 of the Dao De Jing test bed stores (after seed and proof).
    Dao,
    /// Admit the speaker-state revision (Scene and Speaker features) as the
    /// inventory's latest plus one.
    SpeakerScene,
    /// Admit a model's inventory draft (a JSON file, {add, retire} as the
    /// loop's editor writes it) as the inventory's latest plus one.
    Admit { answer: PathBuf },
}

/// The most changes `inventory admit` takes in one draft. A hand-admitted draft
/// is reviewed whole in its cut, so the cap is not the loop editor's.
const MAX_ADMIT_CHANGES: usize = 256;

#[derive(Subcommand)]
enum SceneCommand {
    /// Append a scene revision for one corpus group: its one live speaker's
    /// relation to the addressee, as Scene-slot feature values.
    Put {
        #[arg(long)]
        group: String,
        /// `entity:<slug>` of a named addressee, or `unnamed`.
        #[arg(long)]
        to: String,
        /// The inventory revision the glosses resolve at.
        #[arg(long)]
        inventory_rev: u32,
        /// `gloss=value`, repeated: a grade 0-1000, an option label or true/false.
        #[arg(long = "set", value_name = "GLOSS=VALUE")]
        set: Vec<String>,
        #[arg(long, requires = "generator", conflicts_with = "operator")]
        model_family: Option<ModelFamilyArg>,
        /// The attestation string naming the annotating model.
        #[arg(long, requires = "model_family")]
        generator: Option<String>,
        /// Annotated by the operator, not a model.
        #[arg(long)]
        operator: bool,
    },
    /// Text projection of the scenes for review; not a store.
    Dump,
}

#[derive(Subcommand)]
enum EntityCommand {
    /// Admit an entity, or append forms to a stored one.
    Put(Box<PutArgs>),
    /// Text projection of the entity store for review; not a store.
    Dump,
}

#[derive(Subcommand)]
enum OntologyCommand {
    /// Admit a model's ontology draft (a JSON file) for one language as a
    /// batch, numbered from the language's latest plus one and annotated as
    /// the model that drafted it.
    Admit {
        answer: PathBuf,
        #[arg(long)]
        language: String,
        /// The inventory revision the draft's conditions are read at.
        #[arg(long)]
        inventory_rev: u32,
        #[arg(long)]
        model_family: ModelFamilyArg,
        /// The attestation string naming the drafting model.
        #[arg(long)]
        generator: String,
    },
    /// Text projection of the ontology for review; not a store.
    Dump,
}

#[derive(Args)]
struct PutArgs {
    /// `entity:<slug>`.
    id: String,
    /// `language=form`, repeated; a language's forms keep the order given,
    /// citation form first. Forms are NFC-normalized here.
    #[arg(long = "form", required = true)]
    forms: Vec<String>,
    #[arg(long, value_enum)]
    origin: OriginKind,
    /// The persona of a `cast` or `live` origin; refused with any other origin.
    #[arg(long)]
    persona: Option<String>,
    /// The fixture path a `cast` entity came from; refused with any other origin.
    #[arg(long)]
    fixture: Option<String>,
    /// The work of a `public-domain` origin; refused with any other origin.
    #[arg(long)]
    work: Option<String>,
    /// `language=ipa`: how that language's citation form is said, bare IPA with
    /// stress marks. Needs `--ruled-by`, `--ruled-on` and `--ruling-source`.
    #[arg(long, requires_all = ["ruled_by", "ruled_on", "ruling_source"])]
    pronunciation: Option<String>,
    /// Who ruled the pronunciation.
    #[arg(long)]
    ruled_by: Option<String>,
    /// `YYYY-MM-DD`.
    #[arg(long)]
    ruled_on: Option<String>,
    /// Where it was ruled: a room, a document, a run.
    #[arg(long)]
    ruling_source: Option<String>,
    /// A refused pronunciation, bare IPA, repeated.
    #[arg(long = "rejected", requires = "pronunciation")]
    rejected: Vec<String>,
}

#[derive(Clone, Copy, ValueEnum)]
enum OriginKind {
    Cast,
    PublicDomain,
    Live,
}

impl PutArgs {
    fn entity(self) -> Result<Entity> {
        let mut forms: Vec<LanguageForms> = Vec::new();
        for f in &self.forms {
            let (language, form) = f.split_once('=').context("--form is not language=form")?;
            match forms.iter_mut().find(|lf| lf.language == language) {
                Some(lf) => lf.forms.push(form.nfc().collect()),
                None => forms.push(LanguageForms {
                    language: language.to_string(),
                    forms: vec![form.nfc().collect()],
                    pronunciation: None,
                }),
            }
        }
        if let Some(p) = &self.pronunciation {
            let (language, ipa) = p
                .split_once('=')
                .context("--pronunciation is not language=ipa")?;
            let lf = forms
                .iter_mut()
                .find(|lf| lf.language == language)
                .context("--pronunciation names a language that has no --form")?;
            lf.pronunciation = Some(Pronunciation {
                ipa: ipa.nfc().collect(),
                ruled_by: need(self.ruled_by.clone(), "ruled-by")?,
                date: need(self.ruled_on.clone(), "ruled-on")?,
                source: need(self.ruling_source.clone(), "ruling-source")?,
                rejected: self.rejected.iter().map(|r| r.nfc().collect()).collect(),
            });
        }
        let origin = match (self.origin, self.persona, self.fixture, self.work) {
            (OriginKind::Cast, persona, fixture, None) => EntityOrigin::Cast {
                persona: need(persona, "persona")?,
                fixture: need(fixture, "fixture")?,
            },
            (OriginKind::PublicDomain, None, None, work) => EntityOrigin::PublicDomain {
                work: need(work, "work")?,
            },
            (OriginKind::Live, persona, None, None) => EntityOrigin::Live {
                persona: need(persona, "persona")?,
            },
            _ => bail!(
                "--fixture is only for --origin cast, --persona only for --origin cast or live and --work only for --origin public-domain"
            ),
        };
        Ok(Entity {
            id: self.id.parse()?,
            forms,
            origin,
        })
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum SourceKind {
    CastAuthored,
    OperatorWritten,
    OperatorApproved,
    PublicDomain,
    ModelWritten,
}

#[derive(Clone, Copy, ValueEnum)]
enum ModelFamilyArg {
    Anthropic,
    OpenAi,
    Google,
    Alibaba,
    Meta,
    Mistral,
    Unrecorded,
}

impl From<ModelFamilyArg> for ModelFamily {
    fn from(f: ModelFamilyArg) -> Self {
        match f {
            ModelFamilyArg::Anthropic => ModelFamily::Anthropic,
            ModelFamilyArg::OpenAi => ModelFamily::OpenAi,
            ModelFamilyArg::Google => ModelFamily::Google,
            ModelFamilyArg::Alibaba => ModelFamily::Alibaba,
            ModelFamilyArg::Meta => ModelFamily::Meta,
            ModelFamilyArg::Mistral => ModelFamily::Mistral,
            ModelFamilyArg::Unrecorded => ModelFamily::Unrecorded,
        }
    }
}

#[derive(Args)]
struct AddArgs {
    #[arg(long)]
    slug: String,
    /// NFC-normalized here; `--name` byte offsets index the normalized text.
    #[arg(long)]
    text: String,
    /// The item's language, a BCP 47 tag; no language is the default.
    #[arg(long)]
    language: String,
    /// An entity id.
    #[arg(long)]
    speaker: EntityId,
    /// `start..end=entity:<slug>`, repeated: a byte range of the text naming an entity.
    #[arg(long = "name")]
    names: Vec<String>,
    #[arg(long)]
    group: String,
    /// `speaker=text`, repeated, oldest first.
    #[arg(long = "prior-turn")]
    prior_turns: Vec<String>,
    #[arg(long, value_enum)]
    source: SourceKind,
    #[arg(long)]
    persona: Option<String>,
    #[arg(long)]
    fixture: Option<String>,
    #[arg(long)]
    note: Option<String>,
    #[arg(long)]
    work: Option<String>,
    /// Who wrote the line: for `cast-authored` and `public-domain`.
    #[arg(long)]
    author: Option<String>,
    #[arg(long)]
    edition_url: Option<String>,
    /// The edition's licence, for `public-domain`.
    #[arg(long)]
    licence: Option<String>,
    /// Where the public-domain status is asserted, for `public-domain`.
    #[arg(long)]
    jurisdiction: Option<String>,
    #[arg(long)]
    translator: Option<String>,
    #[arg(long)]
    edition_year: Option<u16>,
    #[arg(long = "model-family", value_enum)]
    family: Option<ModelFamilyArg>,
    /// The generator string the fixture or run wrote, verbatim.
    #[arg(long)]
    generator: Option<String>,
    #[arg(long)]
    prompt_digest: Option<String>,
    /// `YYYY-MM-DD`, a real calendar date.
    #[arg(long, value_parser = parse_date)]
    admitted_on: String,
    #[arg(long)]
    supersedes: Option<String>,
}

fn parse_date(s: &str) -> std::result::Result<String, String> {
    if weksa::form::is_calendar_date(s) {
        Ok(s.to_string())
    } else {
        Err("not a YYYY-MM-DD calendar date".into())
    }
}

/// Parses the command line. A rejected value is reported by the flag that took
/// it and the reason, never by the value: values are the caller's identifiers.
/// The same holds for the unknown flags and subcommands clap would quote.
fn parse_cli() -> Cli {
    use std::error::Error;
    match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e)
            if matches!(
                e.kind(),
                ErrorKind::InvalidValue | ErrorKind::ValueValidation
            ) =>
        {
            let arg = match e.get(ContextKind::InvalidArg) {
                Some(ContextValue::String(a)) => {
                    a.split_whitespace().next().unwrap_or("").to_string()
                }
                _ => String::new(),
            };
            let reason = e.source().map(|s| format!(": {s}")).unwrap_or_default();
            eprintln!("error: invalid value for {arg}{reason}");
            std::process::exit(2);
        }
        Err(e) => match e.kind() {
            ErrorKind::UnknownArgument => refuse_cli("unknown argument"),
            ErrorKind::InvalidSubcommand => refuse_cli("unknown subcommand"),
            _ => e.exit(),
        },
    }
}

/// A clap error that would quote the caller's token, as a fixed message that
/// names the kind only. Usage is `--help`'s to print.
fn refuse_cli(what: &str) -> ! {
    eprintln!("error: {what}\n\nFor more information, try '--help'.");
    std::process::exit(2);
}

fn need(value: Option<String>, flag: &str) -> Result<String> {
    value.with_context(|| format!("this source needs --{flag}"))
}

impl AddArgs {
    fn source(&self) -> Result<Source> {
        let get = |v: &Option<String>, flag: &str| need(v.clone(), flag);
        let applies: &[&str] = match self.source {
            SourceKind::CastAuthored => &["persona", "fixture", "author"],
            SourceKind::OperatorWritten => &[],
            SourceKind::OperatorApproved => &["note"],
            SourceKind::PublicDomain => &[
                "work",
                "author",
                "edition-url",
                "licence",
                "jurisdiction",
                "translator",
                "edition-year",
            ],
            SourceKind::ModelWritten => &["model-family", "generator", "prompt-digest"],
        };
        let given = [
            ("persona", self.persona.is_some()),
            ("fixture", self.fixture.is_some()),
            ("author", self.author.is_some()),
            ("note", self.note.is_some()),
            ("work", self.work.is_some()),
            ("edition-url", self.edition_url.is_some()),
            ("licence", self.licence.is_some()),
            ("jurisdiction", self.jurisdiction.is_some()),
            ("translator", self.translator.is_some()),
            ("edition-year", self.edition_year.is_some()),
            ("model-family", self.family.is_some()),
            ("generator", self.generator.is_some()),
            ("prompt-digest", self.prompt_digest.is_some()),
        ];
        let stray: Vec<String> = given
            .into_iter()
            .filter(|(flag, given)| *given && !applies.contains(flag))
            .map(|(flag, _)| format!("--{flag}"))
            .collect();
        if !stray.is_empty() {
            bail!("{} does not apply to this --source", stray.join(", "));
        }
        Ok(match self.source {
            SourceKind::CastAuthored => Source::CastAuthored {
                persona: get(&self.persona, "persona")?,
                fixture: get(&self.fixture, "fixture")?,
                author: get(&self.author, "author")?,
            },
            SourceKind::OperatorWritten => Source::OperatorWritten,
            SourceKind::OperatorApproved => Source::OperatorApproved {
                note: get(&self.note, "note")?,
            },
            SourceKind::PublicDomain => Source::PublicDomain {
                work: get(&self.work, "work")?,
                author: get(&self.author, "author")?,
                edition_url: get(&self.edition_url, "edition-url")?,
                licence: get(&self.licence, "licence")?,
                jurisdiction: get(&self.jurisdiction, "jurisdiction")?,
                translator: get(&self.translator, "translator")?,
                edition_year: self
                    .edition_year
                    .context("this source needs --edition-year")?,
            },
            SourceKind::ModelWritten => Source::ModelWritten {
                family: self
                    .family
                    .map(ModelFamily::from)
                    .context("this source needs --model-family")?,
                generator: self.generator.clone(),
                prompt_digest: self.prompt_digest.clone(),
            },
        })
    }

    fn item(self, id: String) -> Result<CorpusItem> {
        let prior_turns = self
            .prior_turns
            .iter()
            .map(|t| {
                let (speaker, text) = t
                    .split_once('=')
                    .context("--prior-turn is not speaker=text")?;
                Ok(Turn {
                    speaker: speaker.parse().map_err(|_| {
                        anyhow::anyhow!("--prior-turn speaker is not entity:<slug>")
                    })?,
                    text: text.nfc().collect(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let names = self
            .names
            .iter()
            .map(|n| {
                let bad = || anyhow::anyhow!("--name is not start..end=entity:<slug>");
                let (range, entity) = n.split_once('=').ok_or_else(bad)?;
                let (start, end) = range.split_once("..").ok_or_else(bad)?;
                Ok(NameSpan {
                    entity: entity.parse().map_err(|_| bad())?,
                    start: start.parse().map_err(|_| bad())?,
                    end: end.parse().map_err(|_| bad())?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(CorpusItem {
            id,
            source: self.source()?,
            text: self.text.nfc().collect(),
            language: self.language,
            speaker: self.speaker,
            group: self.group,
            prior_turns,
            admitted_on: self.admitted_on,
            supersedes: self.supersedes,
            withdrawn: false,
            names,
            names_by: None,
        })
    }
}

/// A draw or arm label: [a-z0-9-]{1,32}.
fn is_label(l: &str) -> bool {
    (1..=32).contains(&l.len())
        && l.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The split of an item as `Train`, `Validation`, `Test` or `excluded`.
fn split_text(item: &CorpusItem) -> String {
    SplitScheme::V1
        .split(item)
        .map_or_else(|| "excluded".to_string(), |s| format!("{s:?}"))
}

/// The annotator the flags name: a model family with its generator, or the operator.
fn annotator(
    model_family: Option<ModelFamilyArg>,
    generator: Option<String>,
    operator: bool,
) -> Result<Annotator> {
    match (model_family, generator, operator) {
        (Some(family), Some(generator), false) => Ok(Annotator::Model {
            family: family.into(),
            generator,
        }),
        (None, None, true) => Ok(Annotator::Operator),
        _ => bail!("name the annotator: --model-family with --generator, or --operator"),
    }
}

fn refused(e: StoreError) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

fn main() -> Result<()> {
    let cli = parse_cli();
    match cli.command {
        Command::Validate { store, key } => {
            let inventory = InventoryStore::open(&cli.inventory)?.inventory()?;
            let packets = PacketStore::open(&store)?;
            let packets = match key {
                Some(key) => vec![packets.get(&key)?.context("no such packet")?],
                None => packets.all()?,
            };
            let mut bad = 0;
            for p in &packets {
                match validate(p, &inventory) {
                    Ok(_) => println!("ok {}", p.id),
                    Err(refusals) => {
                        bad += 1;
                        for r in refusals {
                            println!("refused {} {r}", p.id);
                        }
                    }
                }
            }
            if bad > 0 {
                bail!("{bad} of {} packets refused", packets.len());
            }
        }
        Command::Corpus(CorpusCommand::Add(args)) => {
            let mut store = CorpusStore::open(&cli.corpus)?;
            let key = store.next_key(&args.slug).map_err(refused)?;
            let entities = EntityStore::open(&cli.entities)?;
            let outcome = store
                .add((*args).item(key.clone())?, &entities)
                .map_err(refused)?;
            println!("{outcome:?} {key}");
        }
        Command::Corpus(CorpusCommand::Import {
            work,
            names,
            model_family,
            generator,
            operator,
        }) => {
            let annotator = annotator(model_family, generator, operator)?;
            let draft = WorkDraft::parse(&std::fs::read_to_string(&work)?)?;
            let mut marked = Vec::new();
            for (i, line) in std::fs::read_to_string(&names)?.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                marked.push(
                    MarkedLine::parse(line)
                        .map_err(|e| anyhow::anyhow!("names line {}: {e}", i + 1))?,
                );
            }
            let (new_entities, items) = draft.into_import(&marked, &annotator)?;
            // Every document passes the real doors on scratch copies of the
            // stores before the first write, so a refusal leaves them untouched.
            let scratch = std::env::temp_dir().join(format!("weksa-import-{}", std::process::id()));
            std::fs::create_dir_all(&scratch)?;
            let dry = (|| -> Result<()> {
                let copy = |from: &Path, name: &str| -> Result<PathBuf> {
                    let to = scratch.join(name);
                    if from.exists() {
                        std::fs::copy(from, &to)?;
                    }
                    Ok(to)
                };
                let mut entities = EntityStore::open(&copy(&cli.entities, "e.cc")?)?;
                let mut store = CorpusStore::open(&copy(&cli.corpus, "c.cc")?)?;
                for e in new_entities.clone() {
                    entities
                        .put(e)
                        .map_err(|e| anyhow::anyhow!("a cast entity: {e}"))?;
                }
                for item in items.clone() {
                    let key = item.id.clone();
                    store.add(item, &entities).map_err(|e| match e {
                        StoreError::Refused(StoreRefusal::S6TextHeld { held_by, .. }) => {
                            anyhow::anyhow!("{key} repeats the text of {held_by}")
                        }
                        e => anyhow::anyhow!("{key}: {e}"),
                    })?;
                }
                Ok(())
            })();
            let _ = std::fs::remove_dir_all(&scratch);
            dry?;
            let mut entities = EntityStore::open(&cli.entities)?;
            for e in new_entities {
                let id = e.id.clone();
                let outcome = entities.put(e).map_err(refused)?;
                println!("{outcome:?} {id}");
            }
            let mut store = CorpusStore::open(&cli.corpus)?;
            for item in items {
                let (key, split) = (item.id.clone(), split_text(&item));
                let outcome = store.add(item, &entities).map_err(refused)?;
                println!("{outcome:?} {key} {split}");
            }
        }
        Command::Corpus(CorpusCommand::Dump) => {
            for i in CorpusStore::open(&cli.corpus)?.all()? {
                let split = split_text(&i);
                let withdrawn = if i.withdrawn { " withdrawn" } else { "" };
                println!(
                    "{} [{} {} group={} {split}{withdrawn}]\n  {:?}\n  names: {:?} by {:?}\n  source: {:?}",
                    i.id, i.language, i.speaker, i.group, i.text, i.names, i.names_by, i.source
                );
            }
        }
        Command::Corpus(CorpusCommand::Withdraw { key }) => {
            CorpusStore::open(&cli.corpus)?
                .withdraw(&key)
                .map_err(refused)?;
            println!("withdrawn {key}");
        }
        Command::Inventory(InventoryCommand::Dump) => {
            let inv = InventoryStore::open(&cli.inventory)?.inventory()?;
            for r in inv.revisions() {
                println!(
                    "rev:{} parent={:?} frozen={} changes={}",
                    r.n,
                    r.parent,
                    r.frozen,
                    r.changes.len()
                );
            }
            for e in inv.entries() {
                println!(
                    "{} introduced={} retired={:?} {:?} {:?} {}",
                    e.id, e.introduced_rev, e.retired_rev, e.value_kind, e.applies_to, e.gloss
                );
            }
        }
        Command::Entity(EntityCommand::Put(args)) => {
            let entity = args.entity()?;
            let id = entity.id.clone();
            let outcome = EntityStore::open(&cli.entities)?
                .put(entity)
                .map_err(refused)?;
            println!("{outcome:?} {id}");
        }
        Command::Entity(EntityCommand::Dump) => {
            for e in EntityStore::open(&cli.entities)?.all()? {
                println!("{} {:?}\n  forms: {:?}", e.id, e.origin, e.forms);
            }
        }
        Command::Ontology(OntologyCommand::Admit {
            answer,
            language,
            inventory_rev,
            model_family,
            generator,
        }) => {
            let text = std::fs::read_to_string(&answer).context("the draft would not read")?;
            let draft = OntologyDraft::parse(&text)?;
            let mut store = OntologyStore::open(&cli.ontology)?;
            let first = store
                .all()?
                .iter()
                .filter(|e| e.language == language)
                .map(|e| e.n)
                .max()
                .unwrap_or(0)
                + 1;
            let annotator = Annotator::Model {
                family: model_family.into(),
                generator,
            };
            let entries = draft.into_entries(&language, inventory_rev, first, &annotator);
            let count = entries.len();
            let inventory = InventoryStore::open(&cli.inventory)?;
            let outcome = store.admit(entries, &inventory).map_err(refused)?;
            println!("{outcome:?} {count} entries of {language}");
        }
        Command::Ontology(OntologyCommand::Dump) => {
            for e in OntologyStore::open(&cli.ontology)?.all()? {
                println!(
                    "{} {} rev:{} {:?}\n  when: {:?}\n  {}\n  e.g. {:?}",
                    e.key(),
                    e.layer.name(),
                    e.inventory_rev,
                    e.annotator,
                    e.when,
                    e.choice,
                    e.examples
                );
            }
        }
        Command::Inventory(InventoryCommand::Seed) => {
            let (rev, entries) = seed();
            let count = entries.len();
            InventoryStore::open(&cli.inventory)?
                .admit_revision(rev, entries)
                .map_err(refused)?;
            println!("seeded rev:1 with {count} entries");
        }
        Command::Inventory(InventoryCommand::Proof) => {
            let (rev, entries) = proof_revision();
            let count = entries.len();
            InventoryStore::open(&cli.inventory)?
                .admit_revision(rev, entries)
                .map_err(refused)?;
            println!("admitted rev:2 with {count} entries");
        }
        Command::Inventory(InventoryCommand::SpeakerScene) => {
            let store = InventoryStore::open(&cli.inventory)?;
            let latest = store
                .inventory()?
                .latest()
                .map(|r| r.n)
                .context("the inventory holds no revision")?;
            let (rev, entries) = speaker_scene_revision(latest + 1);
            let (n, count) = (rev.n, entries.len());
            let mut store = store;
            store.admit_revision(rev, entries).map_err(refused)?;
            println!("admitted rev:{n} with {count} entries");
        }
        Command::Inventory(InventoryCommand::Admit { answer }) => {
            let text = std::fs::read_to_string(&answer).context("the draft would not read")?;
            let draft = EditDraft::parse(&text)?;
            let mut store = InventoryStore::open(&cli.inventory)?;
            let inventory = store.inventory()?;
            let latest = inventory
                .latest()
                .map(|r| r.n)
                .context("the inventory holds no revision")?;
            let (rev, entries) = draft
                .into_revision(&inventory, latest, MAX_ADMIT_CHANGES)
                .context("the draft makes no change or more than the cap")?;
            let (n, count) = (rev.n, entries.len());
            store.admit_revision(rev, entries).map_err(refused)?;
            println!("admitted rev:{n} with {count} entries");
        }
        Command::Scene(SceneCommand::Put {
            group,
            to,
            inventory_rev,
            set,
            model_family,
            generator,
            operator,
        }) => {
            let annotator = annotator(model_family, generator, operator)?;
            let inventory_store = InventoryStore::open(&cli.inventory)?;
            let inventory = inventory_store.inventory()?;
            let corpus = CorpusStore::open(&cli.corpus)?;
            let live: Vec<CorpusItem> = corpus
                .all()?
                .into_iter()
                .filter(|i| i.group == group && !i.withdrawn)
                .collect();
            let mut speakers: Vec<EntityId> = live.iter().map(|i| i.speaker.clone()).collect();
            speakers.sort();
            speakers.dedup();
            let [from] = speakers.as_slice() else {
                bail!("a scene group holds one live speaker; multi-party scenes wait");
            };
            let to = match to.as_str() {
                "unnamed" => Party::Unnamed,
                named => Party::Named(
                    EntityId::parse(named).map_err(|_| anyhow::anyhow!("the addressee is unnamed or an entity id"))?,
                ),
            };
            let mut features = Vec::new();
            for pair in &set {
                let bad = || anyhow::anyhow!("a --set is gloss=value of a live Scene feature");
                let (gloss, value) = pair.split_once('=').ok_or_else(bad)?;
                let entry = inventory
                    .entries()
                    .find(|e| {
                        e.family() == Family::Feature
                            && e.gloss == gloss
                            && e.live_at(inventory_rev)
                            && e.applies_to.contains(&Slot::Scene)
                    })
                    .ok_or_else(bad)?;
                let value = match entry.value_kind.as_ref().ok_or_else(bad)? {
                    ValueKind::Grade => FeatValue::Grade(
                        Grade::new(value.parse().map_err(|_| bad())?).map_err(|_| bad())?,
                    ),
                    ValueKind::Options(_) => {
                        FeatValue::Option(OptionLabel::parse(value).map_err(|_| bad())?)
                    }
                    ValueKind::Flag => FeatValue::Flag(value.parse().map_err(|_| bad())?),
                };
                features.push(FeatureValue { feature: entry.id, value });
            }
            let mut participants = vec![from.clone()];
            if let Party::Named(a) = &to
                && a != from
            {
                participants.push(a.clone());
            }
            let mut state = SpeakerStateStore::open(&cli.speaker_state)?;
            let latest = state.scene(&group)?;
            let mut scene = Scene {
                group,
                n: latest.as_ref().map_or(1, |l| l.n + 1),
                inventory_rev,
                participants,
                relations: vec![PairRelation { from: from.clone(), to, features }],
                annotator,
            };
            if let Some(l) = &latest {
                let same = Scene { n: l.n, ..scene.clone() };
                if &same == l {
                    println!("unchanged");
                    return Ok(());
                }
            }
            scene.n = latest.map_or(1, |l| l.n + 1);
            let key = weksa::speaker::scene_key(&scene.group, scene.n);
            let outcome = state
                .put_scene(scene, &inventory_store, &corpus, &EntityStore::open(&cli.entities)?)
                .map_err(refused)?;
            println!("{outcome:?} {key}");
        }
        Command::Scene(SceneCommand::Dump) => {
            let inventory = InventoryStore::open(&cli.inventory)?.inventory()?;
            let gloss = |f: &FeatureValue| {
                let name = inventory.entry(&f.feature).map_or("?", |e| e.gloss.as_str());
                match &f.value {
                    FeatValue::Grade(g) => format!("{name}={}", g.get()),
                    FeatValue::Option(l) => format!("{name}={l}"),
                    FeatValue::Flag(b) => format!("{name}={b}"),
                }
            };
            for s in SpeakerStateStore::open(&cli.speaker_state)?.all_scenes()? {
                let who = match &s.annotator {
                    Annotator::Operator => "Operator".to_string(),
                    Annotator::Model { family, generator } => format!("Model {family:?} {generator}"),
                };
                let people: Vec<&str> = s.participants.iter().map(|p| p.as_str()).collect();
                println!(
                    "scene:{}@{} rev:{} {who} [{}]",
                    s.group,
                    s.n,
                    s.inventory_rev,
                    people.join(",")
                );
                for r in &s.relations {
                    let to = match &r.to {
                        Party::Named(e) => e.as_str().to_string(),
                        Party::Unnamed => "unnamed".to_string(),
                    };
                    let feats: Vec<String> = r.features.iter().map(&gloss).collect();
                    println!("  {}->{to} {}", r.from, feats.join(" "));
                }
            }
        }
        Command::Inventory(InventoryCommand::Dao) => {
            let (rev, entries) = weksa::daodejing::revision();
            let count = entries.len();
            InventoryStore::open(&cli.inventory)?
                .admit_revision(rev, entries)
                .map_err(refused)?;
            println!("admitted rev:3 with {count} entries");
        }
        Command::Packet(PacketCommand::Dao { store }) => {
            let mut packets = PacketStore::open(&store)?;
            let inventory = InventoryStore::open(&cli.inventory)?;
            let corpus = CorpusStore::open(&cli.corpus)?;
            let entities = EntityStore::open(&cli.entities)?;
            for packet in weksa::daodejing::packets() {
                let key = packet.id.to_string();
                packets
                    .admit(packet, &inventory, &corpus, &entities)
                    .map_err(refused)?;
                println!("admitted {key}");
            }
        }
        Command::Packet(PacketCommand::Proof { store }) => {
            let packet = proof_packet();
            let key = packet.id.to_string();
            PacketStore::open(&store)?
                .admit(
                    packet,
                    &InventoryStore::open(&cli.inventory)?,
                    &CorpusStore::open(&cli.corpus)?,
                    &EntityStore::open(&cli.entities)?,
                )
                .map_err(refused)?;
            println!("admitted {key}");
        }
        Command::Pronounce { store, key, phones } => {
            let inventory_store = InventoryStore::open(&cli.inventory)?;
            let mut packets = PacketStore::open(&store)?;
            let packet = packets.get(&key)?.context("no such packet")?;
            // Admission is the only source of a LoweringInput; re-admitting the
            // stored packet is a no-op write that re-runs every door check.
            let view = packets
                .admit(
                    packet,
                    &inventory_store,
                    &CorpusStore::open(&cli.corpus)?,
                    &EntityStore::open(&cli.entities)?,
                )
                .map_err(refused)?;
            let entities = EntityStore::open(&cli.entities)?;
            if phones {
                for line in weksa::phones(&view, &entities)? {
                    println!("{}", serde_json::to_string(&line)?);
                }
            } else {
                for line in pronounce(&view, &entities)? {
                    println!("{line}");
                }
            }
        }
        Command::Score(args) => {
            let output = match &args.output {
                Some(path) => std::fs::read_to_string(path)?,
                None => std::io::read_to_string(std::io::stdin())?,
            };
            let corpus = CorpusStore::open(&cli.corpus)?;
            let references = args
                .items
                .iter()
                .map(|key| Ok(corpus.get(key)?.context("no such item")?.text))
                .collect::<Result<Vec<_>>>()?;
            let inputs: Vec<ScoreInput> = references
                .iter()
                .map(|reference| ScoreInput {
                    reference,
                    output: output.trim(),
                })
                .collect();
            let scorer = Scorer::new(OllamaEmbedder::new(&args.ollama_url, &args.model));
            for (key, score) in args.items.iter().zip(scorer.score(&inputs)?) {
                match score.meaning {
                    Some(m) => println!("{key} chrf={:.2} meaning={m:.4}", score.chrf),
                    None => println!("{key} chrf={:.2} meaning=none", score.chrf),
                }
            }
        }
        Command::Split { item_key } => {
            let item = CorpusStore::open(&cli.corpus)?
                .get(&item_key)?
                .context("no such item")?;
            println!("{}", split_text(&item));
        }
        Command::Loop(LoopCommand::Open {
            root,
            label,
            answerer,
            embed_model,
            lowerer_reach,
            answerer_family,
            panel,
        }) => {
            let run = RunStores::create(
                &root,
                &label,
                RunPaths {
                    corpus: &cli.corpus,
                    inventory: &cli.inventory,
                    entities: &cli.entities,
                    ontology: &cli.ontology,
                    speaker_state: &cli.speaker_state,
                },
                &answerer,
                &embed_model,
                lowerer_reach,
                answerer_family.into(),
                &panel,
            )?;
            println!("opened run:{}", run.record().label);
        }
        Command::Loop(LoopCommand::Round { root, ollama_url }) => {
            let unreadable = |_| anyhow::anyhow!("a store would not read");
            let mut run = RunStores::open(&root)?;
            let mut files = run.answer_files(run.next_round_dir().map_err(unreadable)?)?;
            let scorer = Scorer::new(OllamaEmbedder::new(&ollama_url, &run.record().embedder));
            match round(&mut run, &mut files, &scorer)? {
                RoundOutcome::Unanswered { prompts } => {
                    for digest in &prompts {
                        println!("unanswered {digest}");
                    }
                    let asked = run.offers().map_err(unreadable)?.len();
                    println!("unanswered {} of {asked}", prompts.len());
                    std::process::exit(3);
                }
                RoundOutcome::Recorded(r) => println!("{}", run.round_line(&r)),
            }
        }
        Command::Loop(LoopCommand::Ablate {
            root,
            round,
            ollama_url,
        }) => {
            let run = RunStores::open(&root)?;
            let mut recorded = RecordedAnswers::new(
                root.join(format!("round-{round}")),
                run.record().model.clone(),
            );
            let mut files = run.answer_files(run.ablation_dir(round))?;
            let scorer = Scorer::new(OllamaEmbedder::new(&ollama_url, &run.record().embedder));
            match run.stance_ablation(round, &mut recorded, &mut files, &scorer)? {
                AblationOutcome::Unanswered { prompts } => {
                    for digest in &prompts {
                        println!("unanswered {digest}");
                    }
                    println!("unanswered {} prompts", prompts.len());
                    std::process::exit(3);
                }
                AblationOutcome::Read(readings) => {
                    for (language, split, reading) in readings {
                        let name = match split {
                            Split::Train => "train",
                            _ => "validation",
                        };
                        println!("{language} {name} {}", reading.line());
                    }
                }
            }
        }
        Command::Loop(LoopCommand::Relower {
            root,
            round,
            arm,
            lowerer,
            floor,
            ollama_url,
        }) => {
            if !is_label(&arm)
                || floor.iter().any(|f| !is_label(f) || *f == arm)
                || floor.iter().enumerate().any(|(i, f)| floor[..i].contains(f))
            {
                anyhow::bail!("arm and floor labels must match [a-z0-9-]{{1,32}} and differ");
            }
            let run = RunStores::open(&root)?;
            let mut recorded = RecordedAnswers::new(
                root.join(format!("round-{round}")),
                run.record().model.clone(),
            );
            let mut files = run.answer_files(run.relower_dir(round, &arm))?;
            let mut floor_ports: Vec<RecordedAnswers> = floor
                .iter()
                .map(|f| RecordedAnswers::new(run.relower_dir(round, f), run.record().model.clone()))
                .collect();
            let mut floor_dyn: Vec<&mut dyn ModelPort> =
                floor_ports.iter_mut().map(|p| p as &mut dyn ModelPort).collect();
            let scorer = Scorer::new(OllamaEmbedder::new(&ollama_url, &run.record().embedder));
            match run.relower(
                round,
                &arm,
                &mut recorded,
                &mut files,
                &mut floor_dyn,
                &scorer,
            )? {
                RelowerOutcome::Unanswered { prompts } => {
                    for digest in &prompts {
                        println!("unanswered {digest}");
                    }
                    println!("unanswered {} prompts", prompts.len());
                    std::process::exit(3);
                }
                RelowerOutcome::Read { languages, verdict } => {
                    for l in languages {
                        println!(
                            "{}",
                            run.labelled(&format!("{} arm={arm} lowerer={lowerer}", l.line()))
                        );
                    }
                    if let Some(v) = verdict {
                        println!("verdict {v:?}");
                    }
                }
            }
        }
        Command::Loop(LoopCommand::Calibrate {
            root,
            round,
            prompted,
            degraded,
            candidate,
            class,
            ollama_url,
        }) => {
            let labels: Vec<&String> = prompted.iter().chain(&degraded).chain(&candidate).collect();
            if labels.iter().any(|l| !is_label(l))
                || labels.iter().enumerate().any(|(i, l)| labels[..i].contains(l))
            {
                anyhow::bail!("draw labels must match [a-z0-9-]{{1,32}} and differ");
            }
            if prompted.len() < FLOOR_ARMS_MIN + 1 {
                anyhow::bail!("at least {} --prompted labels are needed", FLOOR_ARMS_MIN + 1);
            }
            let mut classes: Vec<(String, Vec<String>)> = Vec::new();
            for c in &class {
                let Some((name, prefixes)) = c.split_once('=') else {
                    anyhow::bail!("--class is NAME=PREFIX[,PREFIX...]");
                };
                if !is_label(name)
                    || name == "all"
                    || classes.iter().any(|(n, _)| n == name)
                    || prefixes.split(',').any(str::is_empty)
                {
                    anyhow::bail!("--class is NAME=PREFIX[,PREFIX...] with a new label NAME other than all");
                }
                classes.push((name.to_string(), prefixes.split(',').map(String::from).collect()));
            }
            let run = RunStores::open(&root)?;
            let scorer = Scorer::new(OllamaEmbedder::new(&ollama_url, &run.record().embedder));
            match run.calibrate(round, &prompted, &degraded, &candidate, &classes, &scorer)? {
                CalibrateOutcome::Unanswered(missing) => {
                    for (label, n) in &missing {
                        println!("unanswered {label} {n}");
                    }
                    eprintln!(
                        "answer it through weksa loop relower ROOT --round N --arm LABEL first"
                    );
                    std::process::exit(1);
                }
                CalibrateOutcome::Read { unusable, classes } => {
                    println!(
                        "{}",
                        run.labelled(&format!(
                            "calibrate draws prompted={} degraded={} candidate={} floor-arms={FLOOR_ARMS_MIN} unusable={unusable}",
                            prompted.len(),
                            degraded.len(),
                            candidate.len(),
                        ))
                    );
                    for c in classes {
                        let items: String = c
                            .languages
                            .iter()
                            .map(|l| format!(" {} n={} skipped={}", l.language, l.n, l.skipped))
                            .collect();
                        let t = &c.trials;
                        println!("{}", run.labelled(&format!("calibrate class={} items{items}", c.name)));
                        println!(
                            "{}",
                            run.labelled(&format!(
                                "calibrate class={} prompted trials={} nogain={} accepted={} regressed={} noise={:.4}",
                                c.name, t.prompted.trials, t.prompted.nogain, t.prompted.accepted, t.prompted.regressed, t.noise
                            ))
                        );
                        for (side, k) in t.r_roles() {
                            println!(
                                "{}",
                                run.labelled(&format!("calibrate class={} prompted-{side} {}", c.name, k.line()))
                            );
                        }
                        for line in c.degraded_lines(&degraded) {
                            println!("{}", run.labelled(&format!("calibrate class={} {line}", c.name)));
                        }
                        println!(
                            "{}",
                            run.labelled(&format!(
                                "calibrate class={} candidate trials={} accepted={} nogain={} regressed={}",
                                c.name, t.candidate.trials, t.candidate.accepted, t.candidate.nogain, t.candidate.regressed
                            ))
                        );
                        for (label, k) in candidate.iter().zip(&t.candidate_arms) {
                            println!(
                                "{}",
                                run.labelled(&format!("calibrate class={} candidate={label} {}", c.name, k.line()))
                            );
                        }
                    }
                }
            }
        }
        Command::Loop(LoopCommand::Recall { root }) => {
            let run = RunStores::open(&root)?;
            let mut files = run.answer_files(run.recall_dir())?;
            match run.recall(&mut files)? {
                RecallOutcome::Unanswered { prompts } => {
                    for digest in &prompts {
                        println!("unanswered {digest}");
                    }
                    println!("unanswered {} prompts", prompts.len());
                    std::process::exit(3);
                }
                RecallOutcome::Read(reading) => println!("{}", reading.line()),
            }
        }
        Command::Loop(LoopCommand::MintOntology { root, language }) => {
            let run = RunStores::open(&root)?;
            let mut files = run.answer_files(run.ontology_mint_dir())?;
            match run.mint_ontology(&mut files, &language)? {
                OntologyMintOutcome::Unanswered { prompt } => {
                    println!("unanswered {prompt}");
                    std::process::exit(3);
                }
                OntologyMintOutcome::Drafted { prompt, entries } => println!(
                    "drafted {entries} entries {}",
                    run.ontology_mint_dir().join(format!("{prompt}.answer")).display()
                ),
                OntologyMintOutcome::Refused { code } => {
                    println!("{code}");
                    std::process::exit(1);
                }
            }
        }
        Command::Loop(LoopCommand::Cycle { root, ollama_url }) => {
            let unreadable = |_| anyhow::anyhow!("a store would not read");
            let mut run = RunStores::open(&root)?;
            let mut files = run.answer_files(run.next_cycle_dir().map_err(unreadable)?)?;
            let scorer = Scorer::new(OllamaEmbedder::new(&ollama_url, &run.record().embedder));
            match cycle(&mut run, &mut files, &scorer)? {
                CycleOutcome::Unanswered { prompts } => {
                    for digest in &prompts {
                        println!("unanswered {digest}");
                    }
                    println!("unanswered {} prompts", prompts.len());
                    std::process::exit(3);
                }
                CycleOutcome::Decided(decision) => {
                    println!("{}", run.cycle_line(&decision))
                }
            }
        }
        Command::Schema { out } => {
            std::fs::create_dir_all(&out)?;
            for (name, json) in weksa::schema::published() {
                std::fs::write(out.join(&name), json)?;
                println!("wrote {name}");
            }
        }
    }
    Ok(())
}

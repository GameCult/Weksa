//! The `weksa` command line: the same validator and store admission the library
//! exposes, plus text projections (`dump`) that are for review and are not stores.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{Args, Parser, Subcommand, ValueEnum};
use unicode_normalization::UnicodeNormalization;
use weksa::corpus::{CorpusItem, ModelFamily, NameSpan, Source, Turn};
use weksa::entity::{Entity, EntityOrigin, LanguageForms, Pronunciation};
use weksa::inventory::{proof_revision, seed};
use weksa::packet::proof_packet;
use weksa::model::RecordedAnswers;
use weksa::run::{
    AblationOutcome, CycleOutcome, LowererReach, RecallOutcome, RoundOutcome, RunPaths, RunStores, cycle, round,
};
use weksa::score::ScoreInput;
use weksa::{
    AnswerFiles, CorpusStore, EntityId, EntityStore, InventoryStore, OllamaEmbedder, PacketStore,
    Scorer, Split, SplitScheme, StoreError, lower, pronounce, validate,
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
    Packet(PacketCommand),
    /// Lower a stored packet to English text.
    Lower {
        store: PathBuf,
        key: String,
        /// Print the ruled IPA of each entity the packet cites, one per line,
        /// instead of the text; refused for an entity with no ruled pronunciation.
        #[arg(long, conflicts_with = "phones")]
        ipa: bool,
        /// Print the phones line of each entity the packet cites, one JSON
        /// object per line ({language, source, phones: [{ipa, stress}]}),
        /// instead of the text; refused for a phone the inventory lacks.
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
    /// Pin the global --corpus, --inventory and --entities as a run at ROOT.
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
    /// run's record: the global --corpus, --inventory and --entities are not
    /// read. Encode prompts are written first; once they are answered a rerun
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
    /// Run one edit-accept cycle of the run at ROOT. The stores are read only
    /// from the run's record. The editor's prompt, then the candidate round's
    /// encode prompts, then its lowering prompts are written in turn; the
    /// command exits 3 each time until every prompt has an answer.
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
}

#[derive(Subcommand)]
enum EntityCommand {
    /// Admit an entity, or append forms to a stored one.
    Put(Box<PutArgs>),
    /// Text projection of the entity store for review; not a store.
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
        })
    }
}

/// The split of an item as `Train`, `Validation`, `Test` or `excluded`.
fn split_text(item: &CorpusItem) -> String {
    SplitScheme::V1
        .split(item)
        .map_or_else(|| "excluded".to_string(), |s| format!("{s:?}"))
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
        Command::Corpus(CorpusCommand::Dump) => {
            for i in CorpusStore::open(&cli.corpus)?.all()? {
                let split = split_text(&i);
                let withdrawn = if i.withdrawn { " withdrawn" } else { "" };
                println!(
                    "{} [{} {} group={} {split}{withdrawn}]\n  {:?}\n  names: {:?}\n  source: {:?}",
                    i.id, i.language, i.speaker, i.group, i.text, i.names, i.source
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
        Command::Lower {
            store,
            key,
            ipa,
            phones,
        } => {
            let inventory_store = InventoryStore::open(&cli.inventory)?;
            let inventory = inventory_store.inventory()?;
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
            if ipa {
                for line in pronounce(&view, &entities)? {
                    println!("{line}");
                }
            } else if phones {
                for line in weksa::phones(&view, &entities)? {
                    println!("{}", serde_json::to_string(&line)?);
                }
            } else {
                println!("{}", lower(&view, &inventory, &entities)?);
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
            let mut files = AnswerFiles::new(
                run.next_round_dir().map_err(unreadable)?,
                run.record().model.clone(),
            );
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
            let model = run.record().model.clone();
            let mut recorded =
                RecordedAnswers::new(root.join(format!("round-{round}")), model.clone());
            let mut files = AnswerFiles::new(run.ablation_dir(round), model);
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
        Command::Loop(LoopCommand::Recall { root }) => {
            let run = RunStores::open(&root)?;
            let mut files = AnswerFiles::new(run.recall_dir(), run.record().model.clone());
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
        Command::Loop(LoopCommand::Cycle { root, ollama_url }) => {
            let unreadable = |_| anyhow::anyhow!("a store would not read");
            let mut run = RunStores::open(&root)?;
            let mut files = AnswerFiles::new(
                run.next_cycle_dir().map_err(unreadable)?,
                run.record().model.clone(),
            );
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

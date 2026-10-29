//! The `weksa` command line: the same validator and store admission the library
//! exposes, plus text projections (`dump`) that are for review and are not stores.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use unicode_normalization::UnicodeNormalization;
use weksa::corpus::{CorpusItem, ModelFamily, NameSpan, Source, Turn};
use weksa::entity::{Entity, EntityOrigin, LanguageForms};
use weksa::inventory::seed;
use weksa::{
    CorpusStore, EntityId, EntityStore, InventoryStore, PacketStore, SplitScheme, StoreError,
    validate,
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
    /// Print the split of a corpus item.
    Split { item_key: String },
    /// Write the published JSON Schemas.
    Schema {
        #[arg(long, default_value = "schemas/cultnet")]
        out: PathBuf,
    },
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
}

#[derive(Subcommand)]
enum EntityCommand {
    /// Admit an entity, or append forms to a stored one.
    Put(PutArgs),
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
    /// The persona of a `cast` origin; refused with any other origin.
    #[arg(long)]
    persona: Option<String>,
    /// The work of a `public-domain` origin; refused with any other origin.
    #[arg(long)]
    work: Option<String>,
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
            let (language, form) = f
                .split_once('=')
                .with_context(|| format!("--form {f:?} is not language=form"))?;
            match forms.iter_mut().find(|lf| lf.language == language) {
                Some(lf) => lf.forms.push(form.nfc().collect()),
                None => forms.push(LanguageForms {
                    language: language.to_string(),
                    forms: vec![form.nfc().collect()],
                }),
            }
        }
        let origin = match (self.origin, self.persona, self.work) {
            (OriginKind::Cast, persona, None) => EntityOrigin::Cast {
                persona: need(persona, "persona")?,
            },
            (OriginKind::PublicDomain, None, work) => EntityOrigin::PublicDomain {
                work: need(work, "work")?,
            },
            (OriginKind::Live, None, None) => EntityOrigin::Live,
            _ => bail!(
                "--persona is only for --origin cast and --work only for --origin public-domain"
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
    #[arg(long, default_value = "en-US")]
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
    #[arg(long)]
    author: Option<String>,
    #[arg(long)]
    edition_url: Option<String>,
    #[arg(long = "model-family", value_enum)]
    family: Option<ModelFamilyArg>,
    /// The generator string the fixture or run wrote, verbatim.
    #[arg(long)]
    generator: Option<String>,
    #[arg(long)]
    prompt_digest: Option<String>,
    #[arg(long)]
    admitted_on: String,
    #[arg(long)]
    supersedes: Option<String>,
}

fn need(value: Option<String>, flag: &str) -> Result<String> {
    value.with_context(|| format!("this source needs --{flag}"))
}

impl AddArgs {
    fn source(&self) -> Result<Source> {
        let get = |v: &Option<String>, flag: &str| need(v.clone(), flag);
        Ok(match self.source {
            SourceKind::CastAuthored => Source::CastAuthored {
                persona: get(&self.persona, "persona")?,
                fixture: get(&self.fixture, "fixture")?,
            },
            SourceKind::OperatorWritten => Source::OperatorWritten,
            SourceKind::OperatorApproved => Source::OperatorApproved {
                note: get(&self.note, "note")?,
            },
            SourceKind::PublicDomain => Source::PublicDomain {
                work: get(&self.work, "work")?,
                author: get(&self.author, "author")?,
                edition_url: get(&self.edition_url, "edition-url")?,
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
                    .with_context(|| format!("--prior-turn {t:?} is not speaker=text"))?;
                Ok(Turn {
                    speaker: speaker.parse()?,
                    text: text.nfc().collect(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let names = self
            .names
            .iter()
            .map(|n| {
                let bad = || format!("--name {n:?} is not start..end=entity:<slug>");
                let (range, entity) = n.split_once('=').with_context(bad)?;
                let (start, end) = range.split_once("..").with_context(bad)?;
                Ok(NameSpan {
                    entity: entity.parse()?,
                    start: start.parse().with_context(bad)?,
                    end: end.parse().with_context(bad)?,
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
    let cli = Cli::parse();
    match cli.command {
        Command::Validate { store, key } => {
            let inventory = InventoryStore::open(&cli.inventory)?.inventory()?;
            let packets = PacketStore::open(&store)?;
            let packets = match key {
                Some(key) => vec![
                    packets
                        .get(&key)?
                        .with_context(|| format!("no packet {key}"))?,
                ],
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
            let key = store.next_key(&args.slug)?;
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
        Command::Split { item_key } => {
            let item = CorpusStore::open(&cli.corpus)?
                .get(&item_key)?
                .with_context(|| format!("no item {item_key}"))?;
            println!("{}", split_text(&item));
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

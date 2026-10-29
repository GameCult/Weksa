//! Admission to the corpus, inventory and packet stores. Each store is a
//! CultCache over one `SingleFileMessagePackBackingStore` file. These functions
//! are the only writers, so the rules S1..S4 hold for every caller.

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use cultcache_rs::{CultCache, SingleFileMessagePackBackingStore};
use unicode_normalization::is_nfc;
use unicode_segmentation::UnicodeSegmentation;

use crate::corpus::{CorpusItem, CorpusItemId, item_key, key_slug};
use crate::entity::Entity;
use crate::form::is_language_tag;
use crate::inventory::{Change, InvId, Inventory, InventoryEntry, InventoryRevision, revision_key};
use crate::packet::{EntityId, Packet};
use crate::validate::{Refusal, validate};

/// A broken store rule, named.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreRefusal {
    /// S1: an existing item key re-added with any field different.
    S1ItemImmutable { key: String },
    /// S2: a revision that is not latest+1 with parent latest.
    S2RevisionChain { expected: u32, got: u32 },
    /// S3: a revision after a frozen one.
    S3Frozen { frozen: u32 },
    /// S4: an entry whose introduced_rev is not the revision adding it.
    S4IntroducedRev {
        id: InvId,
        introduced: u32,
        revision: u32,
    },
    /// S5: a superseding item's group differs from its predecessor's.
    S5SupersedeGroup {
        key: String,
        expected: String,
        got: String,
    },
    /// S6: an item's normalized text equals another key's.
    S6TextHeld { key: String, held_by: String },
    /// A `provenance.corpus_item` key names no item.
    UnknownCorpusItem { key: String },
    /// A packet key already holds a different packet.
    PacketImmutable { key: String },
    /// An entity id the entity store does not hold: a speaker, a prior-turn
    /// speaker, a name span's entity or a referent's entity.
    UnknownEntity { entity: EntityId },
    /// A name span with `start >= end`, `end` past the text, or an end off a
    /// character boundary.
    NameSpanRange { key: String, start: u32, end: u32 },
    /// A name span that starts or ends inside an extended grapheme cluster
    /// (UAX #29) of the item's text.
    NameSpanSplitsCluster { key: String, start: u32, end: u32 },
    /// An item text or prior-turn text, or an entity form, that is not NFC.
    /// Callers normalize at intake; the stores refuse what they were handed.
    NotNfc { key: String },
    /// An entity beyond a capacity bound.
    EntityBounds { id: EntityId, bound: EntityBound },
    /// Two name spans of one item that overlap.
    NameSpanOverlap { key: String },
    /// A name span whose bytes equal none of its entity's forms in the item's
    /// language (an entity with no forms in that language included).
    NameSpanNotAForm {
        key: String,
        entity: EntityId,
        start: u32,
        end: u32,
    },
    /// A `put` that drops, reorders or edits a stored language, form or origin.
    EntityNotAppendOnly { id: EntityId },
    /// A corpus packet's referent cites an entity that is neither named on its
    /// item nor the item's speaker or a prior-turn speaker.
    EntityNotOnItem { entity: EntityId, item: String },
    /// Any other malformed document or change.
    Malformed { key: String, why: &'static str },
}

/// The capacity bound an entity broke.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityBound {
    /// A language that is not a BCP 47-form tag (1 to 35 bytes).
    LanguageTag,
    /// More than 8 languages.
    Languages,
    /// More than 8 forms in one language.
    Forms,
    /// A form of more than 64 bytes.
    FormBytes,
    /// A form of more than 4 words.
    FormWords,
}

impl EntityBound {
    pub const MAX_LANGUAGES: usize = 8;
    pub const MAX_FORMS: usize = 8;
    pub const MAX_FORM_BYTES: usize = 64;
    pub const MAX_FORM_WORDS: usize = 4;
}

#[derive(Debug)]
pub enum StoreError {
    Refused(StoreRefusal),
    /// The packet broke validator rules.
    Invalid(Vec<Refusal>),
    Io(anyhow::Error),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Refused(r) => write!(f, "refused: {r:?}"),
            StoreError::Invalid(rs) => {
                write!(f, "invalid packet:")?;
                rs.iter().try_for_each(|r| write!(f, " [{r}]"))
            }
            StoreError::Io(e) => write!(f, "{e:#}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<anyhow::Error> for StoreError {
    fn from(e: anyhow::Error) -> Self {
        StoreError::Io(e)
    }
}

impl From<StoreRefusal> for StoreError {
    fn from(r: StoreRefusal) -> Self {
        StoreError::Refused(r)
    }
}

fn malformed(key: &str, why: &'static str) -> StoreError {
    StoreRefusal::Malformed {
        key: key.to_string(),
        why,
    }
    .into()
}

fn open_cache(
    path: &Path,
    register: impl FnOnce(&mut CultCache) -> anyhow::Result<()>,
) -> anyhow::Result<CultCache> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut cache = CultCache::new();
    register(&mut cache)?;
    cache.add_generic_backing_store(SingleFileMessagePackBackingStore::new(path))?;
    cache.pull_all_backing_stores()?;
    Ok(cache)
}

#[derive(Debug, PartialEq, Eq)]
pub enum AddOutcome {
    Added,
    /// The key already held this text; nothing was written.
    Unchanged,
}

pub struct CorpusStore {
    cache: CultCache,
}

impl CorpusStore {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            cache: open_cache(path, |c| c.register_entry_type::<CorpusItem>())?,
        })
    }

    pub fn all(&self) -> anyhow::Result<Vec<CorpusItem>> {
        let mut items = self.cache.get_all::<CorpusItem>()?;
        items.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(items)
    }

    pub fn get(&self, key: &str) -> anyhow::Result<Option<CorpusItem>> {
        self.cache.get::<CorpusItem>(key)
    }

    /// The next free key for `slug`; withdrawn items keep their numbers.
    pub fn next_key(&self, slug: &str) -> anyhow::Result<String> {
        let highest = self
            .all()?
            .iter()
            .filter(|i| key_slug(&i.id) == Some(slug))
            .filter_map(|i| i.id.rsplit_once(':')?.1.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        Ok(item_key(slug, highest + 1))
    }

    /// The item `id` names, or `UnknownCorpusItem` when it holds nothing.
    fn resolve(&self, id: &CorpusItemId) -> Result<CorpusItem, StoreError> {
        self.get(id.as_str())?.ok_or_else(|| {
            StoreRefusal::UnknownCorpusItem {
                key: id.to_string(),
            }
            .into()
        })
    }

    /// S1: an item key holds its whole item for good. S6: a text held under
    /// one key (live or withdrawn) is not re-added under another unless the new
    /// item supersedes that key. S5: a correction stays in its predecessor's
    /// group, so it cannot move across the split.
    pub fn add(
        &mut self,
        item: CorpusItem,
        entities: &EntityStore,
    ) -> Result<AddOutcome, StoreError> {
        if key_slug(&item.id).is_none() {
            return Err(malformed(&item.id, "item key is not item:<slug>:<n>"));
        }
        if item.withdrawn {
            return Err(malformed(
                &item.id,
                "an item is admitted live, then withdrawn",
            ));
        }
        if let Some(existing) = self.get(&item.id)? {
            if existing == item {
                return Ok(AddOutcome::Unchanged);
            }
            return Err(StoreRefusal::S1ItemImmutable { key: item.id }.into());
        }
        if !is_language_tag(&item.language) {
            return Err(malformed(&item.id, "language is not a BCP 47-form tag"));
        }
        if !is_nfc(&item.text) || item.prior_turns.iter().any(|t| !is_nfc(&t.text)) {
            return Err(StoreRefusal::NotNfc { key: item.id }.into());
        }
        check_entities(&item, entities)?;
        let text = normalized(&item.text);
        for held in self.all()? {
            if normalized(&held.text) == text && item.supersedes.as_deref() != Some(&held.id) {
                return Err(StoreRefusal::S6TextHeld {
                    key: item.id,
                    held_by: held.id,
                }
                .into());
            }
        }
        if let Some(old) = &item.supersedes {
            let Some(old) = self.get(old)? else {
                return Err(malformed(
                    &item.id,
                    "supersedes an item that does not exist",
                ));
            };
            if old.group != item.group {
                return Err(StoreRefusal::S5SupersedeGroup {
                    key: item.id,
                    expected: old.group,
                    got: item.group,
                }
                .into());
            }
        }
        self.cache.put(item.id.clone(), &item)?;
        Ok(AddOutcome::Added)
    }

    /// Marks an item withdrawn; its text stays.
    pub fn withdraw(&mut self, key: &str) -> Result<(), StoreError> {
        let Some(mut item) = self.get(key)? else {
            return Err(malformed(key, "no such item"));
        };
        if !item.withdrawn {
            item.withdrawn = true;
            self.cache.put(key, &item)?;
        }
        Ok(())
    }
}

/// Every entity an item cites exists, and every name span is a well-formed,
/// non-overlapping byte range of the text, starting and ending on extended
/// grapheme cluster boundaries, equal to one of its entity's forms in the
/// item's language. Id lookup, UAX #29 boundaries and byte equality: no reading
/// of meaning, so every script is held to the same check.
fn check_entities(item: &CorpusItem, entities: &EntityStore) -> Result<(), StoreError> {
    let known = |id: &EntityId| -> Result<Entity, StoreError> {
        entities
            .get(id)?
            .ok_or_else(|| StoreRefusal::UnknownEntity { entity: id.clone() }.into())
    };
    known(&item.speaker)?;
    for turn in &item.prior_turns {
        known(&turn.speaker)?;
    }
    let mut ranges = Vec::with_capacity(item.names.len());
    let cuts: BTreeSet<usize> = item
        .text
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .chain([item.text.len()])
        .collect();
    for span in &item.names {
        let entity = known(&span.entity)?;
        let (start, end) = (span.start as usize, span.end as usize);
        let Some(bytes) = item
            .text
            .get(start..end)
            .filter(|_| start < end)
            .map(str::as_bytes)
        else {
            return Err(StoreRefusal::NameSpanRange {
                key: item.id.clone(),
                start: span.start,
                end: span.end,
            }
            .into());
        };
        if !cuts.contains(&start) || !cuts.contains(&end) {
            return Err(StoreRefusal::NameSpanSplitsCluster {
                key: item.id.clone(),
                start: span.start,
                end: span.end,
            }
            .into());
        }
        let is_form = entity
            .forms_in(&item.language)
            .is_some_and(|forms| forms.iter().any(|f| f.as_bytes() == bytes));
        if !is_form {
            return Err(StoreRefusal::NameSpanNotAForm {
                key: item.id.clone(),
                entity: span.entity.clone(),
                start: span.start,
                end: span.end,
            }
            .into());
        }
        ranges.push((span.start, span.end));
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|w| w[1].0 < w[0].1) {
        return Err(StoreRefusal::NameSpanOverlap {
            key: item.id.clone(),
        }
        .into());
    }
    Ok(())
}

/// Lowercased alphanumerics only, so case, spacing and punctuation variants of
/// a text compare equal.
fn normalized(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
}

pub struct EntityStore {
    cache: CultCache,
}

impl EntityStore {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            cache: open_cache(path, |c| c.register_entry_type::<Entity>())?,
        })
    }

    pub fn all(&self) -> anyhow::Result<Vec<Entity>> {
        let mut entities = self.cache.get_all::<Entity>()?;
        entities.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(entities)
    }

    pub fn get(&self, id: &EntityId) -> anyhow::Result<Option<Entity>> {
        self.cache.get::<Entity>(id.as_str())
    }

    /// Admits a new entity, an identical replay, or a replacement that keeps
    /// every stored language and form in order and origin as it was, and only
    /// appends: forms grow, never shrink, reorder or change. An entity has 1 to
    /// 8 languages, each a BCP 47-form tag named once, with 1 to 8 NFC forms of
    /// 1 to 64 bytes and at most 4 words, no control character, no edge
    /// whitespace, none repeated.
    pub fn put(&mut self, entity: Entity) -> Result<AddOutcome, StoreError> {
        let key = entity.id.to_string();
        check_entity_shape(&entity)?;
        if let Some(old) = self.get(&entity.id)? {
            if old == entity {
                return Ok(AddOutcome::Unchanged);
            }
            let grows = old.origin == entity.origin
                && old.forms.len() <= entity.forms.len()
                && old.forms.iter().zip(&entity.forms).all(|(o, n)| {
                    o.language == n.language
                        && o.forms.len() <= n.forms.len()
                        && n.forms.starts_with(&o.forms)
                });
            if !grows {
                return Err(StoreRefusal::EntityNotAppendOnly { id: entity.id }.into());
            }
        }
        self.cache.put(key, &entity)?;
        Ok(AddOutcome::Added)
    }
}

fn check_entity_shape(entity: &Entity) -> Result<(), StoreError> {
    let bad = |why| malformed(entity.id.as_str(), why);
    let bound = |bound| -> StoreError {
        StoreRefusal::EntityBounds {
            id: entity.id.clone(),
            bound,
        }
        .into()
    };
    if entity.forms.is_empty() {
        return Err(bad("an entity has forms in at least one language"));
    }
    if entity.forms.len() > EntityBound::MAX_LANGUAGES {
        return Err(bound(EntityBound::Languages));
    }
    let mut languages = BTreeSet::new();
    for lf in &entity.forms {
        if !is_language_tag(&lf.language) {
            return Err(bound(EntityBound::LanguageTag));
        }
        if !languages.insert(&lf.language) {
            return Err(bad("a language is named once"));
        }
        if lf.forms.is_empty() {
            return Err(bad("a language has at least one form"));
        }
        if lf.forms.len() > EntityBound::MAX_FORMS {
            return Err(bound(EntityBound::Forms));
        }
        let mut seen = BTreeSet::new();
        for f in &lf.forms {
            if f.len() > EntityBound::MAX_FORM_BYTES {
                return Err(bound(EntityBound::FormBytes));
            }
            if f.split_whitespace().count() > EntityBound::MAX_FORM_WORDS {
                return Err(bound(EntityBound::FormWords));
            }
            if !is_nfc(f) {
                return Err(StoreRefusal::NotNfc {
                    key: entity.id.to_string(),
                }
                .into());
            }
            if f.is_empty() || f.chars().any(char::is_control) || f.trim() != f.as_str() {
                return Err(bad(
                    "a form is not empty and has no control character or edge whitespace",
                ));
            }
            if !seen.insert(f) {
                return Err(bad("a form is not repeated within its language"));
            }
        }
    }
    Ok(())
}

pub struct InventoryStore {
    cache: CultCache,
}

impl InventoryStore {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let cache = open_cache(path, |c| {
            c.register_entry_type::<InventoryEntry>()?;
            c.register_entry_type::<InventoryRevision>()
        })?;
        Ok(Self { cache })
    }

    pub fn inventory(&self) -> anyhow::Result<Inventory> {
        Ok(Inventory::new(
            self.cache.get_all::<InventoryEntry>()?,
            self.cache.get_all::<InventoryRevision>()?,
        ))
    }

    /// Admits revision `rev` and the entries it adds, whole or not at all.
    /// S2: latest+1 with parent latest. S3: nothing follows a frozen revision.
    /// S4: an added entry's introduced_rev is this revision.
    /// An added entry's gloss is 1 to 120 bytes without a control character.
    /// No door judges text against the corpus: leak is owned by the loop's
    /// context builder and the scorer.
    pub fn admit_revision(
        &mut self,
        rev: InventoryRevision,
        entries: Vec<InventoryEntry>,
    ) -> Result<(), StoreError> {
        let inv = self.inventory()?;
        let latest = inv.latest();
        if let Some(l) = latest.filter(|l| l.frozen) {
            return Err(StoreRefusal::S3Frozen { frozen: l.n }.into());
        }
        let (expected, parent) = latest.map_or((1, None), |l| (l.n + 1, Some(l.n)));
        if rev.n != expected || rev.parent != parent {
            return Err(StoreRefusal::S2RevisionChain {
                expected,
                got: rev.n,
            }
            .into());
        }
        let mut touched: BTreeSet<InvId> = BTreeSet::new();
        let mut staged = Vec::new();
        for change in &rev.changes {
            match change {
                Change::Add(id) => {
                    let Some(entry) = entries.iter().find(|e| &e.id == id) else {
                        return Err(malformed(&id.to_string(), "added id has no entry"));
                    };
                    if entry.introduced_rev != rev.n {
                        let (id, introduced, revision) = (*id, entry.introduced_rev, rev.n);
                        return Err(StoreRefusal::S4IntroducedRev {
                            id,
                            introduced,
                            revision,
                        }
                        .into());
                    }
                    let key = id.to_string();
                    if !entry.well_formed() || entry.retired_rev.is_some() {
                        return Err(malformed(&key, "entry shape does not match its family"));
                    }
                    if inv.entry(id).is_some() || !touched.insert(*id) {
                        return Err(malformed(&key, "id is already in the inventory"));
                    }
                    check_entry_text(entry)?;
                    staged.push(self.cache.prepare_entry_named(key, entry)?.0);
                }
                Change::Retire(id) => {
                    let key = id.to_string();
                    let Some(current) = inv.entry(id).filter(|e| e.retired_rev.is_none()) else {
                        return Err(malformed(
                            &key,
                            "retire target is missing or already retired",
                        ));
                    };
                    if !touched.insert(*id) {
                        return Err(malformed(&key, "id changes twice in one revision"));
                    }
                    let retired = InventoryEntry {
                        retired_rev: Some(rev.n),
                        ..current.clone()
                    };
                    staged.push(self.cache.prepare_entry_named(key, &retired)?.0);
                }
            }
        }
        if let Some(stray) = entries.iter().find(|e| !touched.contains(&e.id)) {
            return Err(malformed(
                &stray.id.to_string(),
                "entry is not in the revision's changes",
            ));
        }
        staged.push(self.cache.prepare_entry_named(revision_key(rev.n), &rev)?.0);
        self.cache.put_prepared_batch(staged)?;
        Ok(())
    }
}

fn check_entry_text(entry: &InventoryEntry) -> Result<(), StoreError> {
    if !(1..=120).contains(&entry.gloss.len()) || entry.gloss.chars().any(char::is_control) {
        return Err(malformed(
            &entry.id.to_string(),
            "gloss is 1 to 120 bytes with no control character",
        ));
    }
    Ok(())
}

pub struct PacketStore {
    cache: CultCache,
}

impl PacketStore {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            cache: open_cache(path, |c| c.register_entry_type::<Packet>())?,
        })
    }

    pub fn all(&self) -> anyhow::Result<Vec<Packet>> {
        let mut packets = self.cache.get_all::<Packet>()?;
        packets.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(packets)
    }

    pub fn get(&self, key: &str) -> anyhow::Result<Option<Packet>> {
        self.cache.get::<Packet>(key)
    }

    /// The only door: a packet exists only if `validate` passes against the
    /// inventory the store holds at the packet's revision, its `corpus_item`
    /// resolves, every referent's entity is in the entity store and is named on
    /// that item or is its speaker or a prior-turn speaker (an id-set check). A
    /// key holds its packet for good. A packet that names no corpus item has no
    /// door here.
    pub fn admit(
        &mut self,
        packet: Packet,
        inventory: &InventoryStore,
        corpus: &CorpusStore,
        entities: &EntityStore,
    ) -> Result<(), StoreError> {
        validate(&packet, &inventory.inventory()?).map_err(StoreError::Invalid)?;
        let item = corpus.resolve(&packet.provenance.corpus_item)?;
        for entity in packet.referents.iter().filter_map(|r| r.entity.as_ref()) {
            if entities.get(entity)?.is_none() {
                return Err(StoreRefusal::UnknownEntity {
                    entity: entity.clone(),
                }
                .into());
            }
            let on_item = item.names.iter().any(|n| &n.entity == entity)
                || &item.speaker == entity
                || item.prior_turns.iter().any(|t| &t.speaker == entity);
            if !on_item {
                return Err(StoreRefusal::EntityNotOnItem {
                    entity: entity.clone(),
                    item: item.id.clone(),
                }
                .into());
            }
        }
        let key = packet.id.to_string();
        if let Some(existing) = self.get(&key)? {
            if existing == packet {
                return Ok(());
            }
            return Err(StoreRefusal::PacketImmutable { key }.into());
        }
        self.cache.put(key, &packet)?;
        Ok(())
    }
}

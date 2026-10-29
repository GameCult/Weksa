//! Admission to the corpus, inventory and packet stores. Each store is a
//! CultCache over one `SingleFileMessagePackBackingStore` file. These functions
//! are the only writers, so the rules S1..S4 hold for every caller.

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use cultcache_rs::{CultCache, SingleFileMessagePackBackingStore};

use crate::corpus::{CorpusItem, item_key, key_slug};
use crate::inventory::{Change, InvId, Inventory, InventoryEntry, InventoryRevision, revision_key};
use crate::packet::Packet;
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
    /// Any other malformed document or change.
    Malformed { key: String, why: &'static str },
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

    /// The item `key` names, or `UnknownCorpusItem` when the key is not
    /// `item:<slug>:<n>` or holds nothing.
    fn resolve(&self, key: &str) -> Result<CorpusItem, StoreError> {
        let unknown = || StoreRefusal::UnknownCorpusItem {
            key: key.to_string(),
        };
        if key_slug(key).is_none() {
            return Err(unknown().into());
        }
        self.get(key)?.ok_or_else(|| unknown().into())
    }

    /// S1: an item key holds its whole item for good. S6: a text held under
    /// one key (live or withdrawn) is not re-added under another unless the new
    /// item supersedes that key. S5: a correction stays in its predecessor's
    /// group, so it cannot move across the split.
    pub fn add(&mut self, item: CorpusItem) -> Result<AddOutcome, StoreError> {
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

/// Lowercased alphanumerics only, so case, spacing and punctuation variants of
/// a text compare equal.
fn normalized(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
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
    /// (when set) resolves, and a key holds its packet for good.
    pub fn admit(
        &mut self,
        packet: Packet,
        inventory: &InventoryStore,
        corpus: &CorpusStore,
    ) -> Result<(), StoreError> {
        validate(&packet, &inventory.inventory()?).map_err(StoreError::Invalid)?;
        if let Some(item_key) = &packet.provenance.corpus_item {
            corpus.resolve(item_key)?;
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

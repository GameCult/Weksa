//! Admission to the corpus, inventory and packet stores. Each store is a
//! CultCache over one `SingleFileMessagePackBackingStore` file. These functions
//! are the only writers, so the rules S1..S4 hold for every caller.

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use caseless::default_case_fold_str;
use cultcache_rs::{CacheBackingStore, CultCache, SingleFileMessagePackBackingStore};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark, is_nfc};
use unicode_segmentation::UnicodeSegmentation;

use crate::corpus::{CorpusItem, CorpusItemId, Source, item_key, key_slug};
use crate::split::{Split, SplitScheme};
use crate::entity::{Entity, EntityOrigin, Pronunciation};
use crate::form::{
    is_attestation, is_calendar_date, is_joining_only, is_language_tag, is_refused_format, is_slug,
};
use crate::inventory::{
    Change, InvId, Inventory, InventoryEntry, InventoryRevision, Slot, revision_key,
};
use crate::ontology::{
    MAX_CONDITIONS, MAX_CHOICE_BYTES, MAX_ENTRIES_PER_LANGUAGE, MAX_EXAMPLE_BYTES, MAX_EXAMPLES,
    Ontology, OntologyEntry, Test,
};
use crate::packet::{EntityId, FeatValue, Packet};
use crate::speaker::{Annotator, MAX_PARTICIPANTS, Party, Scene, Speaker, scene_key, speaker_key};
use crate::validate::{LoweringInput, Refusal, feature_refusals, validate};

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
    /// A new packet names a corpus item already withdrawn. Packets admitted
    /// before the withdrawal stay stored and readable.
    WithdrawnCorpusItem { key: String },
    /// A speaker or scene revision that is not the entity's or group's latest
    /// plus one (1 when none).
    RevisionNotNext {
        key: String,
        expected: u32,
        got: u32,
    },
    /// A scene that omits the speaker of a live item of its group.
    SceneSpeakerMissing { group: String, entity: EntityId },
    /// Any other malformed document or change.
    Malformed { key: String, why: &'static str },
}

/// What the CLI prints: the rule broken and the numbers and reasons that
/// explain it. Item keys, entity ids and groups are the caller's identifiers
/// and are left to the typed value, never to text.
impl fmt::Display for StoreRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use StoreRefusal::*;
        match self {
            S1ItemImmutable { .. } => f.write_str("S1ItemImmutable"),
            S2RevisionChain { expected, got } => {
                write!(f, "S2RevisionChain expected {expected} got {got}")
            }
            S3Frozen { frozen } => write!(f, "S3Frozen at {frozen}"),
            S4IntroducedRev {
                introduced,
                revision,
                ..
            } => write!(f, "S4IntroducedRev {introduced} in revision {revision}"),
            S5SupersedeGroup { .. } => f.write_str("S5SupersedeGroup"),
            S6TextHeld { .. } => f.write_str("S6TextHeld"),
            UnknownCorpusItem { .. } => f.write_str("UnknownCorpusItem"),
            PacketImmutable { .. } => f.write_str("PacketImmutable"),
            UnknownEntity { .. } => f.write_str("UnknownEntity"),
            NameSpanRange { start, end, .. } => write!(f, "NameSpanRange {start}..{end}"),
            NameSpanSplitsCluster { start, end, .. } => {
                write!(f, "NameSpanSplitsCluster {start}..{end}")
            }
            NotNfc { .. } => f.write_str("NotNfc"),
            EntityBounds { bound, .. } => write!(f, "EntityBounds {bound:?}"),
            NameSpanOverlap { .. } => f.write_str("NameSpanOverlap"),
            NameSpanNotAForm { start, end, .. } => write!(f, "NameSpanNotAForm {start}..{end}"),
            EntityNotAppendOnly { .. } => f.write_str("EntityNotAppendOnly"),
            EntityNotOnItem { .. } => f.write_str("EntityNotOnItem"),
            WithdrawnCorpusItem { .. } => f.write_str("WithdrawnCorpusItem"),
            RevisionNotNext { expected, got, .. } => {
                write!(f, "RevisionNotNext expected {expected} got {got}")
            }
            SceneSpeakerMissing { .. } => f.write_str("SceneSpeakerMissing"),
            Malformed { why, .. } => write!(f, "Malformed: {why}"),
        }
    }
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
}

impl EntityBound {
    pub const MAX_LANGUAGES: usize = 8;
    pub const MAX_FORMS: usize = 8;
    pub const MAX_FORM_BYTES: usize = 64;
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
            StoreError::Refused(r) => write!(f, "refused: {r}"),
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

pub(crate) fn open_cache(
    path: &Path,
    register: impl FnOnce(&mut CultCache) -> anyhow::Result<()>,
) -> anyhow::Result<CultCache> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    open_cache_over(SingleFileMessagePackBackingStore::new(path), register)
}

pub(crate) fn open_cache_over(
    store: impl CacheBackingStore + 'static,
    register: impl FnOnce(&mut CultCache) -> anyhow::Result<()>,
) -> anyhow::Result<CultCache> {
    let mut cache = CultCache::new();
    register(&mut cache)?;
    cache.add_generic_backing_store(store)?;
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

    /// The text of every item in a Test group, any language, withdrawn and
    /// model-written ones included: what no prompt may hold.
    pub fn test_texts(&self) -> anyhow::Result<Vec<String>> {
        Ok(self
            .all()?
            .into_iter()
            .filter(|item| SplitScheme::V1.of_bucket(SplitScheme::V1.bucket(&item.group)) == Split::Test)
            .map(|item| item.text)
            .collect())
    }

    pub fn get(&self, key: &str) -> anyhow::Result<Option<CorpusItem>> {
        self.cache.get::<CorpusItem>(key)
    }

    /// The next free key for `slug`; withdrawn items keep their numbers.
    pub fn next_key(&self, slug: &str) -> Result<String, StoreError> {
        let highest = self
            .all()?
            .iter()
            .filter(|i| key_slug(&i.id) == Some(slug))
            .filter_map(|i| i.id.rsplit_once(':')?.1.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        let next = highest
            .checked_add(1)
            .ok_or_else(|| malformed(slug, "slug has no free key"))?;
        Ok(item_key(slug, next))
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
        let text = normalized(&item.text);
        // Read from the text as given: NFKC maps modifier letters (ʰ) to base letters.
        // A text of marks, joiners and modifiers says so; any other text with
        // no alphanumeric character (punctuation, space) gets the plainer reason.
        if !item.text.chars().any(is_base) {
            let marks_only = item.text.chars().any(|c| {
                is_combining_mark(c)
                    || matches!(c, '\u{200c}' | '\u{200d}')
                    || is_modifier_letter(c)
            });
            return Err(malformed(
                &item.id,
                if marks_only {
                    "text has no base letter or digit"
                } else {
                    "text has no alphanumeric character"
                },
            ));
        }
        if !is_calendar_date(&item.admitted_on) {
            return Err(malformed(&item.id, "admitted_on is not a YYYY-MM-DD date"));
        }
        if let Some(why) = empty_attestation(&item.source) {
            return Err(malformed(&item.id, why));
        }
        if let Some(by) = &item.names_by {
            check_annotator(&item.id, by)?;
        }
        check_entities(&item, entities)?;
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

/// The S6 key of a text: Unicode compatibility caseless matching (D146: NFKC,
/// default case fold, NFKC) keeping letters, digits and combining marks
/// (General_Category L, N, M) so tone marks and viramas keep words apart. Composed and decomposed,
/// fullwidth, ligature and case-fold variants (straße, STRASSE) and spacing and
/// punctuation variants compare equal. Three writings of one thing also fold:
/// Turkish dotless i and dotted capital I read as i, katakana reads as the
/// hiragana of the same syllable, and Arabic-Indic and Extended Arabic-Indic
/// digits read as 0-9. Homoglyphs across scripts (Cyrillic a for Latin a) stay
/// apart: no key here reads what a character looks like. A combining dot above
/// (U+0307) after i is dropped, so İx and i̇x collide as the old case fold made
/// them; so does plain ix, with both. Over-merging a rare sequence errs toward
/// held, a lost collision toward leak. U+0307 after any other letter stays.
pub(crate) fn normalized(text: &str) -> String {
    let mapped: String = text.nfkc().flat_map(fold_variant).collect();
    // The dot goes before the last NFKC so the marks left on the i compose with
    // it (i, dot, acute reads as the i with an acute).
    let mut dotless = String::new();
    for c in default_case_fold_str(&mapped).chars() {
        if c == '\u{307}' && dotless.ends_with('i') {
            continue;
        }
        dotless.push(c);
    }
    dotless
        .nfkc()
        .filter(|&c| c.is_alphanumeric() || is_combining_mark(c))
        .collect()
}

/// The one-to-one folds `normalized` adds to NFKC and case folding, applied
/// before the case fold. (U+0130 needs no arm: it folds to i plus a combining
/// dot, and the `dotless` loop in `normalized` drops that dot after an `i`, so
/// the key is plain `i`.)
fn fold_variant(c: char) -> impl Iterator<Item = char> {
    let one = |c: char| (c, None);
    let (first, second) = match c {
        '\u{131}' => one('i'),
        // Voiced katakana wa, wi, we, wo: the hiragana (U+308F-3092) plus the
        // combining voiced mark, which is how NFKC spells the hiragana forms.
        '\u{30f7}'..='\u{30fa}' => (
            char::from_u32(c as u32 - 0x68).unwrap_or(c),
            Some('\u{3099}'),
        ),
        // Katakana letters and small letters to hiragana; the iteration marks
        // ヽヾ to ゝゞ. The prolonged sound mark U+30FC stays.
        '\u{30a1}'..='\u{30f6}' | '\u{30fd}'..='\u{30fe}' => {
            one(char::from_u32(c as u32 - 0x60).unwrap_or(c))
        }
        '\u{660}'..='\u{669}' => one(char::from(b'0' + (c as u32 - 0x660) as u8)),
        '\u{6f0}'..='\u{6f9}' => one(char::from(b'0' + (c as u32 - 0x6f0) as u8)),
        _ => one(c),
    };
    std::iter::once(first).chain(second)
}

/// A character that can carry a word: alphanumeric and neither a combining
/// mark (Mn, Mc, Me) nor a modifier letter (Lm). A text of none is marks and
/// modifiers only, like a text of punctuation.
fn is_base(c: char) -> bool {
    c.is_alphanumeric() && !is_combining_mark(c) && !is_modifier_letter(c)
}

/// General_Category Lm, Unicode 17 (the Rust 1.99 tables), as inclusive ranges.
fn is_modifier_letter(c: char) -> bool {
    const LM: [(u32, u32); 79] = [
        (0x2b0, 0x2c1),
        (0x2c6, 0x2d1),
        (0x2e0, 0x2e4),
        (0x2ec, 0x2ec),
        (0x2ee, 0x2ee),
        (0x374, 0x374),
        (0x37a, 0x37a),
        (0x559, 0x559),
        (0x640, 0x640),
        (0x6e5, 0x6e6),
        (0x7f4, 0x7f5),
        (0x7fa, 0x7fa),
        (0x81a, 0x81a),
        (0x824, 0x824),
        (0x828, 0x828),
        (0x8c9, 0x8c9),
        (0x971, 0x971),
        (0xe46, 0xe46),
        (0xec6, 0xec6),
        (0x10fc, 0x10fc),
        (0x17d7, 0x17d7),
        (0x1843, 0x1843),
        (0x1aa7, 0x1aa7),
        (0x1c78, 0x1c7d),
        (0x1d2c, 0x1d6a),
        (0x1d78, 0x1d78),
        (0x1d9b, 0x1dbf),
        (0x2071, 0x2071),
        (0x207f, 0x207f),
        (0x2090, 0x209c),
        (0x2c7c, 0x2c7d),
        (0x2d6f, 0x2d6f),
        (0x2e2f, 0x2e2f),
        (0x3005, 0x3005),
        (0x3031, 0x3035),
        (0x303b, 0x303b),
        (0x309d, 0x309e),
        (0x30fc, 0x30fe),
        (0xa015, 0xa015),
        (0xa4f8, 0xa4fd),
        (0xa60c, 0xa60c),
        (0xa67f, 0xa67f),
        (0xa69c, 0xa69d),
        (0xa717, 0xa71f),
        (0xa770, 0xa770),
        (0xa788, 0xa788),
        (0xa7f1, 0xa7f4),
        (0xa7f8, 0xa7f9),
        (0xa9cf, 0xa9cf),
        (0xa9e6, 0xa9e6),
        (0xaa70, 0xaa70),
        (0xaadd, 0xaadd),
        (0xaaf3, 0xaaf4),
        (0xab5c, 0xab5f),
        (0xab69, 0xab69),
        (0xff70, 0xff70),
        (0xff9e, 0xff9f),
        (0x10780, 0x10785),
        (0x10787, 0x107b0),
        (0x107b2, 0x107ba),
        (0x10d4e, 0x10d4e),
        (0x10d6f, 0x10d6f),
        (0x10ec5, 0x10ec5),
        (0x11dd9, 0x11dd9),
        (0x16b40, 0x16b43),
        (0x16d40, 0x16d42),
        (0x16d6b, 0x16d6c),
        (0x16f93, 0x16f9f),
        (0x16fe0, 0x16fe1),
        (0x16fe3, 0x16fe3),
        (0x16ff2, 0x16ff3),
        (0x1aff0, 0x1aff3),
        (0x1aff5, 0x1affb),
        (0x1affd, 0x1affe),
        (0x1e030, 0x1e06d),
        (0x1e137, 0x1e13d),
        (0x1e4eb, 0x1e4eb),
        (0x1e6ff, 0x1e6ff),
        (0x1e94b, 0x1e94b),
    ];
    let c = c as u32;
    LM.binary_search_by(|&(lo, hi)| {
        if hi < c {
            std::cmp::Ordering::Less
        } else if lo > c {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    })
    .is_ok()
}

/// The field that holds an unattested provenance string, if any: a string with
/// no alphanumeric, a control or format character, or over 200 bytes attests
/// nothing.
fn empty_attestation(source: &Source) -> Option<&'static str> {
    let empty = |s: &String| !is_attestation(s);
    let empty_some = |s: &Option<String>| s.as_ref().is_some_and(empty);
    match source {
        Source::CastAuthored { persona, .. } if empty(persona) => {
            Some("a cast-authored source names its persona")
        }
        Source::CastAuthored { fixture, .. } if empty(fixture) => {
            Some("a cast-authored source names its fixture")
        }
        Source::CastAuthored { author, .. } if empty(author) => {
            Some("a cast-authored source names its author")
        }
        Source::OperatorApproved { note } if empty(note) => {
            Some("an operator-approved source carries its note")
        }
        Source::PublicDomain { work, .. } if empty(work) => {
            Some("a public-domain source names its work")
        }
        Source::PublicDomain { author, .. } if empty(author) => {
            Some("a public-domain source names its author")
        }
        Source::PublicDomain { edition_url, .. } if empty(edition_url) => {
            Some("a public-domain source names its edition url")
        }
        Source::PublicDomain { licence, .. } if empty(licence) => {
            Some("a public-domain source names its licence")
        }
        Source::PublicDomain { jurisdiction, .. } if empty(jurisdiction) => {
            Some("a public-domain source names its jurisdiction")
        }
        Source::PublicDomain { translator, .. } if empty(translator) => {
            Some("a public-domain source names its translator")
        }
        Source::PublicDomain {
            edition_year: 0, ..
        } => Some("a public-domain source names its edition year"),
        Source::ModelWritten { generator, .. } if empty_some(generator) => {
            Some("a model-written source names a non-empty generator")
        }
        Source::ModelWritten { prompt_digest, .. } if empty_some(prompt_digest) => {
            Some("a model-written source carries a non-empty prompt digest")
        }
        _ => None,
    }
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
    /// 1 to 64 bytes, no control or invisible format character (joiners and
    /// variation selectors stay), no edge whitespace, none repeated; the origin
    /// attests where it came from with strings that hold an alphanumeric, no control or format character and at most 200 bytes.
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
                        && (o.pronunciation.is_none() || o.pronunciation == n.pronunciation)
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
    let (persona, fixture, work) = match &entity.origin {
        EntityOrigin::Cast { persona, fixture } => (Some(persona), Some(fixture), None),
        EntityOrigin::Live { persona } => (Some(persona), None, None),
        EntityOrigin::PublicDomain { work } => (None, None, Some(work)),
    };
    if persona.is_some_and(|s| !is_attestation(s)) {
        return Err(bad("an origin names its persona"));
    }
    if fixture.is_some_and(|s| !is_attestation(s)) {
        return Err(bad("a cast origin names its fixture"));
    }
    if work.is_some_and(|s| !is_attestation(s)) {
        return Err(bad("a public-domain origin names its work"));
    }
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
        if let Some(p) = &lf.pronunciation {
            check_pronunciation(entity, p)?;
        }
        let mut seen = BTreeSet::new();
        for f in &lf.forms {
            if f.len() > EntityBound::MAX_FORM_BYTES {
                return Err(bound(EntityBound::FormBytes));
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
            if f.chars().any(is_refused_format) {
                return Err(bad("a form holds an invisible format character"));
            }
            if f.chars().all(is_joining_only) {
                return Err(bad("a form is more than joiners and selectors"));
            }
            if !seen.insert(f) {
                return Err(bad("a form is not repeated within its language"));
            }
        }
    }
    Ok(())
}

/// A pronunciation is bare IPA, NFC, with no brackets, control character or
/// edge whitespace; every rejected variant is the same, differs from the ruled
/// one and is named once; the ruling names its author, a `YYYY-MM-DD` date and
/// its source.
fn check_pronunciation(entity: &Entity, p: &Pronunciation) -> Result<(), StoreError> {
    let bad = |why| malformed(entity.id.as_str(), why);
    let ipa_ok = |s: &str| {
        !s.is_empty()
            && s.len() <= EntityBound::MAX_FORM_BYTES
            && is_nfc(s)
            && s.trim() == s
            && !s
                .chars()
                .any(|c| c.is_control() || is_refused_format(c) || "/[]".contains(c))
    };
    if !ipa_ok(&p.ipa) {
        return Err(bad(
            "a pronunciation is bare NFC IPA with no brackets or slashes",
        ));
    }
    if !is_attestation(&p.ruled_by) || !is_attestation(&p.source) {
        return Err(bad("a pronunciation names who ruled it and where"));
    }
    if !is_calendar_date(&p.date) {
        return Err(bad("a pronunciation is dated YYYY-MM-DD"));
    }
    if p.rejected.len() > EntityBound::MAX_FORMS {
        return Err(StoreRefusal::EntityBounds {
            id: entity.id.clone(),
            bound: EntityBound::Forms,
        }
        .into());
    }
    let mut seen = BTreeSet::from([&p.ipa]);
    if !p.rejected.iter().all(|r| ipa_ok(r) && seen.insert(r)) {
        return Err(bad(
            "a rejected pronunciation is well-formed, named once and not the ruled one",
        ));
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

    /// The only door, and the only source of a [`LoweringInput`]: a packet
    /// exists only if the validator's rules pass against the inventory the
    /// store holds at the packet's revision, its `corpus_item` resolves to an
    /// item that is not withdrawn, every referent's entity is in the entity store and is named on
    /// that item or is its speaker or a prior-turn speaker (an id-set check). A
    /// key holds its packet for good. A packet that names no corpus item has no
    /// door here.
    pub fn admit(
        &mut self,
        packet: Packet,
        inventory: &InventoryStore,
        corpus: &CorpusStore,
        entities: &EntityStore,
    ) -> Result<LoweringInput, StoreError> {
        validate(&packet, &inventory.inventory()?).map_err(StoreError::Invalid)?;
        let item = corpus.resolve(&packet.provenance.corpus_item)?;
        if item.withdrawn {
            return Err(StoreRefusal::WithdrawnCorpusItem {
                key: item.id.clone(),
            }
            .into());
        }
        for entity in packet.referents.iter().filter_map(|r| r.entity.as_ref()) {
            if entities.get(entity)?.is_none() {
                return Err(StoreRefusal::UnknownEntity {
                    entity: entity.clone(),
                }
                .into());
            }
            let on_item = item.cited_entities().any(|e| e == entity);
            if !on_item {
                return Err(StoreRefusal::EntityNotOnItem {
                    entity: entity.clone(),
                    item: item.id.clone(),
                }
                .into());
            }
        }
        let view = LoweringInput::of(&packet, &item);
        let key = packet.id.to_string();
        if let Some(existing) = self.get(&key)? {
            if existing == packet {
                return Ok(view);
            }
            return Err(StoreRefusal::PacketImmutable { key }.into());
        }
        self.cache.put(key, &packet)?;
        Ok(view)
    }
}

/// The speaker and scene documents: append-only revisions, one key each. The
/// only writer of `weksa.speaker` and `weksa.scene`.
pub struct SpeakerStateStore {
    cache: CultCache,
}

impl SpeakerStateStore {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            cache: open_cache(path, |c| {
                c.register_entry_type::<Speaker>()?;
                c.register_entry_type::<Scene>()
            })?,
        })
    }

    /// Admits the next revision of a speaker, or an identical replay. The key
    /// derives from the document. Revision `n` is the entity's latest plus one
    /// (1 when none). The entity is in the entity store, the features pass
    /// `feature_refusals` in slot `Speaker` at the document's inventory
    /// revision, and a model annotator's generator is an attestation.
    pub fn put_speaker(
        &mut self,
        s: Speaker,
        inventory: &InventoryStore,
        entities: &EntityStore,
    ) -> Result<AddOutcome, StoreError> {
        let key = speaker_key(&s.entity, s.n);
        if self.get_speaker(&key)?.is_some_and(|old| old == s) {
            return Ok(AddOutcome::Unchanged);
        }
        check_annotator(&key, &s.annotator)?;
        known_entity(entities, &s.entity)?;
        let out = feature_refusals(
            &inventory.inventory()?,
            s.inventory_rev,
            &s.features,
            Slot::Speaker,
        );
        if !out.is_empty() {
            return Err(StoreError::Invalid(out));
        }
        let latest = self.speaker(&s.entity)?.map_or(0, |l| l.n);
        check_next(&key, latest, s.n)?;
        self.cache.put(key, &s)?;
        Ok(AddOutcome::Added)
    }

    /// Admits the next revision of a scene, or an identical replay. The group
    /// is a slug some corpus item (live or withdrawn) holds; there are 1 to
    /// `MAX_PARTICIPANTS` participants, each named once and in the entity
    /// store; a relation's `from` is a participant, a named `to` is another
    /// participant, and an ordered pair has at most one relation; every speaker
    /// of a live item of the group is a participant. Features pass
    /// `feature_refusals` in slot `Scene` for every relation.
    pub fn put_scene(
        &mut self,
        s: Scene,
        inventory: &InventoryStore,
        corpus: &CorpusStore,
        entities: &EntityStore,
    ) -> Result<AddOutcome, StoreError> {
        let key = scene_key(&s.group, s.n);
        if self.get_scene(&key)?.is_some_and(|old| old == s) {
            return Ok(AddOutcome::Unchanged);
        }
        let bad = |why| malformed(&key, why);
        if !is_slug(&s.group) {
            return Err(bad("a scene group is a slug"));
        }
        check_annotator(&key, &s.annotator)?;
        let items: Vec<CorpusItem> = corpus
            .all()?
            .into_iter()
            .filter(|i| i.group == s.group)
            .collect();
        if items.is_empty() {
            return Err(bad("no corpus item holds the scene's group"));
        }
        if s.participants.is_empty() || s.participants.len() > MAX_PARTICIPANTS {
            return Err(bad("a scene has 1 to 64 participants"));
        }
        let mut seen = BTreeSet::new();
        for p in &s.participants {
            if !seen.insert(p) {
                return Err(bad("a participant is named once"));
            }
            known_entity(entities, p)?;
        }
        let mut pairs = BTreeSet::new();
        for r in &s.relations {
            if !seen.contains(&r.from) {
                return Err(bad("a relation's from is a participant"));
            }
            if let Party::Named(to) = &r.to
                && (!seen.contains(to) || to == &r.from)
            {
                return Err(bad("a relation's named to is another participant"));
            }
            if !pairs.insert((&r.from, &r.to)) {
                return Err(bad("an ordered pair has one relation"));
            }
        }
        if let Some(entity) = s.uncovered(items.iter().filter(|i| !i.withdrawn).map(|i| &i.speaker))
        {
            return Err(StoreRefusal::SceneSpeakerMissing {
                entity: entity.clone(),
                group: s.group,
            }
            .into());
        }
        let inv = inventory.inventory()?;
        let mut out = Vec::new();
        if s.relations.is_empty() {
            out = feature_refusals(&inv, s.inventory_rev, &[], Slot::Scene);
        }
        for r in &s.relations {
            out.extend(feature_refusals(
                &inv,
                s.inventory_rev,
                &r.features,
                Slot::Scene,
            ));
        }
        if !out.is_empty() {
            return Err(StoreError::Invalid(out));
        }
        let latest = self.scene(&s.group)?.map_or(0, |l| l.n);
        check_next(&key, latest, s.n)?;
        self.cache.put(key, &s)?;
        Ok(AddOutcome::Added)
    }

    /// The latest revision (highest `n`) of the entity's speaker document.
    pub fn speaker(&self, entity: &EntityId) -> anyhow::Result<Option<Speaker>> {
        Ok(self
            .all_speakers()?
            .into_iter()
            .filter(|s| &s.entity == entity)
            .max_by_key(|s| s.n))
    }

    /// The latest revision (highest `n`) of the group's scene document.
    pub fn scene(&self, group: &str) -> anyhow::Result<Option<Scene>> {
        Ok(self
            .all_scenes()?
            .into_iter()
            .filter(|s| s.group == group)
            .max_by_key(|s| s.n))
    }

    pub fn get_speaker(&self, key: &str) -> anyhow::Result<Option<Speaker>> {
        self.cache.get::<Speaker>(key)
    }

    pub fn get_scene(&self, key: &str) -> anyhow::Result<Option<Scene>> {
        self.cache.get::<Scene>(key)
    }

    /// Every speaker revision, by entity then revision number.
    pub fn all_speakers(&self) -> anyhow::Result<Vec<Speaker>> {
        let mut all = self.cache.get_all::<Speaker>()?;
        all.sort_by(|a, b| (&a.entity, a.n).cmp(&(&b.entity, b.n)));
        Ok(all)
    }

    /// Every scene revision, by group then revision number.
    pub fn all_scenes(&self) -> anyhow::Result<Vec<Scene>> {
        let mut all = self.cache.get_all::<Scene>()?;
        all.sort_by(|a, b| (&a.group, a.n).cmp(&(&b.group, b.n)));
        Ok(all)
    }
}

fn known_entity(entities: &EntityStore, id: &EntityId) -> Result<(), StoreError> {
    match entities.get(id)? {
        Some(_) => Ok(()),
        None => Err(StoreRefusal::UnknownEntity { entity: id.clone() }.into()),
    }
}

fn check_annotator(key: &str, a: &Annotator) -> Result<(), StoreError> {
    match a {
        Annotator::Model { generator, .. } if !is_attestation(generator) => {
            Err(malformed(key, "a model annotator attests its generator"))
        }
        _ => Ok(()),
    }
}

fn check_next(key: &str, latest: u32, got: u32) -> Result<(), StoreError> {
    let expected = latest
        .checked_add(1)
        .ok_or_else(|| malformed(key, "no revision follows the latest"))?;
    if got != expected {
        return Err(StoreRefusal::RevisionNotNext {
            key: key.to_string(),
            expected,
            got,
        }
        .into());
    }
    Ok(())
}

/// The language ontology: immutable entries admitted in all-or-nothing
/// batches. The only writer of `weksa.ontology_entry`.
pub struct OntologyStore {
    cache: CultCache,
}

impl OntologyStore {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            cache: open_cache(path, |c| c.register_entry_type::<OntologyEntry>())?,
        })
    }

    /// Every entry, by language then number.
    pub fn all(&self) -> anyhow::Result<Vec<OntologyEntry>> {
        Ok(self.ontology()?.entries().to_vec())
    }

    pub fn ontology(&self) -> anyhow::Result<Ontology> {
        Ok(Ontology::of(self.cache.get_all::<OntologyEntry>()?))
    }

    /// Admits a batch whole or not at all; an identical replay of stored
    /// entries is `Unchanged`. Per language the batch's numbers run the
    /// language's latest plus one, consecutively, to at most
    /// `MAX_ENTRIES_PER_LANGUAGE` entries in all. Every entry passes
    /// [`check_ontology_entry`] against the inventory before any is put. No
    /// message echoes an entry's text.
    pub fn admit(
        &mut self,
        batch: Vec<OntologyEntry>,
        inventory: &InventoryStore,
    ) -> Result<AddOutcome, StoreError> {
        let all = self.cache.get_all::<OntologyEntry>()?;
        if batch
            .iter()
            .all(|e| all.iter().any(|old| old == e))
        {
            return Ok(AddOutcome::Unchanged);
        }
        let inv = inventory.inventory()?;
        let mut counted: std::collections::BTreeMap<&str, u32> = std::collections::BTreeMap::new();
        for e in &batch {
            let key = e.key();
            if !is_language_tag(&e.language) {
                return Err(malformed(&key, "an ontology language is a language tag"));
            }
            let latest = match counted.get(e.language.as_str()) {
                Some(n) => *n,
                None => all
                    .iter()
                    .filter(|o| o.language == e.language)
                    .map(|o| o.n)
                    .max()
                    .unwrap_or(0),
            };
            check_next(&key, latest, e.n)?;
            if e.n as usize > MAX_ENTRIES_PER_LANGUAGE {
                return Err(malformed(&key, "a language holds at most 64 entries"));
            }
            counted.insert(e.language.as_str(), e.n);
            check_ontology_entry(e, &inv, e.inventory_rev)?;
        }
        let mut staged = Vec::new();
        for e in &batch {
            staged.push(self.cache.prepare_entry_named(e.key(), e)?.0);
        }
        self.cache.put_prepared_batch(staged)?;
        Ok(AddOutcome::Added)
    }
}

/// The rules of one ontology entry against an inventory: the annotator, the
/// bounded and normalized text of the choice and examples, the condition count
/// and each condition's feature (slot `Stance`, `Discourse` or `Scene`, a grade
/// feature for `AtLeast`, `AtMost` and `AtMostOrUnset`, an option or flag value for `Is`) read at
/// `rev`. The store passes the entry's own `inventory_rev` on admission; a run
/// passes its inventory's head, because beds number revisions independently.
pub(crate) fn check_ontology_entry(
    e: &OntologyEntry,
    inventory: &Inventory,
    rev: u32,
) -> Result<(), StoreError> {
    let key = e.key();
    check_annotator(&key, &e.annotator)?;
    let text_ok = |t: &str, max: usize| {
        (1..=max).contains(&t.len())
            && !t.chars().any(|c| c.is_control() || is_refused_format(c))
            && is_nfc(t)
    };
    if !text_ok(&e.choice, MAX_CHOICE_BYTES) {
        return Err(malformed(
            &key,
            "a choice is 1 to 200 normalized bytes with no control or format character",
        ));
    }
    if e.examples.len() > MAX_EXAMPLES
        || !e.examples.iter().all(|x| text_ok(x, MAX_EXAMPLE_BYTES))
    {
        return Err(malformed(
            &key,
            "an entry has at most 3 examples of 1 to 120 normalized bytes with no control or format character",
        ));
    }
    if e.when.len() > MAX_CONDITIONS {
        return Err(malformed(&key, "an entry has at most 4 conditions"));
    }
    let mut out = Vec::new();
    for c in &e.when {
        if !matches!(c.slot, Slot::Stance | Slot::Discourse | Slot::Scene) {
            return Err(malformed(
                &key,
                "a condition reads slot Stance, Discourse or Scene",
            ));
        }
        if matches!(c.test, Test::Is(FeatValue::Grade(_))) {
            return Err(malformed(&key, "Is names an option or a flag"));
        }
        out.extend(feature_refusals(
            inventory,
            rev,
            &[c.as_value()],
            c.slot,
        ));
    }
    if e.when.is_empty() && !inventory.has_revision(rev) {
        out.extend(feature_refusals(inventory, rev, &[], Slot::Stance));
    }
    if !out.is_empty() {
        return Err(StoreError::Invalid(out));
    }
    Ok(())
}

//! The inventory: the ids packets name, as CultCache documents under appended
//! revisions. An entry is never edited in place; a changed meaning is a retire
//! plus an add, and a retired id still resolves for packets of earlier revisions.

use std::collections::BTreeMap;
use std::fmt;
use std::num::NonZeroU32;

use cultcache_rs::DatabaseEntry;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::form::{FormError, string_form};

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub enum Family {
    Concept,
    Frame,
    Role,
    Relation,
    SpeechAct,
    Feature,
    ReferentKind,
    /// A game- or loop-declared packet field; the entry carries its value kind.
    Extension,
}

impl Family {
    pub const ALL: [Family; 8] = [
        Family::Concept,
        Family::Frame,
        Family::Role,
        Family::Relation,
        Family::SpeechAct,
        Family::Feature,
        Family::ReferentKind,
        Family::Extension,
    ];

    pub fn prefix(self) -> &'static str {
        match self {
            Family::Concept => "concept",
            Family::Frame => "frame",
            Family::Role => "role",
            Family::Relation => "relation",
            Family::SpeechAct => "speech_act",
            Family::Feature => "feature",
            Family::ReferentKind => "referent_kind",
            Family::Extension => "extension",
        }
    }

    /// The id of the `n`th entry of this family.
    pub fn id(self, n: NonZeroU32) -> InvId {
        InvId { family: self, n }
    }
}

/// Id of an inventory entry: `<family prefix>:<n>`, `n` a canonical decimal
/// (no sign, no leading zero) of at least 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct InvId {
    family: Family,
    n: NonZeroU32,
}

impl InvId {
    pub fn parse(s: &str) -> Result<Self, FormError> {
        let bad = || FormError::new("InvId", s);
        let (prefix, n) = s.split_once(':').ok_or_else(bad)?;
        let family = Family::ALL
            .into_iter()
            .find(|f| f.prefix() == prefix)
            .ok_or_else(bad)?;
        if n.starts_with('0') || !n.bytes().all(|b| b.is_ascii_digit()) {
            return Err(bad());
        }
        let n = n.parse::<NonZeroU32>().map_err(|_| bad())?;
        Ok(Self { family, n })
    }

    pub fn family(self) -> Family {
        self.family
    }
}

impl fmt::Display for InvId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.family.prefix(), self.n)
    }
}

string_form!(InvId, { "type": "string", "pattern": "^(concept|frame|role|relation|speech_act|feature|referent_kind|extension):[1-9][0-9]*$" });

/// A value label of an options feature: `^[a-z][a-z0-9_]{0,23}$`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OptionLabel(String);

impl OptionLabel {
    pub fn parse(s: &str) -> Result<Self, FormError> {
        let mut bytes = s.bytes();
        let ok = s.len() <= 24
            && bytes.next().is_some_and(|b| b.is_ascii_lowercase())
            && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if ok {
            Ok(Self(s.to_string()))
        } else {
            Err(FormError::new("OptionLabel", s))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OptionLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

string_form!(OptionLabel, { "type": "string", "pattern": "^[a-z][a-z0-9_]{0,23}$" });

/// 1 to 16 distinct labels.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<OptionLabel>", into = "Vec<OptionLabel>")]
pub struct OptionSet(Vec<OptionLabel>);

impl OptionSet {
    pub const MAX: usize = 16;

    pub fn new(labels: Vec<OptionLabel>) -> Result<Self, FormError> {
        let mut distinct: Vec<&OptionLabel> = labels.iter().collect();
        distinct.sort();
        distinct.dedup();
        if (1..=Self::MAX).contains(&labels.len()) && distinct.len() == labels.len() {
            Ok(Self(labels))
        } else {
            Err(FormError::new(
                "OptionSet",
                format!("{} labels", labels.len()),
            ))
        }
    }

    pub fn contains(&self, label: &OptionLabel) -> bool {
        self.0.contains(label)
    }
}

impl TryFrom<Vec<OptionLabel>> for OptionSet {
    type Error = FormError;
    fn try_from(v: Vec<OptionLabel>) -> Result<Self, FormError> {
        Self::new(v)
    }
}

impl From<OptionSet> for Vec<OptionLabel> {
    fn from(s: OptionSet) -> Self {
        s.0
    }
}

impl JsonSchema for OptionSet {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "OptionSet".into()
    }
    fn json_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "array",
            "items": g.subschema_for::<OptionLabel>(),
            "minItems": 1,
            "maxItems": 16,
            "uniqueItems": true
        })
    }
}

/// Where a feature may sit in a packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Slot {
    Discourse,
    Referent,
    Predication,
    Relation,
    Stance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ValueKind {
    Grade,
    Options(OptionSet),
    Flag,
}

/// Key: the entry's id, `<family>:<n>`.
#[derive(Clone, Debug, PartialEq, Eq, JsonSchema, DatabaseEntry)]
#[cultcache(type = "weksa.inventory_entry")]
pub struct InventoryEntry {
    #[cultcache(key = 0)]
    pub id: InvId,
    /// 1 to 120 bytes, no control character (checked at admission).
    #[cultcache(key = 1)]
    pub gloss: String,
    /// Present exactly on features and extensions.
    #[cultcache(key = 2)]
    pub value_kind: Option<ValueKind>,
    /// Non-empty exactly on features.
    #[cultcache(key = 3)]
    pub applies_to: Vec<Slot>,
    #[cultcache(key = 4)]
    pub introduced_rev: u32,
    #[cultcache(key = 5)]
    pub retired_rev: Option<u32>,
}

impl InventoryEntry {
    pub fn family(&self) -> Family {
        self.id.family()
    }

    /// Whether the entry's shape matches its family: features carry a value kind
    /// and slots, extensions a value kind and no slots, nothing else either.
    pub fn well_formed(&self) -> bool {
        match self.family() {
            Family::Feature => self.value_kind.is_some() && !self.applies_to.is_empty(),
            Family::Extension => self.value_kind.is_some() && self.applies_to.is_empty(),
            _ => self.value_kind.is_none() && self.applies_to.is_empty(),
        }
    }

    /// Whether a packet of inventory revision `rev` may name this entry.
    pub fn live_at(&self, rev: u32) -> bool {
        self.introduced_rev <= rev && self.retired_rev.is_none_or(|r| r > rev)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Change {
    Add(InvId),
    Retire(InvId),
}

/// Key: `rev:<n>`.
#[derive(Clone, Debug, PartialEq, Eq, JsonSchema, DatabaseEntry)]
#[cultcache(type = "weksa.inventory_revision")]
pub struct InventoryRevision {
    #[cultcache(key = 0)]
    pub n: u32,
    #[cultcache(key = 1)]
    pub parent: Option<u32>,
    #[cultcache(key = 2)]
    pub changes: Vec<Change>,
    #[cultcache(key = 3)]
    pub frozen: bool,
}

pub fn revision_key(n: u32) -> String {
    format!("rev:{n}")
}

/// The whole inventory as the store holds it.
#[derive(Clone, Debug, Default)]
pub struct Inventory {
    entries: BTreeMap<InvId, InventoryEntry>,
    revisions: BTreeMap<u32, InventoryRevision>,
}

impl Inventory {
    /// An inventory exists only as `InventoryStore::inventory` returns it, so
    /// no caller can validate or admit against a fabricated one.
    ///
    /// ```compile_fail,E0624
    /// // `Inventory::new` is not callable outside the crate.
    /// let _ = weksa::Inventory::new(vec![], vec![]);
    /// ```
    pub(crate) fn new(entries: Vec<InventoryEntry>, revisions: Vec<InventoryRevision>) -> Self {
        Self {
            entries: entries.into_iter().map(|e| (e.id, e)).collect(),
            revisions: revisions.into_iter().map(|r| (r.n, r)).collect(),
        }
    }

    pub fn entries(&self) -> impl Iterator<Item = &InventoryEntry> {
        self.entries.values()
    }

    pub fn entry(&self, id: &InvId) -> Option<&InventoryEntry> {
        self.entries.get(id)
    }

    pub fn revisions(&self) -> impl Iterator<Item = &InventoryRevision> {
        self.revisions.values()
    }

    pub fn has_revision(&self, n: u32) -> bool {
        self.revisions.contains_key(&n)
    }

    pub fn latest(&self) -> Option<&InventoryRevision> {
        self.revisions.values().next_back()
    }

    /// The entry `id`, if a packet of revision `rev` may name it.
    pub fn resolve(&self, id: &InvId, rev: u32) -> Option<&InventoryEntry> {
        self.entry(id).filter(|e| e.live_at(rev))
    }
}

/// Revision 1: the standard's enumerated values and the nuance features the
/// loop starts from. Concepts start empty.
pub fn seed() -> (InventoryRevision, Vec<InventoryEntry>) {
    let mut entries: Vec<InventoryEntry> = Vec::new();
    let mut add = |family: Family, gloss: &str, kind: Option<ValueKind>, slots: &[Slot]| {
        let taken = entries.iter().filter(|e| e.family() == family).count() as u32;
        entries.push(InventoryEntry {
            id: family.id(NonZeroU32::MIN.saturating_add(taken)),
            gloss: gloss.to_string(),
            value_kind: kind,
            applies_to: slots.to_vec(),
            introduced_rev: 1,
            retired_rev: None,
        });
    };
    for g in [
        "person",
        "group",
        "organism",
        "place",
        "artifact",
        "institution",
        "ecology",
        "route",
        "memory",
        "event",
        "abstract",
    ] {
        add(Family::ReferentKind, g, None, &[]);
    }
    for g in [
        "coreference",
        "part_of",
        "member_of",
        "located_at",
        "near",
        "before",
        "after",
        "causes",
        "enables",
        "prevents",
        "owns",
        "owes",
        "knows",
        "names",
        "represents",
        "contrasts_with",
    ] {
        add(Family::Relation, g, None, &[]);
    }
    for g in [
        "agent",
        "patient",
        "experiencer",
        "instrument",
        "source",
        "goal",
        "location",
        "beneficiary",
    ] {
        add(Family::Role, g, None, &[]);
    }
    for g in [
        "assert",
        "ask",
        "request",
        "promise",
        "warn",
        "greet",
        "thank",
        "apologise",
    ] {
        add(Family::SpeechAct, g, None, &[]);
    }
    let options = |labels: &[&str]| {
        let labels = labels
            .iter()
            .map(|l| OptionLabel::parse(l).expect("seed labels are well formed"))
            .collect();
        Some(ValueKind::Options(
            OptionSet::new(labels).expect("seed option sets are well formed"),
        ))
    };
    let pred = [Slot::Predication];
    add(
        Family::Feature,
        "polarity",
        options(&["positive", "negative"]),
        &pred,
    );
    add(
        Family::Feature,
        "tense",
        options(&["past", "present", "future", "timeless"]),
        &pred,
    );
    add(
        Family::Feature,
        "aspect",
        options(&[
            "perfective",
            "imperfective",
            "progressive",
            "habitual",
            "perfect",
        ]),
        &pred,
    );
    add(
        Family::Feature,
        "modality",
        options(&[
            "actual",
            "potential",
            "possible",
            "necessary",
            "desired",
            "obligated",
        ]),
        &pred,
    );
    add(
        Family::Feature,
        "evidentiality",
        options(&["direct", "inferred", "reported", "assumed"]),
        &pred,
    );
    let nuance = [Slot::Discourse, Slot::Predication];
    for g in ["hedge", "certainty"] {
        add(Family::Feature, g, Some(ValueKind::Grade), &nuance);
    }
    add(Family::Feature, "irony", Some(ValueKind::Flag), &nuance);
    // Felt affect (permille of how strongly it is felt) and the restraint the speaker
    // puts on it: slot Stance only. Affection covers fondness toward the
    // addressee. Displayed affect is each language's ontology's business.
    let stance = [Slot::Stance];
    for g in [
        "anger",
        "fear",
        "sadness",
        "joy",
        "disgust",
        "surprise",
        "contempt",
        "affection",
        "shame",
        "restraint",
    ] {
        add(Family::Feature, g, Some(ValueKind::Grade), &stance);
    }
    let changes = entries.iter().map(|e| Change::Add(e.id)).collect();
    (
        InventoryRevision {
            n: 1,
            parent: None,
            changes,
            frozen: false,
        },
        entries,
    )
}

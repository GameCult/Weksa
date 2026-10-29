//! The interlingua packet. Every id is an opaque form type: a node id `n<k>`, a
//! packet id `packet:<hex16>`, an encoder id `encoder:<hex16>`, an inventory id
//! `<family>:<n>`. The only free text is the trace, which is stripped before
//! lowering. A malformed id cannot be built, and cannot be read back from a
//! store, so a packet cannot carry target-language wording through an id slot.

use std::fmt;

use cultcache_rs::DatabaseEntry;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::corpus::CorpusItemId;
use crate::form::{FormError, is_slug, string_form};
pub use crate::inventory::InvId;
use crate::inventory::OptionLabel;

/// Bumped by a commit that changes these types; the published schema moves with it.
pub const SCHEMA_VERSION: u32 = 4;

/// Id of a referent or predication within one packet: `n<k>`, `k` a canonical
/// decimal 1..=9999.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NodeId(String);

impl NodeId {
    pub fn parse(s: &str) -> Result<Self, FormError> {
        let k = s
            .strip_prefix('n')
            .filter(|d| !d.starts_with('0') && d.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|d| d.parse::<u16>().ok())
            .filter(|k| (1..=9999).contains(k));
        k.map(|_| Self(s.to_string()))
            .ok_or_else(|| FormError::new("NodeId", s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

string_form!(NodeId, { "type": "string", "pattern": "^n[1-9][0-9]{0,3}$" });

/// `<prefix>:<16 lowercase hex>`.
fn hex16(prefix: &str, s: &str) -> bool {
    s.strip_prefix(prefix)
        .is_some_and(|h| h.len() == 16 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

/// Key of a packet: `packet:<hex16>`, a digest the loop derives from the item,
/// the inventory revision and the encoder.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PacketId(String);

impl PacketId {
    pub fn parse(s: &str) -> Result<Self, FormError> {
        if hex16("packet:", s) {
            Ok(Self(s.to_string()))
        } else {
            Err(FormError::new("PacketId", s))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PacketId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

string_form!(PacketId, { "type": "string", "pattern": "^packet:[0-9a-f]{16}$" });

/// `encoder:<hex16>`, a digest of the model id and the prompt digest.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EncoderId(String);

impl EncoderId {
    pub fn parse(s: &str) -> Result<Self, FormError> {
        if hex16("encoder:", s) {
            Ok(Self(s.to_string()))
        } else {
            Err(FormError::new("EncoderId", s))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EncoderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

string_form!(EncoderId, { "type": "string", "pattern": "^encoder:[0-9a-f]{16}$" });

/// Id of an entity: `entity:<slug>`, a slug as an item key's (`[a-z0-9]` and
/// inner `-`, 1 to 48 bytes, no edge `-`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EntityId(String);

impl EntityId {
    pub fn parse(s: &str) -> Result<Self, FormError> {
        match s.strip_prefix("entity:") {
            Some(slug) if is_slug(slug) => Ok(Self(s.to_string())),
            _ => Err(FormError::new("EntityId", s)),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

string_form!(EntityId, { "type": "string", "pattern": "^entity:[a-z0-9]([a-z0-9-]{0,46}[a-z0-9])?$" });

/// Permille, 0..=1000.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct Grade(u16);

impl Grade {
    pub const MAX: u16 = 1000;

    pub fn new(permille: u16) -> Result<Self, FormError> {
        if permille <= Self::MAX {
            Ok(Self(permille))
        } else {
            Err(FormError::new("Grade", permille))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for Grade {
    type Error = FormError;
    fn try_from(v: u16) -> Result<Self, FormError> {
        Self::new(v)
    }
}

impl From<Grade> for u16 {
    fn from(g: Grade) -> u16 {
        g.0
    }
}

impl JsonSchema for Grade {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Grade".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "integer", "minimum": 0, "maximum": 1000 })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSchema, DatabaseEntry)]
#[cultcache(type = "weksa.interlingua_packet")]
pub struct Packet {
    #[cultcache(key = 0)]
    pub id: PacketId,
    #[cultcache(key = 1)]
    pub schema_version: u32,
    #[cultcache(key = 2)]
    pub inventory_rev: u32,
    #[cultcache(key = 3)]
    pub kind: PacketKind,
    #[cultcache(key = 4)]
    pub provenance: Provenance,
    #[cultcache(key = 5)]
    pub context: Context,
    #[cultcache(key = 6)]
    pub discourse: Discourse,
    #[cultcache(key = 7)]
    pub referents: Vec<Referent>,
    #[cultcache(key = 8)]
    pub predications: Vec<Predication>,
    #[cultcache(key = 9)]
    pub relations: Vec<Relation>,
    #[cultcache(key = 10)]
    pub constraints: Constraints,
    #[cultcache(key = 11)]
    pub extensions: Vec<Extension>,
    #[cultcache(key = 12)]
    pub trace: Trace,
    #[cultcache(key = 13)]
    pub stance: Stance,
}

impl Packet {
    /// The most referents, predications and relations one packet holds: a
    /// calibration constant of ours, capacity and not meaning.
    pub const MAX_NODES: usize = 64;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PacketKind {
    Utterance,
    NameRequest,
    CommentaryIntent,
    Narration,
    DialogueLine,
    CorpusExample,
    DebugProbe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ProvenanceSource {
    Authored,
    ModelDecomposition,
    GameTelemetry,
    ImportedCorpus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Provenance {
    pub source: ProvenanceSource,
    /// The corpus item this packet was minted for. Required: the packet store
    /// admits only packets that name their item.
    pub corpus_item: CorpusItemId,
    pub encoder: EncoderId,
    pub confidence: Grade,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Context {
    /// Concept ids.
    pub setting: Option<InvId>,
    pub medium: Option<InvId>,
    pub participants: Vec<NodeId>,
    pub audience: Vec<NodeId>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Discourse {
    /// Speech-act id.
    pub speech_act: Option<InvId>,
    pub addressee: Option<NodeId>,
    pub features: Vec<FeatureValue>,
}

/// The speaker's felt affect and restraint, never displayed affect: how the
/// speaker shows a feeling is each language's ontology's business. A
/// held-back feeling is not a claim, so it is never a predication.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Stance {
    pub features: Vec<FeatureValue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FeatureValue {
    /// Feature id.
    pub feature: InvId,
    pub value: FeatValue,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum FeatValue {
    Grade(Grade),
    /// A label the feature's inventory entry declares.
    Option(OptionLabel),
    Flag(bool),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Referent {
    pub id: NodeId,
    /// Referent-kind id.
    pub kind: InvId,
    /// Concept id.
    pub concept: InvId,
    /// The entity this referent names, when it has one. The entity store holds
    /// its wording; the packet holds only the id.
    pub entity: Option<EntityId>,
    pub features: Vec<FeatureValue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Predication {
    pub id: NodeId,
    /// Concept id.
    pub concept: InvId,
    /// Frame id.
    pub frame: InvId,
    pub roles: Vec<RoleFill>,
    pub features: Vec<FeatureValue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RoleFill {
    /// Role id.
    pub role: InvId,
    pub filler: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Relation {
    /// Relation id.
    pub relation: InvId,
    pub from: NodeId,
    pub to: NodeId,
    pub features: Vec<FeatureValue>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Constraints {
    pub preserve: Vec<NodeId>,
    pub deterministic: bool,
    pub max_latency_ms: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Extension {
    /// Extension-entry id; the inventory declares its value kind.
    pub key: InvId,
    pub value: FeatValue,
}

/// Authoring and debug notes. Stripped before lowering.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Trace {
    pub notes: Vec<String>,
    pub rejected_readings: Vec<String>,
}

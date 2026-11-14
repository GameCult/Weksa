//! The encoder, editor and lowerer steps: what a model's answer becomes. This is
//! the one place JSON is read (the xenos boundary): a model answers in JSON,
//! `PacketDraft::parse`, `EditDraft::parse` or `LoweringDraft::parse` reads it
//! (all through `from_json`), and nothing is stored as JSON. The lowerer writes
//! `text` only. The encoder writes meaning
//! only; `into_packet` sets the id, schema version, inventory revision, kind,
//! provenance, constraints and extensions, and an empty trace, so a model that
//! saw a Validation item cannot hand its words to whoever reads packets. The
//! editor writes family, gloss, value kind and slots only; `into_revision` sets
//! every id and revision number. The public-domain intake file (`WorkDraft`) and
//! its name-span answer (`MarkedLine`) are read here too.

use std::fmt;
use std::num::NonZeroU32;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use unicode_normalization::UnicodeNormalization;

use crate::corpus::{CorpusItem, CorpusItemId, NameSpan, Source, item_key};
use crate::entity::{Entity, EntityOrigin, LanguageForms};
use crate::form::is_slug;
use crate::packet::EntityId;
use crate::inventory::{
    Change, Family, InvId, Inventory, InventoryEntry, InventoryRevision, Slot, ValueKind,
};
use crate::ontology::{Condition, Layer, OntologyEntry};
use crate::packet::{
    Constraints, Context, Discourse, EncoderId, Grade, Packet, PacketId, PacketKind, Predication,
    Provenance, ProvenanceSource, Referent, Relation, SCHEMA_VERSION, Stance, Trace,
};
use crate::speaker::Annotator;
use crate::split::{fnv1a64, hex16};

/// The meaning fields of a packet, as a model writes them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PacketDraft {
    pub confidence: Grade,
    #[serde(default)]
    pub context: Context,
    pub discourse: Discourse,
    pub referents: Vec<Referent>,
    pub predications: Vec<Predication>,
    #[serde(default)]
    pub relations: Vec<Relation>,
    #[serde(default)]
    pub stance: Stance,
}

/// Where an answer stopped being a draft. Position only: a parse message would
/// quote the offending token.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DraftError {
    pub line: usize,
    pub column: usize,
    /// What kind of reading failure, from serde's own fixed vocabulary. It
    /// never holds a token of the answer.
    pub class: DraftClass,
}

/// Why an answer is not a draft: a closed set of classes, never the offending
/// text, so a record may keep it for a held-out item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DraftClass {
    /// Not JSON.
    Syntax,
    /// JSON that ends early.
    Eof,
    MissingField,
    UnknownField,
    /// A value of the wrong JSON type, such as a bare integer for an id.
    InvalidType,
    /// A value of the right type that is not one of the field's, such as a
    /// number out of range.
    InvalidValue,
    /// A string that is not the form its field names: a node id spelled
    /// `entity:x`, an id of the wrong kind ([`crate::form::FormError`]).
    BadForm,
    Other,
}

impl DraftClass {
    /// The code a round record keeps.
    pub fn code(self) -> &'static str {
        match self {
            DraftClass::Syntax => "syntax",
            DraftClass::Eof => "eof",
            DraftClass::MissingField => "missing-field",
            DraftClass::UnknownField => "unknown-field",
            DraftClass::InvalidType => "invalid-type",
            DraftClass::InvalidValue => "invalid-value",
            DraftClass::BadForm => "bad-form",
            DraftClass::Other => "other",
        }
    }

    fn of(e: &serde_json::Error) -> Self {
        use serde_json::error::Category;
        match e.classify() {
            Category::Syntax => DraftClass::Syntax,
            Category::Eof => DraftClass::Eof,
            Category::Io => DraftClass::Other,
            Category::Data => Self::of_data(&e.to_string()),
        }
    }

    /// serde_json offers no kind for a data error: its message opens with serde's
    /// own fixed words, or with a [`crate::form::FormError`]'s `not a `. Only
    /// those words are read, to pick a class; the message is not kept, and any
    /// other opening is [`DraftClass::Other`].
    fn of_data(msg: &str) -> Self {
        if msg.starts_with("missing field") {
            DraftClass::MissingField
        } else if msg.starts_with("unknown field") {
            DraftClass::UnknownField
        } else if msg.starts_with("invalid type") {
            DraftClass::InvalidType
        } else if msg.starts_with("invalid value") {
            DraftClass::InvalidValue
        } else if msg.starts_with("not a ") {
            DraftClass::BadForm
        } else {
            DraftClass::Other
        }
    }
}

impl fmt::Display for DraftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "not a draft at line {} column {}",
            self.line, self.column
        )
    }
}

impl fmt::Debug for DraftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for DraftError {}

/// The one JSON read of a model's answer.
fn from_json<T: DeserializeOwned>(answer: &str) -> Result<T, DraftError> {
    serde_json::from_str(answer.trim()).map_err(|e| DraftError {
        line: e.line(),
        column: e.column(),
        class: DraftClass::of(&e),
    })
}

impl PacketDraft {
    pub fn parse(answer: &str) -> Result<Self, DraftError> {
        from_json(answer)
    }

    /// The draft that says what `packet` says.
    pub fn of(packet: &Packet) -> Self {
        Self {
            confidence: packet.provenance.confidence,
            context: packet.context.clone(),
            discourse: packet.discourse.clone(),
            referents: packet.referents.clone(),
            predications: packet.predications.clone(),
            relations: packet.relations.clone(),
            stance: packet.stance.clone(),
        }
    }

    /// The packet this draft becomes for `item` at inventory revision `rev`.
    pub(crate) fn into_packet(self, item: &CorpusItemId, rev: u32, encoder: &EncoderId) -> Packet {
        Packet {
            id: packet_id(item, rev, encoder),
            schema_version: SCHEMA_VERSION,
            inventory_rev: rev,
            kind: PacketKind::CorpusExample,
            provenance: Provenance {
                source: ProvenanceSource::ModelDecomposition,
                corpus_item: item.clone(),
                encoder: encoder.clone(),
                confidence: self.confidence,
            },
            context: self.context,
            discourse: self.discourse,
            referents: self.referents,
            predications: self.predications,
            relations: self.relations,
            constraints: Constraints::default(),
            extensions: vec![],
            trace: Trace::default(),
            stance: self.stance,
        }
    }
}

/// The most changes (adds and retires together) one edit may make.
pub const MAX_EDIT_CHANGES: usize = 8;

/// One entry an edit adds, as the editor writes it: no id and no revision.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryDraft {
    pub family: Family,
    pub gloss: String,
    #[serde(default)]
    pub value_kind: Option<ValueKind>,
    #[serde(default)]
    pub applies_to: Vec<Slot>,
}

/// An inventory edit as the editor writes it: entries to add and ids to retire.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditDraft {
    #[serde(default)]
    pub add: Vec<EntryDraft>,
    #[serde(default)]
    pub retire: Vec<InvId>,
}

impl EditDraft {
    pub fn parse(answer: &str) -> Result<Self, DraftError> {
        from_json(answer)
    }

    /// The revision after `base` this edit asks for, and the entries it adds.
    /// `None` when the edit makes no change or more than `max_changes`. The
    /// loop's cycle passes [`MAX_EDIT_CHANGES`]; the CLI's `inventory admit`
    /// passes its own larger cap for a draft reviewed whole in its cut.
    /// Every id is the store's: an added entry takes the next number of its
    /// family after the highest in `inventory`, retired entries included, in
    /// draft order. Nothing else is judged here: the gloss, the entry's shape,
    /// whether a retire target is live and a repeated id are
    /// `InventoryStore::admit_revision`'s to refuse.
    pub fn into_revision(
        self,
        inventory: &Inventory,
        base: u32,
        max_changes: usize,
    ) -> Option<(InventoryRevision, Vec<InventoryEntry>)> {
        let count = self.add.len() + self.retire.len();
        if count == 0 || count > max_changes {
            return None;
        }
        let n = base.checked_add(1)?;
        let mut entries: Vec<InventoryEntry> = Vec::new();
        for draft in self.add {
            let highest = inventory
                .entries()
                .map(|e| e.id)
                .chain(entries.iter().map(|e| e.id))
                .filter(|id| id.family() == draft.family)
                .map(|id| id.n().get())
                .max()
                .unwrap_or(0);
            entries.push(InventoryEntry {
                id: draft.family.id(NonZeroU32::new(highest.checked_add(1)?)?),
                gloss: draft.gloss,
                value_kind: draft.value_kind,
                applies_to: draft.applies_to,
                introduced_rev: n,
                retired_rev: None,
            });
        }
        let changes = entries
            .iter()
            .map(|e| Change::Add(e.id))
            .chain(self.retire.into_iter().map(Change::Retire))
            .collect();
        let revision = InventoryRevision {
            n,
            parent: Some(base),
            changes,
            frozen: false,
        };
        Some((revision, entries))
    }
}

/// One ontology entry as a model writes it: no language, number, inventory
/// revision or annotator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryOntologyDraft {
    pub layer: Layer,
    #[serde(default)]
    pub when: Vec<Condition>,
    pub choice: String,
    #[serde(default)]
    pub examples: Vec<String>,
}

/// A batch of ontology entries as a model drafts them: the xenos door of a
/// seed answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OntologyDraft {
    pub entries: Vec<EntryOntologyDraft>,
}

impl OntologyDraft {
    pub fn parse(answer: &str) -> Result<Self, DraftError> {
        from_json(answer)
    }

    /// The entries this draft becomes: numbered from `first_n` in draft order,
    /// all of `language` read at `inventory_rev`, annotated `annotator`.
    /// Whether they are admissible is `OntologyStore::admit`'s to refuse.
    pub fn into_entries(
        self,
        language: &str,
        inventory_rev: u32,
        first_n: u32,
        annotator: &Annotator,
    ) -> Vec<OntologyEntry> {
        self.entries
            .into_iter()
            .zip(first_n..)
            .map(|(d, n)| OntologyEntry {
                language: language.to_string(),
                n,
                inventory_rev,
                layer: d.layer,
                when: d.when,
                choice: d.choice,
                examples: d.examples,
                annotator: annotator.clone(),
            })
            .collect()
    }
}

/// A lowering as the lowerer writes it: the English text, nothing else.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoweringDraft {
    pub text: String,
}

/// The most bytes a lowering may hold: well over any one utterance, far under
/// a megabyte the embedder would be asked to read.
pub const MAX_LOWERING_BYTES: usize = 4096;

impl LoweringDraft {
    pub fn parse(answer: &str) -> Result<Self, DraftError> {
        from_json(answer)
    }

    /// Whether the text can be scored as an utterance: something after trimming,
    /// at most [`MAX_LOWERING_BYTES`] bytes (not chars), and no control character other than a
    /// newline or tab (a carriage return is refused). Nothing is judged about the words.
    pub fn usable(&self) -> bool {
        !self.text.trim().is_empty()
            && self.text.len() <= MAX_LOWERING_BYTES
            && !self
                .text
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    }
}

/// One cast member of a work: the entity and the forms the work spells it with,
/// in the work's language, citation form first.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CastDraft {
    pub entity: EntityId,
    pub forms: Vec<String>,
}

/// One line of a work: its scene group, the speaker its speech prefix names, text.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineDraft {
    pub group: String,
    pub speaker: EntityId,
    pub text: String,
}

/// A transcription of a public-domain work in hand: provenance, cast forms,
/// lines, scene groups and the speaker each speech prefix names. It holds no
/// distributor header, licence or trademark text, and nothing reads a URL.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkDraft {
    pub slug: String,
    pub language: String,
    pub admitted_on: String,
    pub source: Source,
    pub cast: Vec<CastDraft>,
    pub lines: Vec<LineDraft>,
}

/// One line of a name-span answer: the line number and the line copied with
/// `[[entity:<slug>|<form>]]` around each place it names a cast member.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarkedLine {
    pub line: u32,
    pub marked: String,
}

impl MarkedLine {
    pub fn parse(answer: &str) -> Result<Self, DraftError> {
        from_json(answer)
    }
}

/// Why an import was refused before any document was written. The text names
/// the kind and the line number only, never a value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportRefusal {
    NotPublicDomain,
    CastRepeated,
    NoSuchLine { line: u32 },
    LineMarkedTwice { line: u32 },
    Group { line: u32 },
    SpeakerNotInCast { line: u32 },
    Markup { line: u32 },
    NotInCast { line: u32 },
}

impl fmt::Display for ImportRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPublicDomain => write!(f, "the work's source is not public domain"),
            Self::CastRepeated => write!(f, "a cast entity is listed twice"),
            Self::NoSuchLine { line } => write!(f, "the answer marks a line that is not there (line {line})"),
            Self::LineMarkedTwice { line } => write!(f, "line {line} is marked twice"),
            Self::Group { line } => write!(f, "the group of line {line} is not a slug"),
            Self::SpeakerNotInCast { line } => write!(f, "the speaker of line {line} is not in the cast"),
            Self::Markup { line } => write!(f, "the marked text of line {line} is not its text with markup"),
            Self::NotInCast { line } => write!(f, "line {line} marks an entity outside the cast"),
        }
    }
}

impl std::error::Error for ImportRefusal {}

impl WorkDraft {
    pub fn parse(file: &str) -> Result<Self, DraftError> {
        from_json(file)
    }

    /// The cast entities and the corpus items for the lines `marked` covers, in
    /// line order, each item's spans read from its markup and labelled
    /// `annotator`. Decides everything before any write; whether the documents
    /// are admissible is `EntityStore::put` and `CorpusStore::add`'s to refuse.
    pub fn into_import(
        self,
        marked: &[MarkedLine],
        annotator: &Annotator,
    ) -> Result<(Vec<Entity>, Vec<CorpusItem>), ImportRefusal> {
        let Source::PublicDomain { work, .. } = &self.source else {
            return Err(ImportRefusal::NotPublicDomain);
        };
        let mut entities = Vec::new();
        for c in &self.cast {
            if entities.iter().any(|e: &Entity| e.id == c.entity) {
                return Err(ImportRefusal::CastRepeated);
            }
            entities.push(Entity {
                id: c.entity.clone(),
                forms: vec![LanguageForms {
                    language: self.language.clone(),
                    forms: c.forms.iter().map(|f| f.nfc().collect()).collect(),
                    pronunciation: None,
                }],
                origin: EntityOrigin::PublicDomain { work: work.clone() },
            });
        }
        let in_cast = |id: &EntityId| entities.iter().any(|e| &e.id == id);
        let mut answer: Vec<&MarkedLine> = marked.iter().collect();
        answer.sort_by_key(|m| m.line);
        let mut items = Vec::new();
        for (i, m) in answer.iter().enumerate() {
            let line = m.line;
            if line == 0 || line as usize > self.lines.len() {
                return Err(ImportRefusal::NoSuchLine { line });
            }
            if i > 0 && answer[i - 1].line == line {
                return Err(ImportRefusal::LineMarkedTwice { line });
            }
            let draft = &self.lines[line as usize - 1];
            let group = format!("{}-{}", self.slug, draft.group);
            if !is_slug(&group) {
                return Err(ImportRefusal::Group { line });
            }
            if !in_cast(&draft.speaker) {
                return Err(ImportRefusal::SpeakerNotInCast { line });
            }
            let text: String = draft.text.nfc().collect();
            if text.contains("[[") || text.contains("]]") {
                return Err(ImportRefusal::Markup { line });
            }
            let marked_nfc: String = m.marked.nfc().collect();
            let (plain, names) = match unmark(&marked_nfc) {
                Some((plain, names)) if plain == text => (plain, names),
                _ => return Err(ImportRefusal::Markup { line }),
            };
            if names.iter().any(|n| !in_cast(&n.entity)) {
                return Err(ImportRefusal::NotInCast { line });
            }
            items.push(CorpusItem {
                id: item_key(&self.slug, line),
                text: plain,
                language: self.language.clone(),
                speaker: draft.speaker.clone(),
                group,
                prior_turns: Vec::new(),
                source: self.source.clone(),
                admitted_on: self.admitted_on.clone(),
                supersedes: None,
                withdrawn: false,
                names,
                names_by: Some(annotator.clone()),
            });
        }
        Ok((entities, items))
    }
}

/// The text of `marked` with `[[entity|form]]` replaced by `form`, and the
/// byte span of each form in that text. `None` for any malformed markup. The
/// only search is for the markup delimiters.
fn unmark(marked: &str) -> Option<(String, Vec<NameSpan>)> {
    let mut out = String::with_capacity(marked.len());
    let mut names = Vec::new();
    let mut rest = marked;
    while let Some(at) = rest.find("[[") {
        out.push_str(&rest[..at]);
        let inner = &rest[at + 2..];
        let (id, after) = inner.split_once('|')?;
        let (form, tail) = after.split_once("]]")?;
        if form.is_empty() || form.contains("[[") || id.contains("]]") {
            return None;
        }
        let entity = EntityId::parse(id).ok()?;
        let start = u32::try_from(out.len()).ok()?;
        out.push_str(form);
        names.push(NameSpan {
            entity,
            start,
            end: u32::try_from(out.len()).ok()?,
        });
        rest = tail;
    }
    out.push_str(rest);
    Some((out, names))
}

/// `encoder:<hex16>`, a digest of the model id and the prompt digest.
pub fn encoder_id(model: &str, prompt_digest: &str) -> EncoderId {
    let digest = fnv1a64(format!("weksa-encoder-v1:{model}:{prompt_digest}").as_bytes());
    EncoderId::parse(&format!("encoder:{}", hex16(digest))).expect("an encoder id is well formed")
}

/// `packet:<hex16>`, a digest of the item, the inventory revision and the encoder.
pub fn packet_id(item: &CorpusItemId, rev: u32, encoder: &EncoderId) -> PacketId {
    let digest = fnv1a64(format!("weksa-packet-v1:{item}:{rev}:{encoder}").as_bytes());
    PacketId::parse(&format!("packet:{}", hex16(digest))).expect("a packet id is well formed")
}

#[cfg(test)]
mod mutants;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daodejing::packets;

    fn item() -> CorpusItemId {
        CorpusItemId::parse("item:daodejing:1").unwrap()
    }

    #[test]
    fn draft_class_per_real_serde_error() {
        // Each class is fed by the error serde_json raises for a real answer.
        let class = |answer: &str| PacketDraft::parse(answer).unwrap_err().class;
        assert_eq!(class("not json"), DraftClass::Syntax);
        assert_eq!(class("{\"referents\": ["), DraftClass::Eof);
        assert_eq!(class("{}"), DraftClass::MissingField);
        assert_eq!(class("{\"bogus\": 1}"), DraftClass::UnknownField);
        assert_eq!(class("{\"confidence\": \"x\"}"), DraftClass::InvalidType);
        assert_eq!(class("{\"confidence\": -1}"), DraftClass::InvalidValue);
        let bad_id = r#"{"referents": [{"id": "entity:x"}]}"#;
        assert_eq!(class(bad_id), DraftClass::BadForm);
        // A data error opening with words this reader does not know is Other.
        assert_eq!(
            DraftClass::of_data("something serde never said"),
            DraftClass::Other
        );
        assert_eq!(DraftClass::of_data(""), DraftClass::Other);
        // None keeps a token of the answer.
        let secret = r#"{"confidence": "canary-token"}"#;
        assert!(!format!("{:?}", PacketDraft::parse(secret).unwrap_err()).contains("canary"));
        // Other is fed by real serde errors the reader names no class for.
        let dup = r#"{"confidence": 1, "confidence": 1}"#;
        assert_eq!(class(dup), DraftClass::Other);
        let variant = r#"{"stance": {"features": [{"feature": "feature:9", "value": {"Bogus": 1}}]}}"#;
        assert_eq!(class(variant), DraftClass::Other);
        // Each code is its literal, exactly: a round record keeps these strings.
        let pinned = [
            (DraftClass::Syntax, "syntax"),
            (DraftClass::Eof, "eof"),
            (DraftClass::MissingField, "missing-field"),
            (DraftClass::UnknownField, "unknown-field"),
            (DraftClass::InvalidType, "invalid-type"),
            (DraftClass::InvalidValue, "invalid-value"),
            (DraftClass::BadForm, "bad-form"),
            (DraftClass::Other, "other"),
        ];
        for (c, lit) in pinned {
            assert_eq!(c.code(), lit);
        }
    }

    #[test]
    fn ids_are_pinned_digests_of_their_inputs() {
        let encoder = encoder_id("fake-model", "abcd");
        assert_eq!(encoder.as_str(), "encoder:4d35f2befb3108ec");
        let encoder = EncoderId::parse("encoder:0123456789abcdef").unwrap();
        assert_eq!(
            packet_id(&item(), 3, &encoder).as_str(),
            "packet:888ba17103e21a9d"
        );
        // Each input moves the id.
        assert_ne!(
            encoder_id("other", "abcd"),
            encoder_id("fake-model", "abcd")
        );
        assert_ne!(
            encoder_id("fake-model", "abce"),
            encoder_id("fake-model", "abcd")
        );
        let other = CorpusItemId::parse("item:daodejing:2").unwrap();
        let base = packet_id(&item(), 3, &encoder);
        assert_ne!(packet_id(&other, 3, &encoder), base);
        assert_ne!(packet_id(&item(), 4, &encoder), base);
        assert_ne!(packet_id(&item(), 3, &encoder_id("m", "d")), base);
    }

    #[test]
    fn the_encoder_step_sets_everything_the_model_does_not_write() {
        // A source packet with every non-meaning field set to something loud.
        let mut source = packets().remove(0);
        source.trace.notes = vec!["a note a model must not hand on".into()];
        source.constraints.deterministic = true;
        source.extensions = vec![crate::packet::Extension {
            key: "extension:1".parse().unwrap(),
            value: crate::packet::FeatValue::Flag(true),
        }];
        let draft = PacketDraft::of(&source);
        let encoder = encoder_id("m", "d");
        let made = draft.clone().into_packet(&item(), 9, &encoder);
        assert_eq!(made.id, packet_id(&item(), 9, &encoder));
        assert_eq!(made.schema_version, SCHEMA_VERSION);
        assert_eq!(made.inventory_rev, 9);
        assert_eq!(made.kind, PacketKind::CorpusExample);
        assert_eq!(made.provenance.source, ProvenanceSource::ModelDecomposition);
        assert_eq!(made.provenance.corpus_item, item());
        assert_eq!(made.provenance.encoder, encoder);
        assert_eq!(made.provenance.confidence, source.provenance.confidence);
        assert_eq!(made.constraints, Constraints::default());
        assert!(made.extensions.is_empty());
        assert_eq!(made.trace, Trace::default());
        // The meaning is the draft's, whole.
        assert_eq!(PacketDraft::of(&made), draft);
    }

    #[test]
    fn coordinated_fillers_admit_as_one_group_referent() {
        use crate::store::InventoryStore;
        use crate::validate::{Refusal, validate};
        // Item original-ja:19 as measure-ja-2 recorded it: two patients, one role twice.
        const REFUSED: &str = r#"{"confidence":720,"context":{"setting":null,"medium":null,"participants":["n1"],"audience":["n1"]},"discourse":{"speech_act":"speech_act:3","addressee":null,"addressee_consent":null,"features":[]},"referents":[{"id":"n1","kind":"referent_kind:1","concept":"concept:28","entity":"entity:keeper","features":[]},{"id":"n2","kind":"referent_kind:1","concept":"concept:31","entity":null,"features":[]},{"id":"n3","kind":"referent_kind:5","concept":"concept:123","entity":null,"features":[]},{"id":"n4","kind":"referent_kind:5","concept":"concept:123","entity":null,"features":[]}],"predications":[{"id":"n5","concept":"concept:26","frame":"frame:4","roles":[{"role":"role:1","filler":"n1"},{"role":"role:2","filler":"n3"},{"role":"role:2","filler":"n4"}],"features":[{"feature":"feature:1","value":{"Option":"positive"}},{"feature":"feature:2","value":{"Option":"present"}},{"feature":"feature:3","value":{"Option":"habitual"}}]},{"id":"n6","concept":"concept:196","frame":"frame:4","roles":[{"role":"role:1","filler":"n1"},{"role":"role:2","filler":"n5"}],"features":[{"feature":"feature:1","value":{"Option":"positive"}},{"feature":"feature:2","value":{"Option":"future"}},{"feature":"feature:4","value":{"Option":"desired"}}]}],"relations":[{"relation":"relation:11","from":"n2","to":"n3","features":[]},{"relation":"relation:11","from":"n1","to":"n4","features":[]}],"stance":{"features":[{"feature":"feature:18","value":{"Grade":600}},{"feature":"feature:11","value":{"Grade":450}},{"feature":"feature:17","value":{"Grade":250}}]}}"#;
        const PRESCRIBED: &str = r#"{"confidence":720,"context":{"setting":null,"medium":null,"participants":["n1"],"audience":["n1"]},"discourse":{"speech_act":"speech_act:3","addressee":null,"addressee_consent":null,"features":[]},"referents":[{"id":"n1","kind":"referent_kind:1","concept":"concept:28","entity":"entity:keeper","features":[]},{"id":"n2","kind":"referent_kind:1","concept":"concept:31","entity":null,"features":[]},{"id":"n3","kind":"referent_kind:5","concept":"concept:123","entity":null,"features":[]},{"id":"n4","kind":"referent_kind:5","concept":"concept:123","entity":null,"features":[]},{"id":"n7","kind":"referent_kind:2","concept":"concept:123","entity":null,"features":[]}],"predications":[{"id":"n5","concept":"concept:26","frame":"frame:4","roles":[{"role":"role:1","filler":"n1"},{"role":"role:2","filler":"n7"}],"features":[{"feature":"feature:1","value":{"Option":"positive"}},{"feature":"feature:2","value":{"Option":"present"}},{"feature":"feature:3","value":{"Option":"habitual"}}]},{"id":"n6","concept":"concept:196","frame":"frame:4","roles":[{"role":"role:1","filler":"n1"},{"role":"role:2","filler":"n5"}],"features":[{"feature":"feature:1","value":{"Option":"positive"}},{"feature":"feature:2","value":{"Option":"future"}},{"feature":"feature:4","value":{"Option":"desired"}}]}],"relations":[{"relation":"relation:11","from":"n2","to":"n3","features":[]},{"relation":"relation:11","from":"n1","to":"n4","features":[]},{"relation":"relation:3","from":"n3","to":"n7","features":[]},{"relation":"relation:3","from":"n4","to":"n7","features":[]}],"stance":{"features":[{"feature":"feature:18","value":{"Grade":600}},{"feature":"feature:11","value":{"Grade":450}},{"feature":"feature:17","value":{"Grade":250}}]}}"#;
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/original/inventory.cc");
        let inventory = InventoryStore::open(&path).unwrap().inventory().unwrap();
        let item = CorpusItemId::parse("item:original-ja:19").unwrap();
        let encoder = encoder_id("m", "d");
        let packet = |answer: &str| {
            PacketDraft::parse(answer)
                .unwrap()
                .into_packet(&item, 6, &encoder)
        };
        assert_eq!(
            validate(&packet(REFUSED), &inventory),
            Err(vec![Refusal::V11DuplicateRole {
                role: "role:2".parse().unwrap(),
                predication: "n5".parse().unwrap(),
            }])
        );
        assert_eq!(validate(&packet(PRESCRIBED), &inventory), Ok(()));
        // The prompt names the shape the packet above admits, by gloss.
        let gloss = |id: &str| inventory.entry(&id.parse().unwrap()).unwrap().gloss.clone();
        assert_eq!(gloss("referent_kind:2"), "group");
        assert_eq!(gloss("relation:3"), "member_of");
        let prompt = crate::context::ENCODE_INSTRUCTIONS;
        assert!(prompt.contains("member_of") && prompt.contains("group"));
    }

    #[test]
    fn lowering_draft_parse() {
        let draft = LoweringDraft::parse("\n {\"text\":\"a\"} \n").unwrap();
        assert_eq!(draft.text, "a");
        for bad in [
            r#"{"text":"a","trace":"b"}"#,
            r#"{}"#,
            r#"{"text":1}"#,
            "the sage acts",
            "",
        ] {
            let err = LoweringDraft::parse(bad).unwrap_err();
            // Position only: no answer bytes in either rendering.
            let shown = format!("{err} {err:?}");
            assert!(!shown.contains("sage") && !shown.contains("trace"), "{shown}");
        }
    }

    #[test]
    fn parse_reads_a_draft_and_trims() {
        let draft = PacketDraft::of(&packets()[0]);
        let json = serde_json::to_string(&draft).unwrap();
        assert_eq!(PacketDraft::parse(&format!("\n {json} \n")).unwrap(), draft);
        // The optional fields default.
        let minimal = r#"{"confidence":500,"discourse":{"speech_act":null,"addressee":null,"addressee_consent":null,"features":[]},"referents":[],"predications":[]}"#;
        let d = PacketDraft::parse(minimal).unwrap();
        assert_eq!(d.context, Context::default());
        assert!(d.relations.is_empty());
        assert_eq!(d.stance, Stance::default());
    }

    #[test]
    fn parse_refuses_what_a_model_must_not_write() {
        let draft = PacketDraft::of(&packets()[0]);
        let mut value = serde_json::to_value(&draft).unwrap();
        for field in [
            "id",
            "provenance",
            "trace",
            "inventory_rev",
            "kind",
            "extensions",
        ] {
            value[field] = serde_json::json!(null);
            assert!(PacketDraft::parse(&value.to_string()).is_err(), "{field}");
            value.as_object_mut().unwrap().remove(field);
        }
        value["confidence"] = serde_json::json!(1001);
        assert!(PacketDraft::parse(&value.to_string()).is_err());
        value.as_object_mut().unwrap().remove("confidence");
        assert!(
            PacketDraft::parse(&value.to_string()).is_err(),
            "confidence is required"
        );
    }

    #[test]
    fn a_draft_error_is_a_position_and_echoes_no_token() {
        let err = PacketDraft::parse("{\n  \"confidence\": CANARYTOKEN").unwrap_err();
        assert_eq!((err.line, err.column), (2, 17));
        assert_eq!(err.to_string(), "not a draft at line 2 column 17");
        assert_eq!(format!("{err:?}"), err.to_string());
        let err = PacketDraft::parse(r#"{"confidence":500,"CANARYKEY":1}"#).unwrap_err();
        for text in [err.to_string(), format!("{err:?}")] {
            assert!(!text.contains("CANARY"), "{text}");
        }
    }

    fn entry(id: &str, retired: Option<u32>) -> InventoryEntry {
        let id = InvId::parse(id).unwrap();
        InventoryEntry {
            id,
            gloss: "g".into(),
            value_kind: (id.family() == Family::Extension).then_some(ValueKind::Flag),
            applies_to: vec![],
            introduced_rev: 1,
            retired_rev: retired,
        }
    }

    fn add(family: Family, gloss: &str) -> EntryDraft {
        EntryDraft {
            family,
            gloss: gloss.into(),
            value_kind: None,
            applies_to: vec![],
        }
    }

    #[test]
    fn ids_are_the_stores() {
        // concept:1 live, concept:3 retired (the highest), frame:2 live; no role.
        let inv = Inventory::new(
            vec![
                entry("concept:1", None),
                entry("concept:3", Some(2)),
                entry("frame:2", None),
            ],
            vec![],
        );
        let draft = EditDraft {
            add: vec![
                add(Family::Concept, "first"),
                add(Family::Role, "role"),
                add(Family::Concept, "second"),
                add(Family::Frame, "frame"),
            ],
            retire: vec![
                InvId::parse("concept:1").unwrap(),
                InvId::parse("frame:2").unwrap(),
            ],
        };
        let (rev, entries) = draft.clone().into_revision(&inv, 6, MAX_EDIT_CHANGES).unwrap();
        let ids: Vec<String> = entries.iter().map(|e| e.id.to_string()).collect();
        // After the highest of the family, retired ones included, in draft order.
        assert_eq!(ids, ["concept:4", "role:1", "concept:5", "frame:3"]);
        assert_eq!((rev.n, rev.parent, rev.frozen), (7, Some(6), false));
        assert!(
            entries
                .iter()
                .all(|e| e.introduced_rev == 7 && e.retired_rev.is_none())
        );
        let glosses: Vec<&str> = entries.iter().map(|e| e.gloss.as_str()).collect();
        assert_eq!(glosses, ["first", "role", "second", "frame"]);
        // Adds in draft order, then the retires in draft order.
        let changes: Vec<String> = rev
            .changes
            .iter()
            .map(|c| match c {
                Change::Add(id) => format!("+{id}"),
                Change::Retire(id) => format!("-{id}"),
            })
            .collect();
        assert_eq!(
            changes,
            [
                "+concept:4",
                "+role:1",
                "+concept:5",
                "+frame:3",
                "-concept:1",
                "-frame:2"
            ]
        );
    }

    #[test]
    fn the_change_count_is_one_to_eight() {
        let inv = Inventory::new(vec![entry("concept:1", None)], vec![]);
        let adds = |n: usize| EditDraft {
            add: (0..n)
                .map(|i| add(Family::Concept, &format!("g{i}")))
                .collect(),
            retire: vec![],
        };
        assert!(adds(0).into_revision(&inv, 1, MAX_EDIT_CHANGES).is_none());
        assert!(adds(1).into_revision(&inv, 1, MAX_EDIT_CHANGES).is_some());
        assert_eq!(adds(8).into_revision(&inv, 1, MAX_EDIT_CHANGES).unwrap().1.len(), 8);
        assert!(adds(9).into_revision(&inv, 1, MAX_EDIT_CHANGES).is_none());
        // Adds and retires count together.
        let mixed = |a: usize, r: usize| EditDraft {
            add: (0..a).map(|_| add(Family::Concept, "g")).collect(),
            retire: vec![InvId::parse("concept:1").unwrap(); r],
        };
        assert!(mixed(4, 4).into_revision(&inv, 1, MAX_EDIT_CHANGES).is_some());
        assert!(mixed(4, 5).into_revision(&inv, 1, MAX_EDIT_CHANGES).is_none());
        assert!(mixed(0, 9).into_revision(&inv, 1, MAX_EDIT_CHANGES).is_none());
        // A repeated retire is the door's to refuse, not this fn's.
        assert!(mixed(0, 2).into_revision(&inv, 1, MAX_EDIT_CHANGES).is_some());
        // The cap is the caller's.
        let (_, entries) = adds(200).into_revision(&inv, 1, 256).unwrap();
        assert_eq!(entries.first().unwrap().id.to_string(), "concept:2");
        assert_eq!(entries.last().unwrap().id.to_string(), "concept:201");
        assert!(adds(257).into_revision(&inv, 1, 256).is_none());
    }

    #[test]
    fn an_edit_draft_reads_what_an_editor_may_write() {
        let d = EditDraft::parse(
            r#" {"add":[{"family":"Feature","gloss":"g","value_kind":{"Options":["a","b"]},"applies_to":["Referent"]}],"retire":["concept:2"]} "#,
        )
        .unwrap();
        assert_eq!(d.add[0].family, Family::Feature);
        assert_eq!(d.add[0].applies_to, [Slot::Referent]);
        assert_eq!(d.retire, [InvId::parse("concept:2").unwrap()]);
        // Both lists default; value_kind and applies_to default.
        let d = EditDraft::parse(r#"{"add":[{"family":"Concept","gloss":"g"}]}"#).unwrap();
        assert_eq!(d.add[0].value_kind, None);
        assert!(d.add[0].applies_to.is_empty() && d.retire.is_empty());
        assert_eq!(EditDraft::parse("{}").unwrap().add.len(), 0);
    }

    #[test]
    fn an_edit_draft_refuses_what_the_store_writes() {
        // An id, a revision number or a retired mark is the store's to write.
        for field in ["id", "introduced_rev", "retired_rev", "n", "parent"] {
            let json = format!(r#"{{"add":[{{"family":"Concept","gloss":"g","{field}":1}}]}}"#);
            assert!(EditDraft::parse(&json).is_err(), "{field} in an entry");
        }
        for field in ["revision", "frozen", "changes"] {
            let json = format!(r#"{{"add":[],"{field}":1}}"#);
            assert!(EditDraft::parse(&json).is_err(), "{field} in an edit");
        }
        // A family or a retire id that is not one.
        assert!(EditDraft::parse(r#"{"add":[{"family":"Nope","gloss":"g"}]}"#).is_err());
        assert!(EditDraft::parse(r#"{"retire":["concept:0"]}"#).is_err());
        assert!(EditDraft::parse(r#"{"retire":[1]}"#).is_err());
        // Not JSON, not an object, an error that names no token.
        let err = EditDraft::parse("{\n  \"add\": CANARYTOKEN").unwrap_err();
        assert_eq!((err.line, err.column), (2, 10));
        for text in [err.to_string(), format!("{err:?}")] {
            assert!(!text.contains("CANARY"), "{text}");
        }
        assert!(EditDraft::parse("3").is_err() && EditDraft::parse("\"x\"").is_err());
    }

    #[test]
    fn ontology_draft_refuses_unknown_fields_and_numbers_from_first_n() {
        for bad in [
            r#"{"entries": [], "extra": 1}"#,
            r#"{"entries": [{"layer": "Lexicon", "choice": "c", "language": "ja-JP"}]}"#,
            "```json\n{\"entries\": []}\n```",
            "[]",
        ] {
            assert!(OntologyDraft::parse(bad).is_err(), "{bad}");
        }
        let draft = OntologyDraft::parse(
            r#"{"entries": [{"layer": "Grammar", "choice": "a"}, {"layer": "Lexicon", "choice": "b", "examples": ["x"]}]}"#,
        )
        .unwrap();
        let by = Annotator::Operator;
        let entries = draft.into_entries("ja-JP", 3, 5, &by);
        assert_eq!(
            entries.iter().map(|e| (e.language.as_str(), e.n, e.inventory_rev)).collect::<Vec<_>>(),
            [("ja-JP", 5, 3), ("ja-JP", 6, 3)]
        );
        assert!(entries.iter().all(|e| e.annotator == by));
        assert_eq!(entries[1].examples, ["x"]);
    }

    const WORK: &str = r#"{"slug":"lantern-wager","language":"en-US","admitted_on":"2026-11-13",
        "source":{"PublicDomain":{"work":"The Lantern Wager","author":"A. Invented","edition_url":"https://example.invalid/lantern","licence":"public domain","jurisdiction":"US","translator":"none","edition_year":1890}},
        "cast":[{"entity":"entity:orsa","forms":["Orsa"]},{"entity":"entity:tamsin","forms":["Tamsin","Tam"]},{"entity":"entity:wexmoor","forms":["Wexmoor"]}],
        "lines":[{"group":"s1","speaker":"entity:orsa","text":"Tamsin, bring the lantern."},
                 {"group":"s1","speaker":"entity:tamsin","text":"Orsa, I will not, said Tam."},
                 {"group":"s2","speaker":"entity:wexmoor","text":"Quiet, both of you."}]}"#;

    fn marked(line: u32, text: &str) -> MarkedLine {
        MarkedLine { line, marked: text.into() }
    }

    fn model_annotator() -> Annotator {
        Annotator::Model {
            family: crate::corpus::ModelFamily::Anthropic,
            generator: "g".into(),
        }
    }

    fn import(
        work: &str,
        answer: &[MarkedLine],
    ) -> Result<(Vec<Entity>, Vec<CorpusItem>), ImportRefusal> {
        WorkDraft::parse(work).unwrap().into_import(answer, &model_annotator())
    }

    #[test]
    fn a_work_imports_its_cast_and_marked_lines() {
        let answer = [
            marked(1, "[[entity:tamsin|Tamsin]], bring the lantern."),
            marked(2, "[[entity:orsa|Orsa]], I will not, said [[entity:tamsin|Tam]]."),
        ];
        let (entities, items) = import(WORK, &answer).unwrap();
        assert_eq!(entities.len(), 3);
        for e in &entities {
            assert!(matches!(&e.origin, EntityOrigin::PublicDomain { work } if work == "The Lantern Wager"));
            assert_eq!(e.forms[0].language, "en-US");
        }
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "item:lantern-wager:1");
        assert_eq!(items[1].id, "item:lantern-wager:2");
        assert_eq!(items[0].group, "lantern-wager-s1");
        let span = |e: &str, text: &str, form: &str| {
            let start = text.find(form).unwrap() as u32;
            NameSpan { entity: EntityId::parse(e).unwrap(), start, end: start + form.len() as u32 }
        };
        let t2 = &items[1].text;
        assert_eq!(items[0].names, [span("entity:tamsin", &items[0].text, "Tamsin")]);
        assert_eq!(
            items[1].names,
            [span("entity:orsa", t2, "Orsa"), span("entity:tamsin", t2, "Tam")]
        );
        // "will" is unmarked and unspanned; the line's speaker is the file's.
        assert_eq!(items[1].speaker, EntityId::parse("entity:tamsin").unwrap());
        for i in &items {
            assert_eq!(i.names_by, Some(model_annotator()));
            assert!(i.prior_turns.is_empty() && i.supersedes.is_none() && !i.withdrawn);
        }
    }

    #[test]
    fn an_import_refuses_before_any_document() {
        let ok = marked(1, "Tamsin, bring the lantern.");
        let refused = |work: &str, answer: &[MarkedLine]| import(work, answer).unwrap_err();
        let cast_authored = WORK.replace(
            r#"{"PublicDomain":{"work":"The Lantern Wager","author":"A. Invented","edition_url":"https://example.invalid/lantern","licence":"public domain","jurisdiction":"US","translator":"none","edition_year":1890}}"#,
            r#""OperatorWritten""#,
        );
        assert_eq!(refused(&cast_authored, std::slice::from_ref(&ok)), ImportRefusal::NotPublicDomain);
        let repeated = WORK.replace(r#"{"entity":"entity:orsa","forms":["Orsa"]}"#, r#"{"entity":"entity:orsa","forms":["Orsa"]},{"entity":"entity:orsa","forms":["O"]}"#);
        assert_eq!(refused(&repeated, std::slice::from_ref(&ok)), ImportRefusal::CastRepeated);
        let stranger = WORK.replace(r#""speaker":"entity:orsa""#, r#""speaker":"entity:stranger""#);
        assert_eq!(refused(&stranger, std::slice::from_ref(&ok)), ImportRefusal::SpeakerNotInCast { line: 1 });
        assert_eq!(refused(WORK, &[marked(9, "x")]), ImportRefusal::NoSuchLine { line: 9 });
        assert_eq!(refused(WORK, &[marked(0, "x")]), ImportRefusal::NoSuchLine { line: 0 });
        assert_eq!(
            refused(WORK, &[ok.clone(), ok.clone()]),
            ImportRefusal::LineMarkedTwice { line: 1 }
        );
        for bad in [
            "[[entity:tamsin|Tamsin, bring the lantern.",
            "[[entity:tamsin Tamsin]], bring the lantern.",
            "[[tamsin|Tamsin]], bring the lantern.",
            "[[entity:tamsin|]]Tamsin, bring the lantern.",
            "Tamsin, bring the lantern",
        ] {
            assert_eq!(refused(WORK, &[marked(1, bad)]), ImportRefusal::Markup { line: 1 }, "{bad}");
        }
        assert_eq!(
            refused(WORK, &[marked(1, "[[entity:stranger|Tamsin]], bring the lantern.")]),
            ImportRefusal::NotInCast { line: 1 }
        );
        let group = WORK.replace(r#""group":"s1""#, r#""group":"S 1""#);
        assert_eq!(refused(&group, &[ok]), ImportRefusal::Group { line: 1 });
        let bracketed = WORK.replace("Tamsin, bring the lantern.", "Tamsin [[bring]] the lantern.");
        assert_eq!(
            refused(&bracketed, &[marked(1, "[[entity:tamsin|Tamsin]] [[entity:orsa|bring]] the lantern.")]),
            ImportRefusal::Markup { line: 1 }
        );
        // The refusal text names the kind and the line, no value.
        assert_eq!(
            ImportRefusal::Markup { line: 1 }.to_string(),
            "the marked text of line 1 is not its text with markup"
        );
    }

    #[test]
    fn markup_offsets_are_bytes_of_the_nfc_text() {
        let work = WORK
            .replace("Tamsin, bring the lantern.", "Caf\u{e9} Zo\u{eb}")
            .replace(r#""forms":["Orsa"]"#, r#""forms":["Zoe\u0308"]"#);
        let decomposed = "Cafe\u{301} [[entity:orsa|Zoe\u{308}]]";
        let (entities, items) = import(&work, &[marked(1, decomposed)]).unwrap();
        assert_eq!(entities[0].forms[0].forms, ["Zo\u{eb}"]);
        let item = &items[0];
        assert_eq!(item.text, "Caf\u{e9} Zo\u{eb}");
        let n = &item.names[0];
        assert_eq!(&item.text[n.start as usize..n.end as usize], "Zo\u{eb}");
        assert_eq!(n.start, 6);
    }

    #[test]
    fn a_stray_close_bracket_and_a_scattered_duplicate_answer_are_refused() {
        // A text holding only `]]` is not a marked line.
        let close = WORK.replace("Quiet, both of you.", "Quiet ]] both of you.");
        assert_eq!(
            import(&close, &[marked(3, "Quiet ]] both of you.")]).unwrap_err(),
            ImportRefusal::Markup { line: 3 }
        );
        // The answer's order does not matter, and a duplicate need not be adjacent.
        let (_, items) = import(WORK, &[marked(3, "Quiet, both of you."), marked(1, "Tamsin, bring the lantern.")]).unwrap();
        assert_eq!(items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), ["item:lantern-wager:1", "item:lantern-wager:3"]);
        assert_eq!(
            import(
                WORK,
                &[marked(1, "Tamsin, bring the lantern."), marked(3, "Quiet, both of you."), marked(1, "Tamsin, bring the lantern.")]
            )
            .unwrap_err(),
            ImportRefusal::LineMarkedTwice { line: 1 }
        );
    }
}

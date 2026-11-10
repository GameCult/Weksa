//! The encoder step: what a model's answer becomes. This is the one place JSON
//! is read (the xenos boundary): a model answers in JSON, `PacketDraft::parse`
//! reads it, and nothing is stored as JSON. The model writes meaning only;
//! `into_packet` sets the id, schema version, inventory revision, kind,
//! provenance, constraints and extensions, and an empty trace, so a model that
//! saw a Validation item cannot hand its words to whoever reads packets.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::corpus::CorpusItemId;
use crate::packet::{
    Constraints, Context, Discourse, EncoderId, Grade, Packet, PacketId, PacketKind, Predication,
    Provenance, ProvenanceSource, Referent, Relation, SCHEMA_VERSION, Stance, Trace,
};
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
}

impl fmt::Display for DraftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "not a packet draft at line {} column {}",
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

impl PacketDraft {
    pub fn parse(answer: &str) -> Result<Self, DraftError> {
        serde_json::from_str(answer.trim()).map_err(|e| DraftError {
            line: e.line(),
            column: e.column(),
        })
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
mod tests {
    use super::*;
    use crate::daodejing::packets;

    fn item() -> CorpusItemId {
        CorpusItemId::parse("item:daodejing:1").unwrap()
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
        let err = PacketDraft::parse(r#"{"confidence":500,"CANARYKEY":1}"#).unwrap_err();
        for text in [err.to_string(), format!("{err:?}")] {
            assert!(!text.contains("CANARY"), "{text}");
        }
    }
}

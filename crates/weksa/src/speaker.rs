//! The speaker-state documents: a speaker's stable axes and a scene's relations.
//! Both are append-only revisions under one key per revision and hold typed
//! feature values only, never text (ruling `leak-owned-by-flow-and-scorer-3`).
//! `SpeakerStateStore` is their only writer; `LoweringInput::with_scene` is the
//! only reader that reaches a lowering view.

use cultcache_rs::DatabaseEntry;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::corpus::ModelFamily;
use crate::packet::{EntityId, FeatureValue};

/// The most participants one scene holds.
pub const MAX_PARTICIPANTS: usize = 64;

/// Who annotated a document. A model's generator is an attestation string, held
/// to the same form as every provenance string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Annotator {
    Operator,
    Model {
        family: ModelFamily,
        generator: String,
    },
}

/// The speaker's stable axes. Key `speaker:<entity slug>@<n>`; a revision
/// appends and never edits.
#[derive(Clone, Debug, PartialEq, Eq, JsonSchema, DatabaseEntry)]
#[cultcache(type = "weksa.speaker")]
pub struct Speaker {
    #[cultcache(key = 0)]
    pub entity: EntityId,
    /// The revision, from 1.
    #[cultcache(key = 1)]
    pub n: u32,
    #[cultcache(key = 2)]
    pub inventory_rev: u32,
    /// Feature values of slot `Speaker`.
    #[cultcache(key = 3)]
    pub features: Vec<FeatureValue>,
    #[cultcache(key = 4)]
    pub annotator: Annotator,
}

/// The other party of a relation. `Unnamed` is an addressee the scene does not
/// name, such as the player or a crowd.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub enum Party {
    Named(EntityId),
    Unnamed,
}

/// The speaker's relation to one party: feature values of slot `Scene`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PairRelation {
    pub from: EntityId,
    pub to: Party,
    pub features: Vec<FeatureValue>,
}

/// A scene: its participants and the relations between them. Key
/// `scene:<group>@<n>`; a revision appends and never edits.
#[derive(Clone, Debug, PartialEq, Eq, JsonSchema, DatabaseEntry)]
#[cultcache(type = "weksa.scene")]
pub struct Scene {
    /// The corpus split group of the scene, a slug.
    #[cultcache(key = 0)]
    pub group: String,
    /// The revision, from 1.
    #[cultcache(key = 1)]
    pub n: u32,
    #[cultcache(key = 2)]
    pub inventory_rev: u32,
    #[cultcache(key = 3)]
    pub participants: Vec<EntityId>,
    #[cultcache(key = 4)]
    pub relations: Vec<PairRelation>,
    #[cultcache(key = 5)]
    pub annotator: Annotator,
}

impl Scene {
    /// The features of the relation from `speaker` to the named `addressee`, or
    /// to `Unnamed` when there is none; `None` when the scene holds no such
    /// relation (a relation with no features is `Some(&[])`). A named
    /// addressee the scene lacks never falls back to `Unnamed`.
    pub fn relation(
        &self,
        speaker: &EntityId,
        addressee: Option<&EntityId>,
    ) -> Option<&[FeatureValue]> {
        self.relations
            .iter()
            .find(|r| {
                &r.from == speaker
                    && match (&r.to, addressee) {
                        (Party::Named(a), Some(b)) => a == b,
                        (Party::Unnamed, None) => true,
                        _ => false,
                    }
            })
            .map(|r| r.features.as_slice())
    }
}

/// `speaker:<entity slug>@<n>`.
pub fn speaker_key(entity: &EntityId, n: u32) -> String {
    let slug = entity
        .as_str()
        .strip_prefix("entity:")
        .unwrap_or(entity.as_str());
    format!("speaker:{slug}@{n}")
}

/// `scene:<group>@<n>`.
pub fn scene_key(group: &str, n: u32) -> String {
    format!("scene:{group}@{n}")
}

//! The entity port and the ruled pronunciation of a packet's entities.
//!
//! Wording of a packet in a target language is the `ModelPort`'s answer to
//! `Prompt::lower` (`context.rs`); nothing here produces lowered text. This
//! module holds what the entity store owns and the lowering path reads: the
//! narrow [`EntityForms`] port, the ruled IPA of every entity a packet cites
//! ([`pronounce`]), and its segmentation into phones ([`phones`]). A referent
//! that cites an entity is read through the port; an entity the port does not
//! hold, one with no form or no ruled pronunciation in [`TARGET_LANGUAGE`], is
//! refused, because spelling is not a pronunciation and there is no
//! grapheme-to-phoneme guess to offer in its place.

use std::fmt;

use serde::Serialize;

use crate::entity::Entity;
use crate::packet::EntityId;
use crate::phones::{Phone, PhoneInventory, SegmentError};
use crate::store::EntityStore;
use crate::validate::LoweringInput;

/// The language tag of [`pronounce`] and [`phones`] only. The golf loop lowers
/// each item in its own language and does not read this. Spelled as an
/// entity's `LanguageForms::language` and a corpus item's language are.
pub const TARGET_LANGUAGE: &str = "en-US";

/// The lowerer's whole view of the entity store: one entity by id. Narrow so a
/// test or a loop can hand it a map and the lowerer cannot write, list or
/// search.
pub trait EntityForms {
    fn entity(&self, id: &EntityId) -> anyhow::Result<Option<Entity>>;
}

impl EntityForms for EntityStore {
    fn entity(&self, id: &EntityId) -> anyhow::Result<Option<Entity>> {
        self.get(id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LowerError {
    /// A referent cites an entity the entity port does not hold.
    UnknownEntity { entity: EntityId },
    /// The entity has no form in [`TARGET_LANGUAGE`].
    NoForm { entity: EntityId },
    /// IPA was requested for an entity whose [`TARGET_LANGUAGE`] citation form
    /// has no ruled pronunciation. Spelling is not a pronunciation.
    NoPronunciation { entity: EntityId },
    /// IPA was requested for a packet that cites no entity.
    NoEntityReferent,
    /// Phones were requested for an entity whose ruled IPA holds a symbol the
    /// language's phone inventory does not list (or the language has no
    /// inventory). No phone is guessed.
    UnknownPhone {
        entity: EntityId,
        at: usize,
        text: String,
    },
    /// Phones were requested for an entity whose ruled IPA has a stress mark
    /// with no vowel after it.
    DanglingStress { entity: EntityId, at: usize },
    /// The entity port failed to read.
    EntityRead { entity: EntityId, why: String },
    /// A lowering prompt was asked of a view whose scene was never read.
    SceneUnread,
    /// A referent cites the entity whose id is the lowering prompt's speaker
    /// token ([`crate::context::SPEAKER_TOKEN`]), which would read as the speaker.
    SpeakerTokenTaken,
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LowerError::UnknownEntity { .. } => "referent cites an entity the port does not hold",
            LowerError::NoForm { .. } => "entity has no form in the target language",
            LowerError::NoPronunciation { .. } => "entity has no ruled pronunciation",
            LowerError::NoEntityReferent => "packet cites no entity",
            LowerError::UnknownPhone { .. } => {
                "ruled pronunciation holds a symbol with no listed phone"
            }
            LowerError::DanglingStress { .. } => "stress mark with no vowel after it",
            LowerError::EntityRead { .. } => "entity port failed to read",
            LowerError::SceneUnread => "the view's scene was never read",
            LowerError::SpeakerTokenTaken => {
                "a referent cites the entity id the speaker token uses"
            }
        })
    }
}

impl std::error::Error for LowerError {}

/// The ruled IPA of every entity a packet cites, one per entity referent in
/// the packet's referent order, as stored: bare, stress-marked, nothing
/// derived. A rejected pronunciation is never read. An entity the port does not
/// hold, or one with no ruled pronunciation in [`TARGET_LANGUAGE`], is refused:
/// the lowerer has no grapheme-to-phoneme guess to offer in its place.
pub fn pronounce(
    input: &LoweringInput,
    entities: &dyn EntityForms,
) -> Result<Vec<String>, LowerError> {
    Ok(ruled(input, entities)?
        .into_iter()
        .map(|(_, ipa)| ipa)
        .collect())
}

/// One entity's ruled pronunciation as the phones line a sound consumer reads:
/// the language tag, the stored IPA it came from, and the segmented phones.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PhoneLine {
    pub language: &'static str,
    pub source: String,
    pub phones: Vec<Phone>,
}

/// The phones line of every entity a packet cites, in [`pronounce`] order, each
/// segmented by [`TARGET_LANGUAGE`]'s phone inventory. A symbol the inventory
/// does not list refuses the whole call.
pub fn phones(
    input: &LoweringInput,
    entities: &dyn EntityForms,
) -> Result<Vec<PhoneLine>, LowerError> {
    let inventory = PhoneInventory::for_language(TARGET_LANGUAGE);
    ruled(input, entities)?
        .into_iter()
        .map(|(entity, source)| {
            let unknown = |at, text| LowerError::UnknownPhone {
                entity: entity.clone(),
                at,
                text,
            };
            let inventory = inventory.ok_or_else(|| unknown(0, source.clone()))?;
            let phones = inventory.segment(&source).map_err(|e| match e {
                SegmentError::UnknownPhone { at, text } => unknown(at, text),
                SegmentError::DanglingStress { at } => LowerError::DanglingStress {
                    entity: entity.clone(),
                    at,
                },
            })?;
            Ok(PhoneLine {
                language: TARGET_LANGUAGE,
                source,
                phones,
            })
        })
        .collect()
}

/// `(entity, ruled IPA)` for each entity referent, in referent order.
fn ruled(
    input: &LoweringInput,
    entities: &dyn EntityForms,
) -> Result<Vec<(EntityId, String)>, LowerError> {
    let mut out = Vec::new();
    for id in input.referents().iter().filter_map(|r| r.entity.as_ref()) {
        let entity = entity(entities, id)?;
        let p = entity
            .pronunciation_in(TARGET_LANGUAGE)
            .ok_or_else(|| LowerError::NoPronunciation { entity: id.clone() })?;
        out.push((id.clone(), p.ipa.clone()));
    }
    if out.is_empty() {
        return Err(LowerError::NoEntityReferent);
    }
    Ok(out)
}

pub(crate) fn entity(entities: &dyn EntityForms, id: &EntityId) -> Result<Entity, LowerError> {
    entities
        .entity(id)
        .map_err(|e| LowerError::EntityRead {
            entity: id.clone(),
            why: e.to_string(),
        })?
        .ok_or_else(|| LowerError::UnknownEntity { entity: id.clone() })
}

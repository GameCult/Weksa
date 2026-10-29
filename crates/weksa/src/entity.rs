//! The entity store's documents. An entity is a named thing a packet may cite by
//! id. Its forms per language are data, citation form first, then inflected or
//! particle-bound forms; no rule here reads what a form means, only its shape.

use cultcache_rs::DatabaseEntry;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::packet::EntityId;

/// An entity's forms in one language, as the corpus item language tag spells it.
/// The citation form comes first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LanguageForms {
    pub language: String,
    /// 1 to 64 bytes each, no control or invisible format character, no edge
    /// whitespace, none repeated.
    pub forms: Vec<String>,
}

/// Where an entity was minted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum EntityOrigin {
    /// From a cast member's fixture: the persona and the fixture path the
    /// entity came from.
    Cast { persona: String, fixture: String },
    /// From a public-domain work.
    PublicDomain { work: String },
    /// Minted live by the named persona before it names the entity.
    Live { persona: String },
}

/// Key: the entity id. Forms only grow: see `EntityStore::put`.
#[derive(Clone, Debug, PartialEq, Eq, JsonSchema, DatabaseEntry)]
#[cultcache(type = "weksa.entity")]
pub struct Entity {
    #[cultcache(key = 0)]
    pub id: EntityId,
    #[cultcache(key = 1)]
    pub forms: Vec<LanguageForms>,
    #[cultcache(key = 2)]
    pub origin: EntityOrigin,
}

impl Entity {
    /// The forms stored for `language`, if any.
    pub fn forms_in(&self, language: &str) -> Option<&[String]> {
        self.forms
            .iter()
            .find(|f| f.language == language)
            .map(|f| f.forms.as_slice())
    }
}

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
    /// How the citation form (`forms[0]`) is said, when someone with the
    /// authority to rule on it has. Absent means unruled, never "spelled as
    /// read".
    #[serde(default)]
    pub pronunciation: Option<Pronunciation>,
}

/// A ruled pronunciation of an entity's citation form in one language.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Pronunciation {
    /// Broad IPA with stress marks, bare: no enclosing `/` or `[ ]`. 1 to 64
    /// bytes, NFC.
    pub ipa: String,
    /// Who ruled: the entity itself, a writer, an operator.
    pub ruled_by: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    /// Where the ruling was made: a room, a document, a run.
    pub source: String,
    /// Pronunciations that were considered and refused, same shape as `ipa`.
    /// Kept so nobody re-proposes one; never emitted.
    #[serde(default)]
    pub rejected: Vec<String>,
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

    /// The ruled pronunciation of the citation form in `language`, if any.
    pub fn pronunciation_in(&self, language: &str) -> Option<&Pronunciation> {
        self.forms
            .iter()
            .find(|f| f.language == language)
            .and_then(|f| f.pronunciation.as_ref())
    }
}

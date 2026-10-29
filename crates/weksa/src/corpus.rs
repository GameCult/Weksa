//! The English target corpus. An item's text is immutable; a correction is a new
//! item that supersedes the old, and an item is withdrawn, never deleted. The
//! source enum has no variant for a source whose terms forbid training.

use cultcache_rs::DatabaseEntry;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Turn {
    pub speaker: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Source {
    /// A line from a cast member's hand-authored fixture.
    CastAuthored {
        persona: String,
        fixture: String,
    },
    OperatorWritten,
    OperatorApproved {
        note: String,
    },
    PublicDomain {
        work: String,
        author: String,
        edition_url: String,
    },
    /// Labelled model output; never in the test split, never judged by its own family.
    ModelWritten {
        model_family: String,
        prompt_digest: Option<String>,
    },
}

/// Key: `item:<source-slug>:<n>`.
#[derive(Clone, Debug, PartialEq, Eq, JsonSchema, DatabaseEntry)]
#[cultcache(type = "weksa.corpus_item")]
pub struct CorpusItem {
    #[cultcache(key = 0)]
    pub id: String,
    #[cultcache(key = 1)]
    pub text: String,
    #[cultcache(key = 2)]
    pub language: String,
    #[cultcache(key = 3)]
    pub speaker: String,
    /// The unit the split hashes: a scene, a fixture, a work.
    #[cultcache(key = 4)]
    pub group: String,
    #[cultcache(key = 5)]
    pub prior_turns: Vec<Turn>,
    #[cultcache(key = 6)]
    pub source: Source,
    #[cultcache(key = 7)]
    pub admitted_on: String,
    /// Key of the item this one corrects.
    #[cultcache(key = 8)]
    pub supersedes: Option<String>,
    #[cultcache(key = 9)]
    pub withdrawn: bool,
}

/// `item:<slug>:<n>`.
pub fn item_key(slug: &str, n: u32) -> String {
    format!("item:{slug}:{n}")
}

/// The slug of an item key, if it is `item:<slug>:<n>`.
pub fn key_slug(key: &str) -> Option<&str> {
    let rest = key.strip_prefix("item:")?;
    let (slug, n) = rest.rsplit_once(':')?;
    n.parse::<u32>().ok()?;
    (!slug.is_empty()).then_some(slug)
}

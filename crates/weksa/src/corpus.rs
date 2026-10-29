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
    /// `family` is `Unrecorded` when nothing records one. `generator` is the
    /// string the fixture or run wrote, kept verbatim.
    ModelWritten {
        family: ModelFamily,
        generator: Option<String>,
        prompt_digest: Option<String>,
    },
}

/// The family of the model that wrote a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ModelFamily {
    Anthropic,
    OpenAi,
    Google,
    Alibaba,
    Meta,
    Mistral,
    /// Nothing records which family wrote the line.
    Unrecorded,
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

/// The slug of an item key, if it is canonical `item:<slug>:<n>`: a slug of
/// `[a-z0-9]` and inner `-` (at most 48 bytes, no edge `-`), and `n` a decimal
/// of at least 1 without sign or leading zero that fits `u32`.
pub fn key_slug(key: &str) -> Option<&str> {
    let rest = key.strip_prefix("item:")?;
    let (slug, n) = rest.rsplit_once(':')?;
    let canonical_n =
        n.bytes().all(|b| b.is_ascii_digit()) && !n.starts_with('0') && n.parse::<u32>().is_ok();
    let b = slug.as_bytes();
    let slug_ok = (1..=48).contains(&b.len())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        && b[0] != b'-'
        && b[b.len() - 1] != b'-';
    (canonical_n && slug_ok).then_some(slug)
}

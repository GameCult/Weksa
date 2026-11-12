//! The language ontology: per target language, the choices its culture makes
//! about word, grammar, discourse, sound and script for a speaker in a stance
//! (ruling `persona-drives-ontology`). An entry says when it applies (feature
//! conditions over the lowering view's stance, discourse and scene relation) and what it
//! chooses. `select` is the only reader that decides which entries a lowering
//! prompt carries, and it runs only inside `Prompt::lower`;
//! `OntologyStore::admit` is the only writer. Entries are immutable.

use cultcache_rs::DatabaseEntry;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::inventory::{InvId, Slot};
use crate::packet::{FeatValue, FeatureValue, Grade};
use crate::speaker::Annotator;
use crate::validate::LoweringInput;

pub const MAX_CONDITIONS: usize = 4;
pub const MAX_EXAMPLES: usize = 3;
pub const MAX_CHOICE_BYTES: usize = 200;
pub const MAX_EXAMPLE_BYTES: usize = 120;
pub const MAX_ENTRIES_PER_LANGUAGE: usize = 64;

/// Which part of the language an entry shapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Layer {
    Lexicon,
    Grammar,
    Discourse,
    Phonology,
    Orthography,
}

impl Layer {
    pub fn name(self) -> &'static str {
        match self {
            Layer::Lexicon => "lexicon",
            Layer::Grammar => "grammar",
            Layer::Discourse => "discourse",
            Layer::Phonology => "phonology",
            Layer::Orthography => "orthography",
        }
    }
}

/// What a feature's value must be for a condition to hold.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Test {
    AtLeast(Grade),
    AtMost(Grade),
    Is(FeatValue),
    /// Holds when the feature is absent or a grade at most `g`: a bound that
    /// costs nothing where the feature is unrated. `AtMost` still fails on an
    /// absent feature.
    AtMostOrUnset(Grade),
}

/// One feature of one slot of the view, tested. `Stance`, `Discourse` and
/// `Scene` reach a lowering view; `Speaker` does not yet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Condition {
    pub slot: Slot,
    pub feature: InvId,
    pub test: Test,
}

impl Condition {
    /// Whether this condition holds over a slot's feature values: the one
    /// evaluator `select` and the corpus tests share.
    pub fn holds(&self, features: &[FeatureValue]) -> bool {
        let Some(found) = features.iter().find(|f| f.feature == self.feature) else {
            return matches!(self.test, Test::AtMostOrUnset(_));
        };
        match (&self.test, &found.value) {
            (Test::AtLeast(g), FeatValue::Grade(v)) => v >= g,
            (Test::AtMost(g) | Test::AtMostOrUnset(g), FeatValue::Grade(v)) => v <= g,
            (Test::Is(want), v) => want == v,
            _ => false,
        }
    }

    /// The feature value this condition names, as the inventory check reads it.
    pub(crate) fn as_value(&self) -> FeatureValue {
        FeatureValue {
            feature: self.feature,
            value: match &self.test {
                Test::AtLeast(g) | Test::AtMost(g) | Test::AtMostOrUnset(g) => FeatValue::Grade(*g),
                Test::Is(v) => v.clone(),
            },
        }
    }
}

/// One choice of one language. Key `onto:<language>:<n>`.
///
/// The store judges the form of an entry's text (bounds, normalization, no
/// control or format character) and never its meaning: no door judges text
/// (ruling `leak-owned-by-flow-and-scorer-3`). No Train-only ontology mint
/// exists yet, so a seed or a drafted entry is checked against held-out text by
/// nothing today; follow-up `leak-diagnostic-in-round` owns that diagnostic.
#[derive(Clone, Debug, PartialEq, Eq, JsonSchema, DatabaseEntry)]
#[cultcache(type = "weksa.ontology_entry")]
pub struct OntologyEntry {
    #[cultcache(key = 0)]
    pub language: String,
    /// The entry's number within its language, from 1.
    #[cultcache(key = 1)]
    pub n: u32,
    /// The revision, of the inventory the entry was admitted through, at which the
    /// store read its conditions: provenance only; a run reads the conditions'
    /// features at its own head.
    #[cultcache(key = 2)]
    pub inventory_rev: u32,
    #[cultcache(key = 3)]
    pub layer: Layer,
    /// Every condition must hold; none means the entry always applies.
    #[cultcache(key = 4)]
    pub when: Vec<Condition>,
    #[cultcache(key = 5)]
    pub choice: String,
    #[cultcache(key = 6)]
    pub examples: Vec<String>,
    #[cultcache(key = 7)]
    pub annotator: Annotator,
}

/// `onto:<language>:<n>`.
pub fn ontology_key(language: &str, n: u32) -> String {
    format!("onto:{language}:{n}")
}

impl OntologyEntry {
    pub fn key(&self) -> String {
        ontology_key(&self.language, self.n)
    }
}

/// Every entry of the run's ontology store, by language then number.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ontology {
    entries: Vec<OntologyEntry>,
}

impl Ontology {
    pub fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn of(mut entries: Vec<OntologyEntry>) -> Self {
        entries.sort_by(|a, b| (&a.language, a.n).cmp(&(&b.language, b.n)));
        Self { entries }
    }

    pub fn entries(&self) -> &[OntologyEntry] {
        &self.entries
    }
}

/// The entries of exactly `language` whose every condition holds against the
/// view's stance, discourse and scene-relation features (the scene's only when
/// `with_scene` selected a relation), in `n` order. An absent feature meets no
/// condition except `AtMostOrUnset`. Nothing else of the view is read.
pub fn select<'a>(
    o: &'a Ontology,
    language: &str,
    input: &LoweringInput,
) -> Vec<&'a OntologyEntry> {
    o.entries
        .iter()
        .filter(|e| e.language == language)
        .filter(|e| {
            e.when.iter().all(|c| match c.slot {
                Slot::Stance => c.holds(&input.stance().features),
                Slot::Discourse => c.holds(&input.discourse().features),
                Slot::Scene => c.holds(input.scene()),
                _ => false,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cond(test: Test) -> Condition {
        Condition { slot: Slot::Discourse, feature: InvId::parse("feature:7").unwrap(), test }
    }
    fn has(v: FeatValue) -> Vec<FeatureValue> {
        vec![FeatureValue { feature: InvId::parse("feature:7").unwrap(), value: v }]
    }
    fn grade(g: u16) -> Grade {
        Grade::new(g).unwrap()
    }

    #[test]
    fn at_most_or_unset_holds_when_absent_or_at_most_and_nothing_else() {
        let c = cond(Test::AtMostOrUnset(grade(799)));
        assert!(c.holds(&[]));
        assert!(c.holds(&has(FeatValue::Grade(grade(799)))));
        assert!(c.holds(&has(FeatValue::Grade(grade(0)))));
        assert!(!c.holds(&has(FeatValue::Grade(grade(800)))));
        assert!(!c.holds(&has(FeatValue::Flag(true))));
        // AtMost keeps its meaning: an absent feature fails it.
        assert!(!cond(Test::AtMost(grade(799))).holds(&[]));
    }
}

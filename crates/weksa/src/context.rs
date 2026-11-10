//! The one place prompt text is built from corpus text. Corpus text reaches a
//! model call only as a [`Prompt`], and a `Prompt` is made only by minting from
//! [`TrainItems`] or by encoding one item an [`EncoderOffer`] holds: Train-only
//! minting and per-item encoding (leak-owned-by-flow-and-scorer-3,
//! loop-context-builder-2). Instruction text is [`MINT_INSTRUCTIONS`] and
//! [`ENCODE_INSTRUCTIONS`], consts in this module, and nothing else: `mint` and
//! `encode` take none, so no run-time text can become instructions.

use crate::corpus::CorpusItem;
use crate::packet::EntityId;
use crate::split::{Checkpoint, Split, SplitScheme};
use crate::store::{CorpusStore, normalized};

/// The instructions a mint prompt starts with. golf-loop-en edits the wording
/// here.
pub const MINT_INSTRUCTIONS: &str = "Mint glosses from the lines below.";

/// The instructions an encode prompt starts with.
pub const ENCODE_INSTRUCTIONS: &str = "Encode the line below.";

/// The live Train items of a corpus, in key order. The only input minting
/// reads; it cannot be built from a slice, and no accessor returns text or an
/// item.
///
/// ```
/// let dir = tempfile::tempdir().unwrap();
/// let store = weksa::CorpusStore::open(&dir.path().join("c.cc")).unwrap();
/// let t = weksa::context::TrainItems::from_corpus(&store).unwrap();
/// assert!(t.is_empty());
/// ```
///
/// ```compile_fail
/// let t = weksa::context::TrainItems { items: vec![] };
/// ```
///
/// ```compile_fail
/// let dir = tempfile::tempdir().unwrap();
/// let store = weksa::CorpusStore::open(&dir.path().join("c.cc")).unwrap();
/// let t = weksa::context::TrainItems::from_corpus(&store).unwrap();
/// let _u = t.clone();
/// ```
///
/// ```compile_fail
/// fn s<T: serde::Serialize>(_: &T) {}
/// let dir = tempfile::tempdir().unwrap();
/// let store = weksa::CorpusStore::open(&dir.path().join("c.cc")).unwrap();
/// let t = weksa::context::TrainItems::from_corpus(&store).unwrap();
/// s(&t);
/// ```
pub struct TrainItems {
    items: Vec<CorpusItem>,
}

impl TrainItems {
    pub fn from_corpus(corpus: &CorpusStore) -> anyhow::Result<Self> {
        let items = corpus
            .all()?
            .into_iter()
            .filter(|item| SplitScheme::V1.live(item) == Some(Split::Train))
            .collect();
        Ok(Self { items })
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The text of the live Train item of `group` whose store text key equals
    /// `text`'s (store S6 makes it unique).
    fn held(&self, group: &str, text: &str) -> Option<&str> {
        let key = normalized(text);
        self.items
            .iter()
            .find(|item| item.group == group && normalized(&item.text) == key)
            .map(|item| item.text.as_str())
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.items.iter().map(|item| item.id.as_str())
    }
}

/// Why an item is not offered to the encoder.
#[derive(Debug, PartialEq, Eq)]
pub enum OfferRefusal {
    Withdrawn,
    /// A model-written item of a Test group is in no split.
    NoSplit,
    /// A Test item is offered only under a [`Checkpoint`].
    TestBeforeCheckpoint,
    /// An item is offered at most once under one [`Checkpoint`].
    OfferedAtCheckpoint,
}

/// One item the encoder may be shown, with the entities a packet for it may
/// cite. Built per item, so a call sees one item and nothing of its group.
pub struct EncoderOffer {
    item: CorpusItem,
    entities: Vec<EntityId>,
}

impl EncoderOffer {
    /// A round offers Train and Validation items.
    pub fn for_round(item: &CorpusItem) -> Result<Self, OfferRefusal> {
        match Self::gate(item)? {
            Split::Test => Err(OfferRefusal::TestBeforeCheckpoint),
            _ => Ok(Self::of(item)),
        }
    }

    /// A convergence checkpoint offers any live item, Test included, each item
    /// at most once under one `Checkpoint`. The gate runs first, so a refused
    /// item records no key.
    pub fn for_checkpoint(
        item: &CorpusItem,
        checkpoint: &mut Checkpoint,
    ) -> Result<Self, OfferRefusal> {
        Self::gate(item)?;
        if !checkpoint.first_offer(&item.id) {
            return Err(OfferRefusal::OfferedAtCheckpoint);
        }
        Ok(Self::of(item))
    }

    fn gate(item: &CorpusItem) -> Result<Split, OfferRefusal> {
        if item.withdrawn {
            return Err(OfferRefusal::Withdrawn);
        }
        SplitScheme::V1.live(item).ok_or(OfferRefusal::NoSplit)
    }

    fn of(item: &CorpusItem) -> Self {
        let mut entities: Vec<EntityId> = item.cited_entities().cloned().collect();
        entities.sort();
        entities.dedup();
        Self {
            item: item.clone(),
            entities,
        }
    }

    pub fn key(&self) -> &str {
        &self.item.id
    }

    /// The item's own cited entities, sorted, without repeats.
    pub fn entities(&self) -> &[EntityId] {
        &self.entities
    }
}

/// Text for a model call, and the keys of the items it holds. The only
/// constructors are [`Prompt::mint`] and [`Prompt::encode`].
///
/// ```
/// let dir = tempfile::tempdir().unwrap();
/// let store = weksa::CorpusStore::open(&dir.path().join("c.cc")).unwrap();
/// let t = weksa::context::TrainItems::from_corpus(&store).unwrap();
/// let p = weksa::context::Prompt::mint(&t);
/// assert_eq!(p.text(), weksa::context::MINT_INSTRUCTIONS);
/// ```
///
/// Instructions are not a parameter:
///
/// ```compile_fail
/// let dir = tempfile::tempdir().unwrap();
/// let store = weksa::CorpusStore::open(&dir.path().join("c.cc")).unwrap();
/// let t = weksa::context::TrainItems::from_corpus(&store).unwrap();
/// let p = weksa::context::Prompt::mint("x", &t);
/// ```
///
/// ```compile_fail
/// fn s<T: serde::Serialize>(_: &T) {}
/// let dir = tempfile::tempdir().unwrap();
/// let store = weksa::CorpusStore::open(&dir.path().join("c.cc")).unwrap();
/// let t = weksa::context::TrainItems::from_corpus(&store).unwrap();
/// let p = weksa::context::Prompt::mint(&t);
/// s(&p);
/// ```
pub struct Prompt {
    text: String,
    items: Vec<String>,
}

/// What a prompt prints of an item: its line, and its prior turns only as the
/// one owner allows. A prior turn is a copy of text, not a reference, so it can
/// outlive a withdrawal or quote a held-out line; what is printed is the text of
/// the live Train item of the item's group that holds it (never the turn's own
/// bytes), and the speaker alone otherwise. Mint prints no prior turns (`prior: None`).
fn block(item: &CorpusItem, prior: Option<&TrainItems>) -> String {
    let mut lines: Vec<String> = prior
        .into_iter()
        .flat_map(|train| {
            item.prior_turns
                .iter()
                .map(move |t| match train.held(&item.group, &t.text) {
                    Some(text) => format!("{}: {text}", t.speaker),
                    None => format!("{}: {ELIDED}", t.speaker),
                })
        })
        .collect();
    lines.push(format!("[{}] {}: {}", item.id, item.speaker, item.text));
    lines.join("\n")
}

/// Printed in place of a prior turn whose text no live Train item holds.
const ELIDED: &str = "[elided]";

impl Prompt {
    /// [`MINT_INSTRUCTIONS`], then every Train item in key order.
    pub fn mint(train: &TrainItems) -> Prompt {
        let mut parts = vec![MINT_INSTRUCTIONS.to_string()];
        parts.extend(train.items.iter().map(|item| block(item, None)));
        Prompt {
            text: parts.join("\n\n"),
            items: train.keys().map(str::to_string).collect(),
        }
    }

    /// [`ENCODE_INSTRUCTIONS`], the one offered item with its prior turns resolved
    /// against `train`, and the entities a packet for it may cite.
    pub fn encode(offer: &EncoderOffer, train: &TrainItems) -> Prompt {
        let ids: Vec<&str> = offer.entities.iter().map(EntityId::as_str).collect();
        Prompt {
            text: format!(
                "{ENCODE_INSTRUCTIONS}\n\n{}\n\nentities: {}",
                block(&offer.item, Some(train)),
                ids.join(", ")
            ),
            items: vec![offer.item.id.clone()],
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// The keys of the items the text holds; what a round record lists.
    pub fn item_keys(&self) -> &[String] {
        &self.items
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::split::fixture::{group_in, item};

    #[test]
    fn checkpoint_offer_gate() {
        // A fresh checkpoint per call: this test is the gate, not the once rule.
        let check =
            |i: &CorpusItem| EncoderOffer::for_checkpoint(i, &mut Checkpoint::for_test(1)).err();
        let train = item("item:a:1", "t", &group_in(Split::Train), false);
        let validation = item("item:b:1", "v", &group_in(Split::Validation), false);
        let test = item("item:c:1", "x", &group_in(Split::Test), false);
        let no_split = item("item:c:2", "m", &group_in(Split::Test), true);
        let mut withdrawn = item("item:a:2", "w", &group_in(Split::Train), false);
        withdrawn.withdrawn = true;
        assert_eq!(check(&train), None);
        assert_eq!(check(&validation), None);
        assert_eq!(check(&test), None);
        assert_eq!(check(&no_split), Some(OfferRefusal::NoSplit));
        assert_eq!(check(&withdrawn), Some(OfferRefusal::Withdrawn));
        // Withdrawn wins over NoSplit.
        let mut both = no_split.clone();
        both.withdrawn = true;
        assert_eq!(check(&both), Some(OfferRefusal::Withdrawn));
    }

    #[test]
    fn checkpoint_offers_each_item_once() {
        let mut cp = Checkpoint::for_test(1);
        let offer = |cp: &mut Checkpoint, i: &CorpusItem| EncoderOffer::for_checkpoint(i, cp).err();
        let a = item("item:c:1", "a", &group_in(Split::Test), false);
        let b = item("item:c:3", "b", &group_in(Split::Test), false);
        let v = item("item:b:1", "v", &group_in(Split::Validation), false);
        assert_eq!(offer(&mut cp, &a), None);
        assert_eq!(offer(&mut cp, &a), Some(OfferRefusal::OfferedAtCheckpoint));
        assert_eq!(offer(&mut cp, &b), None);
        assert_eq!(offer(&mut cp, &v), None);
        assert_eq!(offer(&mut cp, &v), Some(OfferRefusal::OfferedAtCheckpoint));

        // A refused item records no key: its refusal stays the gate's.
        let mut gone = item("item:c:2", "w", &group_in(Split::Test), false);
        gone.withdrawn = true;
        assert_eq!(offer(&mut cp, &gone), Some(OfferRefusal::Withdrawn));
        assert_eq!(offer(&mut cp, &gone), Some(OfferRefusal::Withdrawn));
        let mut live = gone.clone();
        live.withdrawn = false;
        assert_eq!(offer(&mut cp, &live), None, "the refusal recorded no key");

        // The checkpoint is spent by its reading; a new one offers afresh.
        let _ = crate::score::TestReadings::at_checkpoint(cp, &[], &[], &[]);
        let mut fresh = Checkpoint::for_test(2);
        assert_eq!(offer(&mut fresh, &a), None);
    }
}

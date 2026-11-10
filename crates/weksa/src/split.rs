//! The train/validation/test split is derived from an item's group and nothing
//! else. No stored assignment exists, so no writer can reassign one. An item is
//! read in its live split. The round path (round records, prompts, `aggregate`,
//! `leak_diagnostic`) carries no Test reading; a typed `TestReadings` and a Test
//! `EncoderOffer` need a Checkpoint, and only the lib can make one
//! (convergence-freeze adds the door). Operator tools (`weksa corpus dump`,
//! `weksa score`, `CorpusStore`) read corpus text of any split; that is not
//! gated here.

use std::collections::BTreeSet;

use crate::corpus::{CorpusItem, Source};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Split {
    Train,
    Validation,
    Test,
}

/// A scheme is a constant; a new scheme is a new variant, never a reassignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitScheme {
    V1,
}

pub const fn fnv1a64(bytes: &[u8]) -> u64 {
    fnv1a64_of(&[bytes])
}

/// FNV-1a 64 of the parts laid end to end.
pub const fn fnv1a64_of(parts: &[&[u8]]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut p = 0;
    while p < parts.len() {
        let bytes = parts[p];
        let mut i = 0;
        while i < bytes.len() {
            h ^= bytes[i] as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
            i += 1;
        }
        p += 1;
    }
    h
}

/// 16 lowercase hex digits, zero-padded: how a digest is written everywhere.
pub(crate) fn hex16(n: u64) -> String {
    format!("{n:016x}")
}

impl SplitScheme {
    /// 0..100, FNV-1a 64 of `weksa-split-v1:` + group, mod 100.
    pub fn bucket(self, group: &str) -> u64 {
        match self {
            SplitScheme::V1 => fnv1a64(format!("weksa-split-v1:{group}").as_bytes()) % 100,
        }
    }

    /// <40 Train, <80 Validation, else Test.
    pub fn of_bucket(self, bucket: u64) -> Split {
        match self {
            SplitScheme::V1 if bucket < 40 => Split::Train,
            SplitScheme::V1 if bucket < 80 => Split::Validation,
            SplitScheme::V1 => Split::Test,
        }
    }

    /// The split of an item: its group's, except that a model-written item of a
    /// test group is in no split. It is never moved to train: a model line of
    /// the test meaning would teach the test.
    pub fn split(self, item: &CorpusItem) -> Option<Split> {
        match self.of_bucket(self.bucket(&item.group)) {
            Split::Test if matches!(item.source, Source::ModelWritten { .. }) => None,
            split => Some(split),
        }
    }

    /// The split an item is read in; a withdrawn item is read in none.
    pub fn live(self, item: &CorpusItem) -> Option<Split> {
        if item.withdrawn {
            None
        } else {
            self.split(item)
        }
    }
}

/// Permission to read Test once, as typed `TestReadings` or as Test
/// `EncoderOffer`s. Only the weksa lib can make one: there is no public
/// constructor, so no code outside it can obtain those. This is not a seal on
/// corpus text: operator tools and `CorpusStore` can read any item, and the
/// guarded invariant is that the round path holds no Test reading. The one door
/// is added by
/// convergence-freeze, which builds a `Checkpoint` from the convergence window
/// over the run's persisted round records (convergence-test).
/// `TestReadings::at_checkpoint` consumes it, so one `Checkpoint` buys one Test
/// reading. One `Checkpoint` also offers each item at most once
/// (`EncoderOffer::for_checkpoint`), and an offer borrows its Checkpoint, so it
/// cannot outlive the reading that spends it.
///
/// ```
/// fn f(c: &weksa::split::Checkpoint) -> u32 { c.round() }
/// ```
///
/// No public constructor:
///
/// ```compile_fail
/// let _ = weksa::split::Checkpoint::declare(1);
/// ```
///
/// The test constructor is not exported (this doctest names it today; the
/// line-based tripwire in `tests/checkpoint_door.rs` catches only common
/// single-line forms of a new public constructor):
///
/// ```compile_fail
/// let _ = weksa::split::Checkpoint::for_test(1);
/// ```
///
/// ```compile_fail
/// let _ = weksa::split::Checkpoint { round: 1, offered: Default::default() };
/// ```
///
/// One reading per checkpoint:
///
/// ```
/// use weksa::score::TestReadings;
/// use weksa::split::Checkpoint;
/// fn f(c: Checkpoint) { let _ = TestReadings::at_checkpoint(c, &[], &[], &[]); }
/// ```
///
/// ```compile_fail
/// use weksa::score::TestReadings;
/// use weksa::split::Checkpoint;
/// fn f(c: Checkpoint) {
///     let _ = TestReadings::at_checkpoint(c, &[], &[], &[]);
///     let _ = TestReadings::at_checkpoint(c, &[], &[], &[]);
/// }
/// ```
///
/// ```compile_fail
/// fn f(c: weksa::split::Checkpoint) { let _ = c.clone(); }
/// ```
pub struct Checkpoint {
    round: u32,
    offered: BTreeSet<String>,
}

impl Checkpoint {
    #[cfg(test)]
    pub(crate) fn for_test(round: u32) -> Self {
        Self {
            round,
            offered: BTreeSet::new(),
        }
    }

    /// Records `key` as offered; false when it already was.
    pub(crate) fn first_offer(&mut self, key: &str) -> bool {
        self.offered.insert(key.to_string())
    }

    pub fn round(&self) -> u32 {
        self.round
    }
}

/// Item builders for the unit tests that live in `src`: the twins of
/// `tests/common`'s `group_in_bucket`, `item_in`, `cast` and `model`.
#[cfg(test)]
pub(crate) mod fixture {
    use super::{Split, SplitScheme};
    use crate::corpus::{CorpusItem, ModelFamily, Source};
    use crate::packet::EntityId;

    /// The first group `g<i>` whose V1 bucket is in `split`.
    pub(crate) fn group_in(split: Split) -> String {
        (0..100_000)
            .map(|i| format!("g{i}"))
            .find(|g| SplitScheme::V1.of_bucket(SplitScheme::V1.bucket(g)) == split)
            .unwrap()
    }

    pub(crate) fn item(key: &str, text: &str, group: &str, model_written: bool) -> CorpusItem {
        let source = if model_written {
            Source::ModelWritten {
                family: ModelFamily::Unrecorded,
                generator: None,
                prompt_digest: None,
            }
        } else {
            Source::CastAuthored {
                persona: "nibu".into(),
                fixture: "f.yaml".into(),
                author: "Ada Writer".into(),
            }
        };
        CorpusItem {
            id: key.into(),
            text: text.into(),
            language: "en-US".into(),
            speaker: EntityId::parse("entity:nibu").unwrap(),
            names: vec![],
            group: group.into(),
            prior_turns: vec![],
            source,
            admitted_on: "2026-10-29".into(),
            supersedes: None,
            withdrawn: false,
        }
    }
}

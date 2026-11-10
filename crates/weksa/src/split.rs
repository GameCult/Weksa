//! The train/validation/test split is derived from an item's group and nothing
//! else. No stored assignment exists, so no writer can reassign one. An item is
//! read in its live split; Test is read only under a Checkpoint.

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

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
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

/// Permission to read Test once. The loop declares one at a convergence
/// checkpoint (convergence-test); `TestReadings::at_checkpoint` consumes it, so
/// one declaration buys one Test reading, and `EncoderOffer::for_checkpoint`
/// borrows it first.
///
/// ```
/// use weksa::score::TestReadings;
/// use weksa::split::Checkpoint;
/// let c = Checkpoint::declare(3);
/// assert_eq!(c.round(), 3);
/// let _ = TestReadings::at_checkpoint(c, &[], &[], &[]);
/// ```
///
/// One declaration, one reading:
///
/// ```compile_fail
/// use weksa::score::TestReadings;
/// use weksa::split::Checkpoint;
/// let c = Checkpoint::declare(3);
/// let _ = TestReadings::at_checkpoint(c, &[], &[], &[]);
/// let _ = TestReadings::at_checkpoint(c, &[], &[], &[]);
/// ```
///
/// ```compile_fail
/// use weksa::split::Checkpoint;
/// let c = Checkpoint::declare(3);
/// let _again = c.clone();
/// ```
pub struct Checkpoint {
    round: u32,
}

impl Checkpoint {
    pub fn declare(round: u32) -> Self {
        Self { round }
    }

    pub fn round(&self) -> u32 {
        self.round
    }
}

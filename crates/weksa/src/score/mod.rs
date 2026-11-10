//! What a score counts: chrF and the meaning cosine of the whole lowered
//! output against the whole reference. Nothing is deleted, hidden or replaced
//! before either is read, and the scorer takes no input from a packet, so no
//! text the output carries can raise a score except by resembling the
//! reference. No door judges text; the diagnostics report. The round path
//! (`aggregate`, `leak_diagnostic`) reads Train and Validation; Test is read
//! only through `TestReadings::at_checkpoint` (convergence-test).

mod chrf;
mod diagnostics;
mod embed;
mod words;

use anyhow::{Result, ensure};

pub use chrf::{CHRF_VERSION, chrf};
pub use diagnostics::{OrderCoverage, PairChrf, SplitLeak, WorstItem, leak_diagnostic};
pub use embed::{Embedder, OllamaEmbedder, cosine};

use crate::corpus::CorpusItem;
use crate::split::{Checkpoint, Split, SplitScheme};

/// One reference/output pair; both are scored whole.
pub struct ScoreInput<'a> {
    pub reference: &'a str,
    pub output: &'a str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemScore {
    /// chrF of the output against the reference.
    pub chrf: f64,
    /// Embedding cosine of the output and the reference; `None` when either
    /// is empty after trimming.
    pub meaning: Option<f64>,
}

pub struct Scorer<E: Embedder> {
    embedder: E,
}

/// Every vector has one nonzero length and a nonzero norm.
fn check_vectors(vectors: &[Vec<f32>]) -> Result<()> {
    let dim = vectors.first().map_or(0, Vec::len);
    for vector in vectors {
        ensure!(
            dim > 0 && vector.len() == dim,
            "embedder returned vectors of length {} and {}",
            dim,
            vector.len()
        );
        let norm: f64 = vector.iter().map(|x| f64::from(*x).powi(2)).sum();
        ensure!(
            norm > 0.0 && norm.is_finite(),
            "embedder returned a zero or non-finite vector"
        );
    }
    Ok(())
}

impl<E: Embedder> Scorer<E> {
    pub fn new(embedder: E) -> Self {
        Self { embedder }
    }

    pub fn embedder(&self) -> &E {
        &self.embedder
    }

    /// Scores every input with one embed call; the caller chunks `inputs`.
    /// Errs when the embedder's vectors disagree in length or are empty or zero.
    pub fn score(&self, inputs: &[ScoreInput<'_>]) -> Result<Vec<ItemScore>> {
        let mut texts: Vec<&str> = Vec::new();
        let slots: Vec<Option<usize>> = inputs
            .iter()
            .map(|input| {
                if input.output.trim().is_empty() || input.reference.trim().is_empty() {
                    return None;
                }
                texts.push(input.reference);
                texts.push(input.output);
                Some(texts.len() - 2)
            })
            .collect();
        let vectors = self.embedder.embed(&texts)?;
        ensure!(
            vectors.len() == texts.len(),
            "embedder returned {} vectors for {} texts",
            vectors.len(),
            texts.len()
        );
        check_vectors(&vectors)?;
        Ok(inputs
            .iter()
            .zip(slots)
            .map(|(input, slot)| ItemScore {
                chrf: chrf(input.output, input.reference),
                meaning: slot.map(|i| cosine(&vectors[i], &vectors[i + 1])),
            })
            .collect())
    }
}

/// Mean scores of one derived split.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SplitMean {
    pub count: usize,
    /// `None` when the split has no items.
    pub chrf: Option<f64>,
    /// Mean over the items that have a meaning score; `None` when none do.
    pub meaning: Option<f64>,
}

/// The round readings a round record may hold. The scorer decides when Test
/// can be read, by type: `Aggregate` has no Test field.
///
/// ```
/// let agg = weksa::score::aggregate(&[]);
/// let _ = agg.validation;
/// ```
///
/// ```compile_fail
/// let agg = weksa::score::aggregate(&[]);
/// let _ = agg.test;
/// ```
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Aggregate {
    pub train: SplitMean,
    pub validation: SplitMean,
}

/// The mean of the scores of the items read in split `want`.
fn split_mean(scored: &[(&CorpusItem, ItemScore)], want: Split) -> SplitMean {
    let of: Vec<&ItemScore> = scored
        .iter()
        .filter(|(item, _)| SplitScheme::V1.live(item) == Some(want))
        .map(|(_, score)| score)
        .collect();
    let average = |values: Vec<f64>| {
        (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
    };
    SplitMean {
        count: of.len(),
        chrf: average(of.iter().map(|s| s.chrf).collect()),
        meaning: average(of.iter().filter_map(|s| s.meaning).collect()),
    }
}

/// Groups scores by the split each item is read in. Items read in no split
/// (withdrawn, or model-written in a Test group) are counted nowhere.
pub fn aggregate(scored: &[(&CorpusItem, ItemScore)]) -> Aggregate {
    Aggregate {
        train: split_mean(scored, Split::Train),
        validation: split_mean(scored, Split::Validation),
    }
}

/// The Test split's readings, scalars only: no item key, no minted string, no
/// pair. The checkpoint decision (convergence-test) needs numbers; a key beside
/// a minted string is the handle an edit step would act on. The only producer
/// is `at_checkpoint`, which consumes a `Checkpoint`.
///
/// ```
/// use weksa::score::TestReadings;
/// use weksa::split::Checkpoint;
/// fn s<T: serde::Serialize>(_: &T) {}
/// let t = TestReadings::at_checkpoint(Checkpoint::declare(1), &[], &[], &[]);
/// assert_eq!(t.count(), 0);
/// s(&0u8);
/// ```
///
/// ```compile_fail
/// use weksa::score::TestReadings;
/// use weksa::split::Checkpoint;
/// let t = TestReadings::at_checkpoint(Checkpoint::declare(1), &[], &[], &[]);
/// let _u = t.clone();
/// ```
///
/// ```compile_fail
/// use weksa::score::TestReadings;
/// use weksa::split::Checkpoint;
/// let t = TestReadings::at_checkpoint(Checkpoint::declare(1), &[], &[], &[]);
/// let _ = format!("{t:?}");
/// ```
///
/// ```compile_fail
/// use weksa::score::TestReadings;
/// use weksa::split::Checkpoint;
/// fn s<T: serde::Serialize>(_: &T) {}
/// let t = TestReadings::at_checkpoint(Checkpoint::declare(1), &[], &[], &[]);
/// s(&t);
/// ```
pub struct TestReadings {
    mean: SplitMean,
    leak: SplitLeak,
}

impl TestReadings {
    /// Reads the live Test items against the scores and the minted strings,
    /// by the same `split_leak` method the Validation reading uses. The
    /// checkpoint is spent.
    pub fn at_checkpoint(
        checkpoint: Checkpoint,
        scored: &[(&CorpusItem, ItemScore)],
        minted: &[&str],
        items: &[CorpusItem],
    ) -> TestReadings {
        let _spent = checkpoint;
        TestReadings {
            mean: split_mean(scored, Split::Test),
            leak: diagnostics::held_out(minted, items, Split::Test),
        }
    }

    /// Scored Test items.
    pub fn count(&self) -> usize {
        self.mean.count
    }

    pub fn chrf(&self) -> Option<f64> {
        self.mean.chrf
    }

    pub fn meaning(&self) -> Option<f64> {
        self.mean.meaning
    }

    /// Test items in the leak reading.
    pub fn items(&self) -> usize {
        self.leak.items
    }

    /// The highest chrF of any minted string against any Test item; the score
    /// only, never the pair.
    pub fn max_chrf(&self) -> Option<f64> {
        self.leak.max_chrf.as_ref().map(|p| p.chrf)
    }

    pub fn coverage_by_n(&self) -> &[OrderCoverage] {
        &self.leak.coverage_by_n
    }
}

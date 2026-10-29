//! What a score counts: chrF and the meaning cosine of the whole lowered
//! output against the whole reference. Nothing is deleted, hidden or replaced
//! before either is read, and the scorer takes no input from a packet, so no
//! text the output carries can raise a score except by resembling the
//! reference. No door judges text; the diagnostics report.

mod chrf;
mod diagnostics;
mod embed;
mod words;

use anyhow::{Result, ensure};

pub use chrf::{CHRF_VERSION, chrf};
pub use diagnostics::{
    LeakDiagnostic, OrderCoverage, PairChrf, SplitLeak, WorstItem, leak_diagnostic,
};
pub use embed::{Embedder, OllamaEmbedder, cosine};

use crate::corpus::CorpusItem;
use crate::split::{Split, SplitScheme};

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

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Aggregate {
    pub train: SplitMean,
    pub validation: SplitMean,
    pub test: SplitMean,
}

/// Groups scores by the item's derived split. Items in no split are counted
/// nowhere. The scorer only groups: the loop decides when Test is read.
pub fn aggregate(scored: &[(&CorpusItem, ItemScore)]) -> Aggregate {
    let mean = |want: Split| {
        let of: Vec<&ItemScore> = scored
            .iter()
            .filter(|(item, _)| SplitScheme::V1.split(item) == Some(want))
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
    };
    Aggregate {
        train: mean(Split::Train),
        validation: mean(Split::Validation),
        test: mean(Split::Test),
    }
}

//! Leak diagnostics: values reported beside scores, never refusals. Pooled
//! coverage at n >= 2 above zero beside single-word glosses, or a high pairwise
//! chrF, means a flow bug or a corpus duplicate across splits.

use std::collections::HashSet;

use super::chrf::chrf;
use super::words::words;
use crate::corpus::CorpusItem;
use crate::split::{Split, SplitScheme};

/// Word n-gram orders read, 1 to this.
const ORDERS: usize = 3;

/// The pair that reached the highest chrF.
#[derive(Clone, Debug, PartialEq)]
pub struct PairChrf {
    pub chrf: f64,
    pub minted: String,
    pub item: String,
}

/// One word-order's pooled novel coverage over a split's items.
#[derive(Clone, Debug, PartialEq)]
pub struct OrderCoverage {
    pub n: usize,
    /// Distinct n-grams of the split's items absent from Train, summed over items.
    pub novel: usize,
    /// Of those, the ones that occur in the minted text.
    pub covered: usize,
    /// `covered / novel`; `None` when `novel` is 0, never NaN.
    pub share: Option<f64>,
}

/// The item that read most at `n` >= 2.
#[derive(Clone, Debug, PartialEq)]
pub struct WorstItem {
    pub item: String,
    pub n: usize,
    pub covered: usize,
    pub novel: usize,
}

/// One held-out split's readings.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SplitLeak {
    /// Items of the split.
    pub items: usize,
    /// Max chrF of any minted string (hypothesis) against any item (reference).
    pub max_chrf: Option<PairChrf>,
    /// Pooled novel coverage for n in 1..=3, in order.
    pub coverage_by_n: Vec<OrderCoverage>,
    /// The item and order (n >= 2) with the largest covered count, then the
    /// larger share; the first item, then the smaller n, on ties. `None` when
    /// no item has a covered n-gram at n >= 2.
    pub worst: Option<WorstItem>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct LeakDiagnostic {
    pub validation: SplitLeak,
    pub test: SplitLeak,
}

/// The distinct word n-grams of a text for each order 1..=ORDERS, over its
/// normalised word segments.
fn grams(text: &str) -> Vec<HashSet<String>> {
    let keys = words(text);
    (1..=ORDERS)
        .map(|n| keys.windows(n).map(|w| w.join(" ")).collect())
        .collect()
}

fn union<'a>(texts: impl Iterator<Item = &'a str>) -> Vec<HashSet<String>> {
    let mut all = vec![HashSet::new(); ORDERS];
    for text in texts {
        for (into, from) in all.iter_mut().zip(grams(text)) {
            into.extend(from);
        }
    }
    all
}

fn split_leak(
    minted: &[&str],
    train: &[HashSet<String>],
    minted_grams: &[HashSet<String>],
    items: &[&CorpusItem],
) -> SplitLeak {
    let mut leak = SplitLeak {
        items: items.len(),
        ..SplitLeak::default()
    };
    let mut totals = [(0usize, 0usize); ORDERS];
    for item in items {
        for m in minted {
            let score = chrf(m, &item.text);
            if leak.max_chrf.as_ref().is_none_or(|best| score > best.chrf) {
                leak.max_chrf = Some(PairChrf {
                    chrf: score,
                    minted: (*m).to_string(),
                    item: item.id.clone(),
                });
            }
        }
        for (i, item_grams) in grams(&item.text).iter().enumerate() {
            let novel: Vec<&String> = item_grams
                .iter()
                .filter(|g| !train[i].contains(*g))
                .collect();
            let covered = novel
                .iter()
                .filter(|g| minted_grams[i].contains(**g))
                .count();
            totals[i].0 += novel.len();
            totals[i].1 += covered;
            if i == 0 || covered == 0 {
                continue;
            }
            let share = covered as f64 / novel.len() as f64;
            let better = leak.worst.as_ref().is_none_or(|w| {
                covered > w.covered
                    || (covered == w.covered && share > w.covered as f64 / w.novel as f64)
            });
            if better {
                leak.worst = Some(WorstItem {
                    item: item.id.clone(),
                    n: i + 1,
                    covered,
                    novel: novel.len(),
                });
            }
        }
    }
    leak.coverage_by_n = totals
        .iter()
        .enumerate()
        .map(|(i, (novel, covered))| OrderCoverage {
            n: i + 1,
            novel: *novel,
            covered: *covered,
            share: (*novel > 0).then(|| *covered as f64 / *novel as f64),
        })
        .collect();
    leak
}

/// Reads minted text (inventory glosses, option labels, later ontology text)
/// against the held-out items. Per held-out item and for n in 1..=3, its word
/// n-grams that occur in no Train item are its novel n-grams; coverage at each
/// n is the covered over the novel n-grams summed across the split's items
/// (unigrams over all minted strings, longer grams within one string). Honest
/// single-word glosses saturate n = 1, so the leak signature is pooled n >= 2
/// and the worst item at n >= 2. Train items are not measured; items in no
/// split are dropped.
pub fn leak_diagnostic(minted: &[&str], items: &[CorpusItem]) -> LeakDiagnostic {
    let of = |want: Split| -> Vec<&CorpusItem> {
        items
            .iter()
            .filter(|i| SplitScheme::V1.split(i) == Some(want))
            .collect()
    };
    let train = union(of(Split::Train).iter().map(|i| i.text.as_str()));
    let minted_grams = union(minted.iter().copied());
    LeakDiagnostic {
        validation: split_leak(minted, &train, &minted_grams, &of(Split::Validation)),
        test: split_leak(minted, &train, &minted_grams, &of(Split::Test)),
    }
}

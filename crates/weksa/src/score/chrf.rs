//! chrF as sacrebleu 2.4.3 computes it by default: character n-gram orders 1 to
//! 6, whitespace removed, case kept, beta 2.

use std::collections::HashMap;

/// The metric this module implements; a score stored beside it names its definition.
pub const CHRF_VERSION: &str = "chrF2|nc:6|nw:0|space:no|sacrebleu:2.4.3";

const ORDER: usize = 6;
const BETA_SQ: f64 = 4.0;

fn grams(chars: &[char], n: usize) -> HashMap<&[char], u32> {
    let mut counts = HashMap::new();
    for window in chars.windows(n) {
        *counts.entry(window).or_insert(0) += 1;
    }
    counts
}

/// Python `str.split`'s whitespace, which sacrebleu strips before gramming:
/// Unicode White_Space plus U+001C..=U+001F.
pub(super) fn is_split_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// chrF of `hypothesis` against `reference`, 0 to 100. Beta 2 weights recall,
/// so the arguments are not interchangeable. 0 when no order has n-grams on
/// both sides.
pub fn chrf(hypothesis: &str, reference: &str) -> f64 {
    let strip = |s: &str| {
        s.chars()
            .filter(|c| !is_split_space(*c))
            .collect::<Vec<char>>()
    };
    let (hyp, reference) = (strip(hypothesis), strip(reference));
    let (mut precision, mut recall, mut orders) = (0.0, 0.0, 0u32);
    for n in 1..=ORDER {
        let (hyp_grams, ref_grams) = (grams(&hyp, n), grams(&reference, n));
        let (hyp_total, ref_total): (u32, u32) =
            (hyp_grams.values().sum(), ref_grams.values().sum());
        if hyp_total == 0 || ref_total == 0 {
            continue;
        }
        let matched: u32 = hyp_grams
            .iter()
            .map(|(gram, count)| (*count).min(ref_grams.get(gram).copied().unwrap_or(0)))
            .sum();
        precision += f64::from(matched) / f64::from(hyp_total);
        recall += f64::from(matched) / f64::from(ref_total);
        orders += 1;
    }
    if orders == 0 {
        return 0.0;
    }
    let (p, r) = (precision / f64::from(orders), recall / f64::from(orders));
    if p + r == 0.0 {
        return 0.0;
    }
    100.0 * (1.0 + BETA_SQ) * p * r / (BETA_SQ * p + r)
}

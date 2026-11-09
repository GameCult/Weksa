//! The one way the scorer reads words, then NFC and full Unicode case folding.
//! The novel-coverage diagnostic reads text through here, so every script is
//! treated by one rule.
//!
//! A word is a run of word characters: alphanumerics, combining marks and the
//! joiners ZWNJ and ZWJ. Any other character ends it, so `kick:the:bucket`,
//! `kick·the·bucket`, `kick_the_bucket` and `kick'the'bucket` read as
//! `kick the bucket`, as does `kickTheBucket` (a lowercase-to-uppercase
//! transition also ends a word).
//!
//! Han, kana, Thai, Lao, Khmer and Burmese write no word boundaries, and the
//! scorer holds no dictionary to find them. A run of those characters is one
//! word, never one word per character: per-character words would make every
//! honest single-word gloss share bigrams with a held-out sentence, which is
//! the leak signature. A change between such a run and any other script also
//! ends a word. A leak in these scripts shows at n = 1 and in chrF; a
//! dictionary segmenter is the ja-JP panel's to add.

use caseless::default_case_fold_str;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// NFC, then full default case folding.
pub(super) fn normalise(s: &str) -> String {
    default_case_fold_str(&s.nfc().collect::<String>())
}

fn is_joiner(c: char) -> bool {
    matches!(c, '\u{200c}' | '\u{200d}')
}

/// A character that belongs inside a word. Marks must stay: Thai vowels and
/// tone marks, Hindi vowel signs and Hangul jamo are not alphanumeric.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || is_combining_mark(c) || is_joiner(c)
}

/// Han, kana, Thai, Lao, Khmer and Burmese: scripts written without spaces.
fn is_unspaced(c: char) -> bool {
    matches!(
        c,
        '\u{3005}'
            | '\u{3007}'
            | '\u{3021}'..='\u{3029}'
            | '\u{3038}'..='\u{303b}'
            | '\u{3041}'..='\u{30ff}'
            | '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{f900}'..='\u{fad9}'
            | '\u{ff66}'..='\u{ff9f}'
            | '\u{31f0}'..='\u{31ff}'
            | '\u{20000}'..='\u{323af}'
            | '\u{0e01}'..='\u{0e5b}'
            | '\u{0e81}'..='\u{0edf}'
            | '\u{1000}'..='\u{109f}'
            | '\u{1780}'..='\u{17ff}'
    )
}

/// The normalised words of `text` in order; runs with no alphanumeric char
/// (marks alone, say) are dropped.
pub(super) fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut previous: Option<char> = None;
    let end = |from: usize, to: usize, out: &mut Vec<String>| {
        let word = &text[from..to];
        if word.chars().any(char::is_alphanumeric) {
            out.push(normalise(word));
        }
    };
    for (at, c) in text.char_indices() {
        if !is_word_char(c) {
            end(start, at, &mut out);
            start = at + c.len_utf8();
            continue;
        }
        // a mark or joiner belongs to the character before it
        let attached = is_combining_mark(c) || is_joiner(c);
        let breaks = !attached
            && previous.is_some_and(|p| {
                (p.is_lowercase() && c.is_uppercase()) || is_unspaced(p) != is_unspaced(c)
            });
        if breaks {
            end(start, at, &mut out);
            start = at;
        }
        if !attached {
            previous = Some(c);
        }
    }
    end(start, text.len(), &mut out);
    out
}

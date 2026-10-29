//! Form types. A form lives in a newtype whose only constructor is a fallible
//! `parse`; deserializing goes through the same parse, so a malformed value can
//! neither be built in Rust nor read back from a store.

use std::fmt;

/// A value that is not of the form its type names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormError {
    pub form: &'static str,
    pub got: String,
}

impl FormError {
    pub fn new(form: &'static str, got: impl fmt::Display) -> Self {
        Self {
            form,
            got: got.to_string(),
        }
    }
}

impl fmt::Display for FormError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a {}", self.got, self.form)
    }
}

impl std::error::Error for FormError {}

/// Wires a string form type (with `parse` and `Display`) to FromStr, serde's
/// `try_from`/`into` String and a published schema.
macro_rules! string_form {
    ($t:ident, $schema:tt) => {
        impl std::str::FromStr for $t {
            type Err = $crate::form::FormError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::parse(s)
            }
        }
        impl TryFrom<String> for $t {
            type Error = $crate::form::FormError;
            fn try_from(s: String) -> Result<Self, Self::Error> {
                Self::parse(&s)
            }
        }
        impl From<$t> for String {
            fn from(v: $t) -> String {
                v.to_string()
            }
        }
        impl schemars::JsonSchema for $t {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                stringify!($t).into()
            }
            fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
                schemars::json_schema!($schema)
            }
        }
    };
}
pub(crate) use string_form;

/// A BCP 47-form language tag: 1 to 35 bytes of ASCII alphanumerics in
/// hyphen-separated subtags of 1 to 8 bytes; the first is 2 to 8 letters, or the
/// single letter `x` of a private-use tag. Form only: no registry is consulted.
pub(crate) fn is_language_tag(s: &str) -> bool {
    if s.len() > 35 {
        return false;
    }
    let mut subtags = s.split('-');
    let first = subtags.next().unwrap_or("");
    let first_ok = first.bytes().all(|b| b.is_ascii_alphabetic())
        && ((2..=8).contains(&first.len()) || first == "x");
    first_ok
        && subtags
            .all(|t| (1..=8).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_alphanumeric()))
}

/// A slug: `[a-z0-9]` and inner `-`, 1 to 48 bytes, no edge `-`.
pub(crate) fn is_slug(s: &str) -> bool {
    let b = s.as_bytes();
    (1..=48).contains(&b.len())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        && b[0] != b'-'
        && b[b.len() - 1] != b'-'
}

/// A General_Category Cf code point, or the line and paragraph separators
/// U+2028 and U+2029, other than the joiners ZWNJ U+200C and ZWJ U+200D. The
/// ranges are Unicode 15.1 Cf; `tests/format_refused.txt` is the full list from
/// Python's `unicodedata` that the table is swept against.
pub(crate) fn is_format(c: char) -> bool {
    matches!(
        c,
        '\u{ad}'
            | '\u{600}'..='\u{605}'
            | '\u{61c}'
            | '\u{6dd}'
            | '\u{70f}'
            | '\u{890}'..='\u{891}'
            | '\u{8e2}'
            | '\u{180e}'
            | '\u{200b}'
            | '\u{200e}'..='\u{200f}'
            | '\u{2028}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206f}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
            | '\u{110bd}'
            | '\u{110cd}'
            | '\u{13430}'..='\u{1343f}'
            | '\u{1bca0}'..='\u{1bca3}'
            | '\u{1d173}'..='\u{1d17a}'
            | '\u{e0001}'
            | '\u{e0020}'..='\u{e007f}'
    )
}

/// ZWNJ and ZWJ: Cf, but scripts and emoji sequences need them inside a form.
pub(crate) fn is_joiner(c: char) -> bool {
    matches!(c, '\u{200c}' | '\u{200d}')
}

/// A character a name or form may not hold: every `is_format` code point, and
/// the invisible fillers that are Default_Ignorable without being Cf (Hangul
/// U+115F, U+1160, U+3164, U+FFA0 and Khmer U+17B4, U+17B5). Joiners,
/// variation selectors, CGJ and the Mongolian selectors are not refused.
pub(crate) fn is_refused_format(c: char) -> bool {
    is_format(c)
        || matches!(
            c,
            '\u{115f}' | '\u{1160}' | '\u{17b4}' | '\u{17b5}' | '\u{3164}' | '\u{ffa0}'
        )
}

/// A joiner, a variation selector (U+FE00..U+FE0F, U+E0100..U+E01EF), CGJ
/// U+034F or a Mongolian free variation selector (U+180B..U+180D, U+180F): it
/// shapes a neighbour and shows nothing alone.
pub(crate) fn is_joining_only(c: char) -> bool {
    is_joiner(c)
        || matches!(
            c,
            '\u{34f}'
                | '\u{180b}'..='\u{180d}'
                | '\u{180f}'
                | '\u{fe00}'..='\u{fe0f}'
                | '\u{e0100}'..='\u{e01ef}'
        )
}

/// An attestation string (author, persona, fixture, note, work, url, generator,
/// digest): at most 200 bytes, at least one alphanumeric, no control, format or
/// joiner character. Whitespace between words is fine; it attests something.
pub(crate) fn is_attestation(s: &str) -> bool {
    s.len() <= 200
        && s.chars().any(char::is_alphanumeric)
        && !s
            .chars()
            .any(|c| c.is_control() || is_refused_format(c) || is_joiner(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule against every code point: `is_format` is exactly the listed
    /// Cf-minus-joiners-plus-U+2028/9 set, so each range has both of its
    /// edges and the gaps between ranges pinned.
    #[test]
    fn format_rule_matches_the_unicode_list_at_every_code_point() {
        let listed: std::collections::BTreeSet<u32> = include_str!("../tests/format_refused.txt")
            .lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| u32::from_str_radix(l, 16).unwrap())
            .collect();
        for code in 0..=0x10ffff_u32 {
            if let Some(c) = char::from_u32(code) {
                assert_eq!(is_format(c), listed.contains(&code), "U+{code:X}");
            }
        }
        assert!(!is_format('\u{200c}') && !is_format('\u{200d}'));
    }

    #[test]
    fn an_attestation_is_bounded_readable_text() {
        assert!(is_attestation("a"));
        assert!(is_attestation(&"x".repeat(200)));
        assert!(!is_attestation(&"x".repeat(201)));
        assert!(is_attestation("two words, 2 more"));
        for bad in [
            "", " ", "\t", "\u{200b}", "\u{200d}", "\0", "--", "a\nb", "a\u{ad}b",
        ] {
            assert!(!is_attestation(bad), "{bad:?}");
        }
    }
}

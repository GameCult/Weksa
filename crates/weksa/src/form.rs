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

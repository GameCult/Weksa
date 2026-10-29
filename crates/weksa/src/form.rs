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

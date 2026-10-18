//! Exact decimal values, kept as the text the server sent.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, Deserialize, Deserializer, Unexpected, Visitor};
use serde::ser::{Serialize, Serializer};

/// A decimal value kept exactly as its text: an optional leading `-`, one or
/// more digits, and optionally a `.` followed by one or more digits.
///
/// suri never turns a decimal into a binary float, so a price or quantity
/// keeps every digit the server sent, however many there are. Equality is
/// equality of the text: `"5"` and `"5.0"` are different values, as they are
/// on the wire. There is no ordering and no arithmetic; read the text with
/// [`Decimal::as_str`] into the decimal type of your choice.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Decimal(String);

impl Decimal {
    /// The decimal's text, exactly as received or parsed.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The error returned when text is not a decimal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDecimalError {
    kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    Grammar,
}

impl fmt::Display for ParseDecimalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            Kind::Grammar => f.write_str(
                "invalid decimal: expected digits with an optional leading '-' \
                 and an optional '.' followed by digits",
            ),
        }
    }
}

impl std::error::Error for ParseDecimalError {}

fn is_decimal(text: &str) -> bool {
    fn digits(part: &str) -> bool {
        !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())
    }
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    match unsigned.split_once('.') {
        Some((whole, fraction)) => digits(whole) && digits(fraction),
        None => digits(unsigned),
    }
}

impl FromStr for Decimal {
    type Err = ParseDecimalError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if is_decimal(text) {
            Ok(Decimal(text.to_owned()))
        } else {
            Err(ParseDecimalError {
                kind: Kind::Grammar,
            })
        }
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Accepts a JSON string in the decimal grammar or a JSON integer within 64
/// bits (kept as its digits). Every other input, a fractional or exponent
/// number and the number `-0` among them, is rejected rather than rounded.
impl<'de> Deserialize<'de> for Decimal {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(DecimalVisitor)
    }
}

struct DecimalVisitor;

// Only these three methods are implemented: serde's defaults reject every
// other input, including the 128-bit integer methods.
impl Visitor<'_> for DecimalVisitor {
    type Value = Decimal;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a decimal string or an integer")
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Decimal, E> {
        text.parse()
            .map_err(|_| E::invalid_value(Unexpected::Str(text), &self))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Decimal, E> {
        Ok(Decimal(value.to_string()))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Decimal, E> {
        Ok(Decimal(value.to_string()))
    }
}

/// Always written as a JSON string of the exact text, never as a number.
impl Serialize for Decimal {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::is_decimal;

    #[test]
    fn the_grammar_accepts_digits_with_an_optional_sign_and_fraction() {
        for text in ["0", "-0", "007", "1.0", "1.00", "-12.5"] {
            assert!(is_decimal(text), "{text:?}");
        }
        for text in ["", "-", "-.", ".", "+1", ".5", "5.", "1.2.3", "--1", "1-"] {
            assert!(!is_decimal(text), "{text:?}");
        }
    }
}

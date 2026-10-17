//! Values that become a segment of a request's URL path.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

/// A symbol or other value placed in a URL path: printable ASCII other than
/// space, not empty, and not `.` or `..`.
///
/// A `.` or `..` segment would change which endpoint a request reaches (a
/// close-one-position request could become close-all), so it cannot be built.
/// A `/`, as in the crypto pair `BTC/USD`, is allowed. Equality is equality
/// of the text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symbol(String);

impl Symbol {
    /// The symbol's text, exactly as given.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A UUID-shaped identifier placed in a URL path: 8-4-4-4-12 hexadecimal
/// digits in either case, kept as written.
///
/// Equality and hashing ignore ASCII case, so the same ID written in upper
/// and lower case is one value.
#[derive(Debug, Clone)]
pub struct Id(String);

impl Id {
    /// The ID's text, exactly as given.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PartialEq for Id {
    fn eq(&self, other: &Id) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}

impl Eq for Id {}

impl Hash for Id {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for byte in self.0.bytes() {
            state.write_u8(byte.to_ascii_lowercase());
        }
    }
}

/// The error returned when text cannot be a path value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsePathValueError {
    kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Empty,
    DotSegment,
    Bytes,
    NotUuid,
}

impl fmt::Display for ParsePathValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            Kind::Empty => "empty value",
            Kind::DotSegment => "'.' and '..' are not allowed",
            Kind::Bytes => "only printable ASCII other than space is allowed",
            Kind::NotUuid => "not a UUID: expected 8-4-4-4-12 hexadecimal digits",
        })
    }
}

impl std::error::Error for ParsePathValueError {}

fn check_symbol(text: &str) -> Result<(), ParsePathValueError> {
    let kind = if text.is_empty() {
        Kind::Empty
    } else if text == "." || text == ".." {
        Kind::DotSegment
    } else if !text.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
        Kind::Bytes
    } else {
        return Ok(());
    };
    Err(ParsePathValueError { kind })
}

fn is_uuid(text: &str) -> bool {
    text.len() == 36
        && text.bytes().enumerate().all(|(at, b)| match at {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

impl FromStr for Symbol {
    type Err = ParsePathValueError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        check_symbol(text)?;
        Ok(Symbol(text.to_owned()))
    }
}

impl FromStr for Id {
    type Err = ParsePathValueError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        check_symbol(text)?;
        if !is_uuid(text) {
            return Err(ParsePathValueError {
                kind: Kind::NotUuid,
            });
        }
        Ok(Id(text.to_owned()))
    }
}

/// Every ID is a valid symbol, so an ID can be passed where a symbol or an
/// ID is accepted.
impl From<Id> for Symbol {
    fn from(id: Id) -> Symbol {
        Symbol(id.0)
    }
}

impl From<&Id> for Symbol {
    fn from(id: &Id) -> Symbol {
        Symbol(id.0.clone())
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

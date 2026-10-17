//! Enum values received from the server, known or not.

use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;

use serde::de::{self, Deserialize, Deserializer, Visitor};

/// A value of an enumeration the server sends, which may be one suri does not
/// know yet.
///
/// Alpaca adds enum values (a new order status, a new activity type) without
/// a version change. A value suri knows decodes as `Known`; any other arrives
/// as `Unknown` with its text, never as an error and never as a different
/// known value. An `Inbound` cannot be sent back: request types take the
/// plain enum.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Inbound<T> {
    /// A value suri knows.
    Known(T),
    /// A value suri does not know, with the text the server sent.
    Unknown(String),
}

impl<T> Inbound<T> {
    /// The known value, if there is one.
    pub fn known(&self) -> Option<&T> {
        match self {
            Inbound::Known(value) => Some(value),
            Inbound::Unknown(_) => None,
        }
    }
}

/// Writes the known value's text, or the unknown text unchanged.
impl<T: fmt::Display> fmt::Display for Inbound<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Inbound::Known(value) => value.fmt(f),
            Inbound::Unknown(text) => f.write_str(text),
        }
    }
}

/// Reads a JSON string: `Known` when `T` parses it, else `Unknown` with the
/// whole text. `T`'s `FromStr` accepts exactly the documented wire values.
impl<'de, T: FromStr> Deserialize<'de> for Inbound<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(InboundVisitor(PhantomData))
    }
}

struct InboundVisitor<T>(PhantomData<T>);

impl<T: FromStr> Visitor<'_> for InboundVisitor<T> {
    type Value = Inbound<T>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string")
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Inbound<T>, E> {
        Ok(match text.parse() {
            Ok(value) => Inbound::Known(value),
            Err(_) => Inbound::Unknown(text.to_owned()),
        })
    }
}

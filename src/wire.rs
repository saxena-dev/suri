//! Enumerations with documented wire values.

use std::fmt;

/// Declares a public enum whose variants each stand for one documented wire
/// text: `as_str` and `Display` give the text, `FromStr` accepts exactly the
/// documented texts, and `Serialize` writes the text. Responses hold such an
/// enum inside [`Inbound`](crate::Inbound), so a value suri does not know is
/// kept rather than refused.
macro_rules! wire_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $( $(#[$variant_meta:meta])* $variant:ident => $text:literal, )+
        }
    ) => {
        $(#[$meta])*
        #[non_exhaustive]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $( $(#[$variant_meta])* $variant, )+
        }

        impl $name {
            /// The value as Alpaca writes it.
            pub fn as_str(&self) -> &'static str {
                match self {
                    $( Self::$variant => $text, )+
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl std::str::FromStr for $name {
            type Err = crate::ParseEnumError;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                match text {
                    $( $text => Ok(Self::$variant), )+
                    _ => Err(crate::ParseEnumError::new()),
                }
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }
    };
}

pub(crate) use wire_enum;

/// The error returned when text is not one of an enum's documented values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseEnumError {
    kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    NotDocumented,
}

impl ParseEnumError {
    pub(crate) fn new() -> ParseEnumError {
        ParseEnumError {
            kind: Kind::NotDocumented,
        }
    }
}

impl fmt::Display for ParseEnumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            Kind::NotDocumented => f.write_str("not a documented value"),
        }
    }
}

impl std::error::Error for ParseEnumError {}

#[cfg(test)]
mod tests {
    use crate::Inbound;

    wire_enum! {
        /// A two-value enum to test the macro with.
        Sample {
            /// The first value.
            First => "first",
            /// The second value, written with an underscore.
            SecondValue => "second_value",
        }
    }

    #[test]
    fn each_documented_text_parses_and_prints_back() {
        for (text, value) in [
            ("first", Sample::First),
            ("second_value", Sample::SecondValue),
        ] {
            assert_eq!(text.parse::<Sample>(), Ok(value));
            assert_eq!(value.as_str(), text);
            assert_eq!(value.to_string(), text);
            assert_eq!(
                serde_json::to_string(&value).unwrap(),
                format!("\"{text}\"")
            );
        }
    }

    #[test]
    fn other_texts_are_not_values() {
        for text in ["First", "FIRST", "second-value", "", " first"] {
            let error = text.parse::<Sample>().unwrap_err();
            assert_eq!(error.to_string(), "not a documented value");
        }
    }

    #[test]
    fn inbound_keeps_an_unknown_text() {
        let known: Inbound<Sample> = serde_json::from_str("\"first\"").unwrap();
        assert_eq!(known, Inbound::Known(Sample::First));
        let unknown: Inbound<Sample> = serde_json::from_str("\"third\"").unwrap();
        assert_eq!(unknown, Inbound::Unknown("third".to_owned()));
    }
}

//! `Inbound` keeps enum values suri does not know, with their text.

use std::fmt;
use std::str::FromStr;

use suri::Inbound;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Side {
    Buy,
    Sell,
}

impl FromStr for Side {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, ()> {
        match text {
            "buy" => Ok(Side::Buy),
            "sell" => Ok(Side::Sell),
            _ => Err(()),
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Side::Buy => "buy",
            Side::Sell => "sell",
        })
    }
}

fn decode(json: &str) -> Result<Inbound<Side>, serde_json::Error> {
    serde_json::from_str(json)
}

#[test]
fn a_known_value_decodes_as_known() {
    assert_eq!(decode(r#""buy""#).unwrap(), Inbound::Known(Side::Buy));
    assert_eq!(decode(r#""sell""#).unwrap(), Inbound::Known(Side::Sell));
}

#[test]
fn any_other_string_is_kept_as_unknown_with_its_text() {
    assert_eq!(
        decode(r#""hold""#).unwrap(),
        Inbound::Unknown("hold".to_owned())
    );
    assert_eq!(decode(r#""""#).unwrap(), Inbound::Unknown(String::new()));
    assert_eq!(
        decode(r#""Buy""#).unwrap(),
        Inbound::Unknown("Buy".to_owned())
    );
}

#[test]
fn a_value_that_is_not_a_string_is_an_error() {
    assert!(decode("1").is_err());
    assert!(decode("null").is_err());
    assert!(decode("[]").is_err());
}

#[test]
fn a_null_field_is_none_when_optional() {
    let value: Option<Inbound<Side>> = serde_json::from_str("null").unwrap();
    assert_eq!(value, None);
}

#[test]
fn display_writes_the_wire_text() {
    assert_eq!(Inbound::Known(Side::Buy).to_string(), "buy");
    assert_eq!(
        Inbound::<Side>::Unknown("hold".to_owned()).to_string(),
        "hold"
    );
}

#[test]
fn known_returns_only_a_known_value() {
    assert_eq!(Inbound::Known(Side::Sell).known(), Some(&Side::Sell));
    assert_eq!(Inbound::<Side>::Unknown("hold".to_owned()).known(), None);
}

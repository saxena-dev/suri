//! `Decimal` keeps the server's decimal text exactly, in and out of JSON.

use serde::Deserialize;
use suri::{Decimal, ParseDecimalError};

const MESSAGE: &str = "invalid decimal: expected digits with an optional leading '-' \
                       and an optional '.' followed by digits";

#[test]
fn text_in_the_grammar_is_kept_as_written() {
    for text in ["0", "-0", "007", "1.0", "1.00", "123.456", "-0.000001"] {
        let decimal: Decimal = text.parse().unwrap();
        assert_eq!(decimal.as_str(), text);
        assert_eq!(decimal.to_string(), text);
    }
}

#[test]
fn text_outside_the_grammar_is_rejected() {
    for text in [
        "", "-", "+1", ".5", "5.", "1e5", "1E5", "NaN", "inf", " 1", "1 ", "1_000", "0x10", "１",
    ] {
        let error: ParseDecimalError = text.parse::<Decimal>().unwrap_err();
        assert_eq!(error.to_string(), MESSAGE, "{text:?}");
    }
}

#[test]
fn a_thousand_digits_are_kept() {
    let text = format!("{}.{}", "9".repeat(500), "1".repeat(500));
    let decimal: Decimal = text.parse().unwrap();
    assert_eq!(decimal.as_str(), text);
}

#[test]
fn equality_is_equality_of_the_text() {
    let five: Decimal = "5".parse().unwrap();
    let five_point_zero: Decimal = "5.0".parse().unwrap();
    assert_ne!(five, five_point_zero);
    assert_eq!(five, "5".parse().unwrap());
}

#[test]
fn json_strings_in_the_grammar_and_64_bit_integers_decode() {
    let cases = [
        (r#""0""#, "0"),
        (r#""-0""#, "-0"),
        (r#""007""#, "007"),
        (r#""1.0""#, "1.0"),
        (r#""1.00""#, "1.00"),
        ("1", "1"),
        // Not representable in a binary64 float: an integer routed through
        // one would come back as ...992.
        ("9007199254740993", "9007199254740993"),
        ("-9007199254740993", "-9007199254740993"),
        ("-3", "-3"),
        ("18446744073709551615", "18446744073709551615"),
        ("-9223372036854775808", "-9223372036854775808"),
    ];
    for (json, text) in cases {
        let decimal: Decimal = serde_json::from_str(json).unwrap();
        assert_eq!(decimal.as_str(), text, "{json}");
    }
}

#[test]
fn every_other_json_value_is_rejected() {
    for json in [
        r#""1e5""#,
        r#""""#,
        "1.5",
        "1e5",
        "-0",
        "18446744073709551616",
        "-9223372036854775809",
        "true",
        "null",
        "[]",
        "{}",
    ] {
        assert!(serde_json::from_str::<Decimal>(json).is_err(), "{json}");
    }
}

#[test]
fn a_thirty_digit_money_value_survives_a_round_trip() {
    // A cost basis from alpaca-py's Broker position fixtures: 30 significant
    // digits, more than common decimal crates hold.
    let body = r#"{"qty": "49.6499849505605227113711001112"}"#;
    let value: serde_json::Value = serde_json::from_str(body).unwrap();
    let decimal = Decimal::deserialize(&value["qty"]).unwrap();
    assert_eq!(decimal.as_str(), "49.6499849505605227113711001112");
    assert_eq!(
        serde_json::to_string(&decimal).unwrap(),
        r#""49.6499849505605227113711001112""#
    );
}

#[test]
fn a_decimal_is_written_as_a_json_string() {
    for text in ["0", "-0", "007", "1.0", "1.00", "-12.5"] {
        let decimal: Decimal = text.parse().unwrap();
        assert_eq!(
            serde_json::to_string(&decimal).unwrap(),
            format!("\"{text}\"")
        );
    }
}

#[test]
fn a_value_integer_beyond_64_bits_is_rejected() {
    // Through serde_json::Value with arbitrary_precision on, such an integer
    // reaches the 128-bit visitor methods, which Decimal does not accept.
    let value: serde_json::Value = serde_json::from_str("[18446744073709551616]").unwrap();
    assert!(Decimal::deserialize(&value[0]).is_err());
}

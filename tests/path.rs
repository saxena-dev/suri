//! `Symbol` and `Id` accept only values that stay one URL path segment.

use std::collections::HashSet;

use suri::{Id, ParsePathValueError, Symbol};

fn symbol_error(text: &str) -> String {
    let error: ParsePathValueError = text.parse::<Symbol>().unwrap_err();
    error.to_string()
}

#[test]
fn values_that_would_change_the_path_are_rejected() {
    let dots = "'.' and '..' are not allowed";
    let bytes = "only printable ASCII other than space is allowed";
    assert_eq!(symbol_error(""), "empty value");
    assert_eq!(symbol_error("."), dots);
    assert_eq!(symbol_error(".."), dots);
    assert_eq!(symbol_error(".\t."), bytes);
    assert_eq!(symbol_error("\n.."), bytes);
    assert_eq!(symbol_error("AA\tPL"), bytes);
    assert_eq!(symbol_error("AA PL"), bytes);
    assert_eq!(symbol_error("café"), bytes);
    assert_eq!(symbol_error("AA\u{7f}"), bytes);
}

#[test]
fn symbols_with_slashes_dots_and_percent_signs_are_kept() {
    for text in ["AAPL", "BTC/USD", "BRK.B", "%2e%2e", "..A", "~", "!"] {
        let symbol: Symbol = text.parse().unwrap();
        assert_eq!(symbol.as_str(), text);
        assert_eq!(symbol.to_string(), text);
    }
}

#[test]
fn symbols_compare_as_text() {
    let upper: Symbol = "AAPL".parse().unwrap();
    let lower: Symbol = "aapl".parse().unwrap();
    assert_ne!(upper, lower);
}

#[test]
fn an_id_is_a_uuid_in_either_case_kept_as_written() {
    let lower: Id = "904837e3-3b76-47ec-b432-046db621571b".parse().unwrap();
    let upper: Id = "904837E3-3B76-47EC-B432-046DB621571B".parse().unwrap();
    assert_eq!(upper.to_string(), "904837E3-3B76-47EC-B432-046DB621571B");
    assert_eq!(upper.as_str(), "904837E3-3B76-47EC-B432-046DB621571B");
    assert_eq!(lower, upper);
    let set: HashSet<Id> = [lower, upper].into_iter().collect();
    assert_eq!(set.len(), 1);
}

#[test]
fn text_that_is_not_a_uuid_is_not_an_id() {
    let message = "not a UUID: expected 8-4-4-4-12 hexadecimal digits";
    for text in [
        // 36 characters with a hex digit where the first hyphen belongs.
        "904837e3a3b76-47ec-b432-046db621571b",
        "904837e3-3b76-47ec-b432-046db621571g",
        "904837e3-3b76-47ec-b432-046db621571",
        "904837e3-3b76-47ec-b432-046db621571bb",
    ] {
        let error = text.parse::<Id>().unwrap_err();
        assert_eq!(error.to_string(), message, "{text}");
    }
    assert_eq!(
        "..".parse::<Id>().unwrap_err().to_string(),
        "'.' and '..' are not allowed"
    );
    assert_eq!("".parse::<Id>().unwrap_err().to_string(), "empty value");
}

//! `Timestamp` and `Date` read RFC 3339 text exactly, reject what they cannot
//! hold without loss, and write back text they read the same way.

use suri::{Date, ParseTimeError, Timestamp};

fn at(seconds: i64, nanoseconds: u32) -> Timestamp {
    Timestamp::from_unix(seconds, nanoseconds).unwrap()
}

fn rejected(text: &str) -> String {
    let error: ParseTimeError = text.parse::<Timestamp>().unwrap_err();
    error.to_string()
}

#[test]
fn fractions_of_zero_to_nine_digits_are_kept_exactly() {
    let cases = [
        ("2021-02-22T15:51:44Z", at(1_614_009_104, 0)),
        ("2021-02-22T15:51:44.208Z", at(1_614_009_104, 208_000_000)),
        (
            "2021-02-22T15:51:44.208123Z",
            at(1_614_009_104, 208_123_000),
        ),
        (
            "2021-02-22T15:51:44.20812345Z",
            at(1_614_009_104, 208_123_450),
        ),
        (
            "2021-02-22T15:51:44.208123456Z",
            at(1_614_009_104, 208_123_456),
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(text.parse::<Timestamp>(), Ok(expected), "{text}");
        assert_eq!(expected.to_string().parse::<Timestamp>(), Ok(expected));
    }
}

#[test]
fn a_fraction_of_ten_digits_or_none_after_the_point_is_rejected() {
    let message = "fractional seconds need 1 to 9 digits";
    assert_eq!(rejected("2021-02-22T15:51:44.2081234567Z"), message);
    assert_eq!(rejected("2021-02-22T15:51:44.Z"), message);
}

#[test]
fn offsets_are_applied_and_dropped() {
    // The order example of Alpaca's streaming guide: 13:45 at -04:00.
    assert_eq!(
        "2022-04-19T13:45:04.981350886-04:00".parse::<Timestamp>(),
        Ok(at(1_650_390_304, 981_350_886))
    );
    let utc = at(1_650_390_304, 0);
    for text in [
        "2022-04-19T17:45:04Z",
        "2022-04-19t17:45:04z",
        "2022-04-19T23:15:04+05:30",
        "2022-04-19T17:45:04-00:00",
    ] {
        assert_eq!(text.parse::<Timestamp>(), Ok(utc), "{text}");
        assert_eq!(utc.to_string().parse::<Timestamp>(), Ok(utc));
    }
}

#[test]
fn malformed_offsets_and_separators_are_rejected() {
    assert_eq!(
        rejected("2022-04-19T17:45:04+0530"),
        "invalid time-zone offset: expected Z or ±HH:MM"
    );
    assert_eq!(
        rejected("2022-04-19T17:45:04+05"),
        "invalid time-zone offset: expected Z or ±HH:MM"
    );
    assert_eq!(rejected("2022-04-19T17:45:04"), "missing time-zone offset");
    assert_eq!(
        rejected("2022-04-19T17:45:04+24:00"),
        "time-zone offset out of range"
    );
    assert_eq!(
        rejected("2022-04-19T17:45:04+05:60"),
        "time-zone offset out of range"
    );
    assert_eq!(
        rejected("2022-04-19 17:45:04Z"),
        "expected 'T' between the date and the time"
    );
    assert_eq!(
        rejected("2022-04-19T17:45:04Zx"),
        "unexpected text after the value"
    );
    assert_eq!(
        rejected("2022/04/19T17:45:04Z"),
        "invalid date: expected YYYY-MM-DD"
    );
    assert_eq!(
        rejected("2022-04-19T17-45-04Z"),
        "invalid time: expected HH:MM:SS"
    );
    assert_eq!(
        rejected("2022-04-19T17:45:04+05-30"),
        "invalid time-zone offset: expected Z or ±HH:MM"
    );
    assert_eq!(
        rejected("2022-04-19T17:45:04,5Z"),
        "invalid time-zone offset: expected Z or ±HH:MM"
    );
}

#[test]
fn every_field_is_range_checked() {
    assert_eq!(rejected("2022-13-01T00:00:00Z"), "month out of range");
    assert_eq!(rejected("2022-00-01T00:00:00Z"), "month out of range");
    assert_eq!(
        rejected("2024-04-31T00:00:00Z"),
        "day out of range for the month"
    );
    assert_eq!(
        rejected("2024-04-00T00:00:00Z"),
        "day out of range for the month"
    );
    assert_eq!(rejected("2024-04-30T24:00:00Z"), "hour out of range");
    assert_eq!(rejected("2024-04-30T23:60:00Z"), "minute out of range");
    assert_eq!(rejected("2024-04-30T23:59:61Z"), "second out of range");
    assert_eq!(
        rejected("2016-12-31T23:59:60Z"),
        "leap second (60) is not supported"
    );
    assert_eq!(
        rejected("22-04-19T00:00:00Z"),
        "invalid date: expected YYYY-MM-DD"
    );
    assert_eq!(
        rejected("2022-04-19T1:00:00Z"),
        "invalid time: expected HH:MM:SS"
    );
}

#[test]
fn february_follows_the_gregorian_leap_year_rules() {
    for text in ["2024-02-29", "2000-02-29", "0000-02-29"] {
        assert!(text.parse::<Date>().is_ok(), "{text}");
    }
    for text in ["2023-02-29", "1900-02-29"] {
        assert_eq!(
            text.parse::<Date>().unwrap_err().to_string(),
            "day out of range for the month",
            "{text}"
        );
    }
}

#[test]
fn the_range_is_years_0000_to_9999() {
    assert_eq!(
        "0000-01-01T00:00:00Z".parse::<Timestamp>(),
        Ok(at(-62_167_219_200, 0))
    );
    assert_eq!(
        "9999-12-31T23:59:59.999999999Z".parse::<Timestamp>(),
        Ok(at(253_402_300_799, 999_999_999))
    );
    let message = "outside the supported range (years 0000 to 9999)";
    assert_eq!(rejected("0000-01-01T00:00:00+00:01"), message);
    assert_eq!(rejected("9999-12-31T23:59:59-00:01"), message);
    assert_eq!(Timestamp::from_unix(-62_167_219_201, 0), None);
    assert_eq!(Timestamp::from_unix(253_402_300_800, 0), None);
    assert_eq!(Timestamp::from_unix(0, 1_000_000_000), None);
}

#[test]
fn from_unix_takes_the_floor_of_the_seconds() {
    assert_eq!(at(0, 0).to_string(), "1970-01-01T00:00:00Z");
    assert_eq!(at(1, 0).to_string(), "1970-01-01T00:00:01Z");
    assert_eq!(at(-1, 500_000_000).to_string(), "1969-12-31T23:59:59.5Z");
    assert_eq!(at(951_868_800, 0).to_string(), "2000-03-01T00:00:00Z");
    let t = at(-1, 500_000_000);
    assert_eq!((t.unix_seconds(), t.subsec_nanos()), (-1, 500_000_000));
}

#[test]
fn display_round_trips_and_trims_trailing_zeros() {
    let cases = [
        ("2021-02-22T15:51:44Z", "2021-02-22T15:51:44Z"),
        ("2021-02-22T15:51:44.200Z", "2021-02-22T15:51:44.2Z"),
        (
            "2021-02-22T15:51:44.000000001Z",
            "2021-02-22T15:51:44.000000001Z",
        ),
        (
            "2022-04-19T13:45:04.981350886-04:00",
            "2022-04-19T17:45:04.981350886Z",
        ),
        ("0000-01-01T00:00:00Z", "0000-01-01T00:00:00Z"),
        (
            "9999-12-31T23:59:59.999999999Z",
            "9999-12-31T23:59:59.999999999Z",
        ),
    ];
    for (text, written) in cases {
        let t: Timestamp = text.parse().unwrap();
        assert_eq!(t.to_string(), written);
        assert_eq!(written.parse::<Timestamp>(), Ok(t));
    }
}

#[test]
fn non_ascii_input_at_any_position_is_an_error_not_a_panic() {
    let valid = "2022-04-19T17:45:04.5+05:30";
    for at in 0..=valid.len() {
        for c in ['é', '٠', '€'] {
            let inserted = format!("{}{c}{}", &valid[..at], &valid[at..]);
            assert!(inserted.parse::<Timestamp>().is_err(), "{inserted}");
            if at < valid.len() {
                let replaced = format!("{}{c}{}", &valid[..at], &valid[at + 1..]);
                assert!(replaced.parse::<Timestamp>().is_err(), "{replaced}");
            }
        }
    }
}

#[test]
fn dates_are_read_and_written_as_yyyy_mm_dd() {
    let date: Date = "2024-02-29".parse().unwrap();
    assert_eq!((date.year(), date.month(), date.day()), (2024, 2, 29));
    assert_eq!(date.to_string(), "2024-02-29");
    for text in ["0000-01-01", "9999-12-31"] {
        assert_eq!(text.parse::<Date>().unwrap().to_string(), text);
    }
    assert_eq!(
        "2024-02-29T00:00:00Z"
            .parse::<Date>()
            .unwrap_err()
            .to_string(),
        "unexpected text after the value"
    );
}

#[test]
fn both_types_are_json_strings_in_and_out() {
    let t: Timestamp = serde_json::from_str(r#""2022-04-19T13:45:04.5-04:00""#).unwrap();
    assert_eq!(
        serde_json::to_string(&t).unwrap(),
        r#""2022-04-19T17:45:04.5Z""#
    );
    let d: Date = serde_json::from_str(r#""2024-02-29""#).unwrap();
    assert_eq!(serde_json::to_string(&d).unwrap(), r#""2024-02-29""#);
    assert!(serde_json::from_str::<Timestamp>("1650390304").is_err());
    assert!(serde_json::from_str::<Date>("null").is_err());
    let error = serde_json::from_str::<Timestamp>(r#""2016-12-31T23:59:60Z""#).unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains("leap second (60) is not supported"),
        "{message}"
    );
    assert!(!message.contains("2016-12-31"), "{message}");
}

//! Instants and calendar dates, parsed from RFC 3339 text without losing or
//! folding a digit.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, Serializer};

/// The first instant a `Timestamp` holds: 0000-01-01T00:00:00Z.
const MIN_SECONDS: i64 = -62_167_219_200;
/// The last whole second a `Timestamp` holds: 9999-12-31T23:59:59Z.
const MAX_SECONDS: i64 = 253_402_300_799;
const SECONDS_PER_DAY: i64 = 86_400;
const NANOS_PER_SECOND: u32 = 1_000_000_000;

/// An instant in UTC, to the nanosecond, between 0000-01-01T00:00:00Z and
/// 9999-12-31T23:59:59.999999999Z.
///
/// Parsed from RFC 3339 text: the offset the server sends is applied and not
/// kept. Ten or more fractional digits and leap seconds are rejected rather
/// than trimmed or folded. Written as `YYYY-MM-DDTHH:MM:SS[.fraction]Z`, with
/// trailing zeros removed from the fraction and no fraction when it is zero.
// Field order makes the derived ordering the order in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    seconds: i64,
    nanoseconds: u32,
}

impl Timestamp {
    /// The instant `seconds` after the Unix epoch plus `nanoseconds`.
    /// `seconds` is the floor, so `(-1, 500_000_000)` is
    /// 1969-12-31T23:59:59.5Z. `None` outside the range or when
    /// `nanoseconds` is a second or more.
    pub fn from_unix(seconds: i64, nanoseconds: u32) -> Option<Timestamp> {
        ((MIN_SECONDS..=MAX_SECONDS).contains(&seconds) && nanoseconds < NANOS_PER_SECOND)
            .then_some(Timestamp {
                seconds,
                nanoseconds,
            })
    }

    /// Whole seconds since the Unix epoch (the floor for instants before it).
    pub fn unix_seconds(&self) -> i64 {
        self.seconds
    }

    /// The nanoseconds past [`Timestamp::unix_seconds`], below one second.
    pub fn subsec_nanos(&self) -> u32 {
        self.nanoseconds
    }
}

/// A calendar date between 0000-01-01 and 9999-12-31, read and written as
/// `YYYY-MM-DD`.
// Field order makes the derived ordering calendar order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    year: u16,
    month: u8,
    day: u8,
}

impl Date {
    /// The year, 0 to 9999.
    pub fn year(&self) -> u16 {
        self.year
    }

    /// The month, 1 to 12.
    pub fn month(&self) -> u8 {
        self.month
    }

    /// The day of the month, from 1.
    pub fn day(&self) -> u8 {
        self.day
    }
}

/// The error returned when text is not an RFC 3339 timestamp or date suri
/// accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseTimeError {
    kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Date,
    Month,
    Day,
    Separator,
    Time,
    Hour,
    Minute,
    Second,
    LeapSecond,
    Fraction,
    MissingOffset,
    Offset,
    OffsetRange,
    Range,
    Trailing,
}

impl fmt::Display for ParseTimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            Kind::Date => "invalid date: expected YYYY-MM-DD",
            Kind::Month => "month out of range",
            Kind::Day => "day out of range for the month",
            Kind::Separator => "expected 'T' between the date and the time",
            Kind::Time => "invalid time: expected HH:MM:SS",
            Kind::Hour => "hour out of range",
            Kind::Minute => "minute out of range",
            Kind::Second => "second out of range",
            Kind::LeapSecond => "leap second (60) is not supported",
            Kind::Fraction => "fractional seconds need 1 to 9 digits",
            Kind::MissingOffset => "missing time-zone offset",
            Kind::Offset => "invalid time-zone offset: expected Z or ±HH:MM",
            Kind::OffsetRange => "time-zone offset out of range",
            Kind::Range => "outside the supported range (years 0000 to 9999)",
            Kind::Trailing => "unexpected text after the value",
        })
    }
}

impl std::error::Error for ParseTimeError {}

fn error(kind: Kind) -> ParseTimeError {
    ParseTimeError { kind }
}

/// The value of `count` ASCII digits starting at `at`, if they are all there.
fn digits(bytes: &[u8], at: usize, count: usize) -> Option<u32> {
    let part = bytes.get(at..at + count)?;
    part.iter().try_fold(0u32, |value, &b| {
        b.is_ascii_digit().then(|| value * 10 + u32::from(b - b'0'))
    })
}

fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date: Howard Hinnant's
/// `days_from_civil`.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day_of_year =
        (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The proleptic Gregorian date of a day count since 1970-01-01: Howard
/// Hinnant's `civil_from_days`.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Parses `YYYY-MM-DD` at the start of `bytes`.
fn parse_date(bytes: &[u8]) -> Result<Date, ParseTimeError> {
    let (Some(year), Some(b'-'), Some(month), Some(b'-'), Some(day)) = (
        digits(bytes, 0, 4),
        bytes.get(4),
        digits(bytes, 5, 2),
        bytes.get(7),
        digits(bytes, 8, 2),
    ) else {
        return Err(error(Kind::Date));
    };
    if !(1..=12).contains(&month) {
        return Err(error(Kind::Month));
    }
    if day == 0 || day > days_in_month(i64::from(year), month) {
        return Err(error(Kind::Day));
    }
    Ok(Date {
        year: year as u16,
        month: month as u8,
        day: day as u8,
    })
}

fn parse_timestamp(bytes: &[u8]) -> Result<Timestamp, ParseTimeError> {
    let date = parse_date(bytes)?;
    if !matches!(bytes.get(10), Some(b'T' | b't')) {
        return Err(error(Kind::Separator));
    }
    let (Some(hour), Some(b':'), Some(minute), Some(b':'), Some(second)) = (
        digits(bytes, 11, 2),
        bytes.get(13),
        digits(bytes, 14, 2),
        bytes.get(16),
        digits(bytes, 17, 2),
    ) else {
        return Err(error(Kind::Time));
    };
    if hour > 23 {
        return Err(error(Kind::Hour));
    }
    if minute > 59 {
        return Err(error(Kind::Minute));
    }
    if second == 60 {
        return Err(error(Kind::LeapSecond));
    }
    if second > 59 {
        return Err(error(Kind::Second));
    }

    let mut at = 19;
    let mut nanoseconds = 0;
    if bytes.get(at) == Some(&b'.') {
        let count = bytes[at + 1..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        if count == 0 || count > 9 {
            return Err(error(Kind::Fraction));
        }
        let fraction = digits(bytes, at + 1, count).ok_or(error(Kind::Fraction))?;
        nanoseconds = fraction * 10u32.pow(9 - count as u32);
        at += 1 + count;
    }

    let offset_seconds = match bytes.get(at) {
        None => return Err(error(Kind::MissingOffset)),
        Some(b'Z' | b'z') => {
            at += 1;
            0
        }
        Some(&sign @ (b'+' | b'-')) => {
            let (Some(hours), Some(b':'), Some(minutes)) = (
                digits(bytes, at + 1, 2),
                bytes.get(at + 3),
                digits(bytes, at + 4, 2),
            ) else {
                return Err(error(Kind::Offset));
            };
            if hours > 23 || minutes > 59 {
                return Err(error(Kind::OffsetRange));
            }
            at += 6;
            let magnitude = i64::from(hours * 3_600 + minutes * 60);
            if sign == b'-' { -magnitude } else { magnitude }
        }
        Some(_) => return Err(error(Kind::Offset)),
    };
    if at != bytes.len() {
        return Err(error(Kind::Trailing));
    }

    let days = days_from_civil(
        i64::from(date.year),
        u32::from(date.month),
        u32::from(date.day),
    );
    let local = days * SECONDS_PER_DAY + i64::from(hour * 3_600 + minute * 60 + second);
    Timestamp::from_unix(local - offset_seconds, nanoseconds).ok_or(error(Kind::Range))
}

impl FromStr for Timestamp {
    type Err = ParseTimeError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        parse_timestamp(text.as_bytes())
    }
}

impl FromStr for Date {
    type Err = ParseTimeError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let date = parse_date(text.as_bytes())?;
        if text.len() != 10 {
            return Err(error(Kind::Trailing));
        }
        Ok(date)
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (year, month, day) = civil_from_days(self.seconds.div_euclid(SECONDS_PER_DAY));
        let time = self.seconds.rem_euclid(SECONDS_PER_DAY);
        write!(
            f,
            "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}",
            time / 3_600,
            time % 3_600 / 60,
            time % 60
        )?;
        if self.nanoseconds != 0 {
            let (mut digits, mut width) = (self.nanoseconds, 9);
            while digits % 10 == 0 {
                digits /= 10;
                width -= 1;
            }
            write!(f, ".{digits:0width$}")?;
        }
        f.write_str("Z")
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

struct TextVisitor<T>(std::marker::PhantomData<T>);

impl<T: FromStr<Err = ParseTimeError>> Visitor<'_> for TextVisitor<T> {
    type Value = T;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an RFC 3339 string")
    }

    // An invalid-value error carrying suri's message, without the input.
    fn visit_str<E: de::Error>(self, text: &str) -> Result<T, E> {
        text.parse().map_err(|error: ParseTimeError| {
            E::invalid_value(de::Unexpected::Other(&error.to_string()), &self)
        })
    }
}

/// Reads a `Timestamp`: RFC 3339 text and, with the `stream` feature, the
/// MessagePack timestamp extension.
struct TimestampVisitor;

impl<'de> Visitor<'de> for TimestampVisitor {
    type Value = Timestamp;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an RFC 3339 string")
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Timestamp, E> {
        TextVisitor(std::marker::PhantomData).visit_str(text)
    }

    // rmp-serde hands an extension over as a newtype holding its type and
    // its data.
    #[cfg(feature = "stream")]
    fn visit_newtype_struct<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Timestamp, D::Error> {
        deserializer.deserialize_seq(ExtensionVisitor)
    }
}

/// Reads a MessagePack extension's type and data as a timestamp.
#[cfg(feature = "stream")]
struct ExtensionVisitor;

#[cfg(feature = "stream")]
impl<'de> Visitor<'de> for ExtensionVisitor {
    type Value = Timestamp;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a MessagePack timestamp")
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Timestamp, A::Error> {
        let kind: i8 = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(0, &self))?;
        let Extension(timestamp) = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(1, &self))?;
        match (kind, timestamp) {
            (-1, Some(timestamp)) => Ok(timestamp),
            _ => Err(de::Error::invalid_value(
                de::Unexpected::Other("another extension"),
                &self,
            )),
        }
    }
}

/// An extension's data read as a timestamp's: `None` for a length the
/// format does not define, nanoseconds of a second or more, or an instant
/// outside `Timestamp`'s range.
#[cfg(feature = "stream")]
struct Extension(Option<Timestamp>);

#[cfg(feature = "stream")]
impl<'de> Deserialize<'de> for Extension {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Data;

        impl Visitor<'_> for Data {
            type Value = Extension;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an extension's data")
            }

            fn visit_bytes<E: de::Error>(self, data: &[u8]) -> Result<Extension, E> {
                Ok(Extension(timestamp_extension(data)))
            }
        }

        deserializer.deserialize_bytes(Data)
    }
}

/// The MessagePack timestamp's three forms, big-endian: 32-bit seconds;
/// 64-bit, nanoseconds in the upper 30 bits and seconds in the lower 34;
/// 96-bit, 32-bit nanoseconds then signed 64-bit seconds.
#[cfg(feature = "stream")]
fn timestamp_extension(data: &[u8]) -> Option<Timestamp> {
    match data.len() {
        4 => Timestamp::from_unix(i64::from(u32::from_be_bytes(data.try_into().ok()?)), 0),
        8 => {
            let value = u64::from_be_bytes(data.try_into().ok()?);
            let nanoseconds = u32::try_from(value >> 34).ok()?;
            let seconds = i64::try_from(value & 0x3_ffff_ffff).ok()?;
            Timestamp::from_unix(seconds, nanoseconds)
        }
        12 => {
            let nanoseconds = u32::from_be_bytes(data[..4].try_into().ok()?);
            let seconds = i64::from_be_bytes(data[4..].try_into().ok()?);
            Timestamp::from_unix(seconds, nanoseconds)
        }
        _ => None,
    }
}

/// Accepts a JSON string in RFC 3339 form and, with the `stream` feature,
/// the MessagePack timestamp extension; anything else is an error.
impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(TimestampVisitor)
    }
}

/// Accepts a JSON string `YYYY-MM-DD`; anything else is an error.
impl<'de> Deserialize<'de> for Date {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(TextVisitor(std::marker::PhantomData))
    }
}

/// Written as the [`Display`](fmt::Display) text.
impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// Written as the [`Display`](fmt::Display) text.
impl Serialize for Date {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{civil_from_days, days_from_civil};

    #[test]
    fn the_two_conversions_invert_each_other_over_the_whole_range() {
        let first = days_from_civil(0, 1, 1);
        let last = days_from_civil(9999, 12, 31);
        assert_eq!(first, -719_528);
        assert_eq!(last, 2_932_896);
        for days in first..=last {
            let (year, month, day) = civil_from_days(days);
            assert_eq!(days_from_civil(year, month, day), days);
        }
    }

    #[cfg(feature = "stream")]
    #[test]
    fn each_messagepack_timestamp_form_reads_and_date_refuses_them() {
        use super::{Date, Timestamp};

        let read = |bytes: &[u8]| rmp_serde::from_slice::<Timestamp>(bytes);
        // 32-bit: seconds only.
        let mut bytes = vec![0xd6, 0xff];
        bytes.extend(1_700_000_000_u32.to_be_bytes());
        assert_eq!(
            read(&bytes).unwrap(),
            Timestamp::from_unix(1_700_000_000, 0).unwrap()
        );
        // 64-bit: nanoseconds in the upper 30 bits, seconds in the lower 34.
        let value = (123_456_789_u64 << 34) | 17_179_869_183;
        let mut bytes = vec![0xd7, 0xff];
        bytes.extend(value.to_be_bytes());
        assert_eq!(
            read(&bytes).unwrap(),
            Timestamp::from_unix(17_179_869_183, 123_456_789).unwrap()
        );
        // 96-bit: nanoseconds, then signed seconds, before the epoch here.
        let mut bytes = vec![0xc7, 12, 0xff];
        bytes.extend(999_999_999_u32.to_be_bytes());
        bytes.extend((-1_i64).to_be_bytes());
        assert_eq!(
            read(&bytes).unwrap(),
            Timestamp::from_unix(-1, 999_999_999).unwrap()
        );
        // Text still reads.
        let mut bytes = vec![0xb4];
        bytes.extend(b"2024-03-12T10:27:48Z");
        assert_eq!(read(&bytes).unwrap().to_string(), "2024-03-12T10:27:48Z");

        // Another type, a length the format does not define, nanoseconds of
        // a second, and an instant out of range are refused.
        let mut other_type = vec![0xd6, 0x01];
        other_type.extend(1_u32.to_be_bytes());
        let five_bytes = vec![0xc7, 5, 0xff, 0, 0, 0, 0, 1];
        let mut whole_second = vec![0xd7, 0xff];
        whole_second.extend((1_000_000_000_u64 << 34).to_be_bytes());
        let mut out_of_range = vec![0xc7, 12, 0xff, 0, 0, 0, 0];
        out_of_range.extend(i64::MAX.to_be_bytes());
        for bytes in [&other_type, &five_bytes, &whole_second, &out_of_range] {
            assert!(read(bytes).is_err(), "{bytes:?}");
        }

        let mut bytes = vec![0xd6, 0xff];
        bytes.extend(1_700_000_000_u32.to_be_bytes());
        assert!(rmp_serde::from_slice::<Date>(&bytes).is_err());
    }
}

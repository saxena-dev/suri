//! The market calendar.

use std::fmt;

use reqwest::Method;
use serde::Deserialize;
use serde::de::{self, Deserializer, Visitor};

use crate::rest::client::{Endpoint, Request};
use crate::value::wire::wire_enum;
use crate::{Client, Date, Error, Response};

/// A local market time of day, to the minute, with no date and no time
/// zone: Alpaca gives neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClockTime {
    hour: u8,
    minute: u8,
}

impl ClockTime {
    /// The hour, `0` to `23`.
    pub fn hour(&self) -> u8 {
        self.hour
    }

    /// The minute, `0` to `59`.
    pub fn minute(&self) -> u8 {
        self.minute
    }

    /// Reads `HHMM`, or `HH:MM` when `colon` is set: two digits each, hour
    /// `00`–`23`, minute `00`–`59`.
    fn parse(text: &str, colon: bool) -> Option<ClockTime> {
        let bytes = text.as_bytes();
        let (hour, minute) = match (colon, bytes) {
            (true, [h1, h2, b':', m1, m2]) | (false, [h1, h2, m1, m2]) => ([*h1, *h2], [*m1, *m2]),
            _ => return None,
        };
        let two = |digits: [u8; 2]| {
            digits
                .iter()
                .all(u8::is_ascii_digit)
                .then(|| (digits[0] - b'0') * 10 + (digits[1] - b'0'))
        };
        let (hour, minute) = (two(hour)?, two(minute)?);
        (hour <= 23 && minute <= 59).then_some(ClockTime { hour, minute })
    }
}

impl fmt::Display for ClockTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.hour, self.minute)
    }
}

struct ClockTimeVisitor {
    colon: bool,
}

impl Visitor<'_> for ClockTimeVisitor {
    type Value = ClockTime;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.colon {
            "a time as HH:MM"
        } else {
            "a time as HHMM"
        })
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<ClockTime, E> {
        ClockTime::parse(text, self.colon)
            .ok_or_else(|| E::invalid_value(de::Unexpected::Str(text), &self))
    }
}

/// Reads a time sent as `HH:MM`.
fn hh_mm<'de, D: Deserializer<'de>>(deserializer: D) -> Result<ClockTime, D::Error> {
    deserializer.deserialize_str(ClockTimeVisitor { colon: true })
}

/// Reads a time sent as `HHMM`.
fn hhmm<'de, D: Deserializer<'de>>(deserializer: D) -> Result<ClockTime, D::Error> {
    deserializer.deserialize_str(ClockTimeVisitor { colon: false })
}

/// A market day: its regular hours and its extended session, as local
/// market times.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CalendarDay {
    /// The date.
    pub date: Date,
    /// The time the market opens.
    #[serde(deserialize_with = "hh_mm")]
    pub open: ClockTime,
    /// The time the market closes.
    #[serde(deserialize_with = "hh_mm")]
    pub close: ClockTime,
    /// The time the extended session opens.
    #[serde(deserialize_with = "hhmm")]
    pub session_open: ClockTime,
    /// The time the extended session closes.
    #[serde(deserialize_with = "hhmm")]
    pub session_close: ClockTime,
    /// The settlement date for a trade made on this date.
    pub settlement_date: Option<Date>,
}

wire_enum! {
    /// What a calendar query's dates select.
    CalendarDateType {
        /// Trading dates.
        Trading => "TRADING",
        /// Settlement dates.
        Settlement => "SETTLEMENT",
    }
}

/// Which market days to list; every day Alpaca has unless narrowed.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CalendarQuery {
    /// The first date, inclusive.
    pub start: Option<Date>,
    /// The last date, inclusive.
    pub end: Option<Date>,
    /// Whether `start` and `end` are trading or settlement dates; trading
    /// unless set.
    pub date_type: Option<CalendarDateType>,
}

impl CalendarQuery {
    /// A query for every market day.
    pub fn new() -> CalendarQuery {
        CalendarQuery::default()
    }
}

const CALENDAR: Endpoint = Endpoint {
    method: Method::GET,
    template: "/v2/calendar",
};

impl Client {
    /// Lists market days with their hours (`GET /v2/calendar`).
    pub async fn calendar(
        &self,
        query: &CalendarQuery,
    ) -> Result<Response<Vec<CalendarDay>>, Error> {
        let mut request = Request::new(&CALENDAR, &["v2", "calendar"]);
        if let Some(start) = &query.start {
            request.pair("start", start.to_string());
        }
        if let Some(end) = &query.end {
            request.pair("end", end.to_string());
        }
        if let Some(date_type) = &query.date_type {
            request.pair("date_type", date_type.as_str());
        }
        self.send_request(request).await
    }
}

//! The market calendar: days with local market times, and the query on the
//! wire.

#![cfg(feature = "rest")]

#[path = "support/fixture.rs"]
mod fixture;
#[path = "support/server.rs"]
mod server;

use server::{Request, Server, Step, response};
use suri::trading::{CalendarDateType, CalendarDay, CalendarQuery};
use suri::{Client, Config, Credentials, Date, Environment, ErrorKind, Region};

const BROKER_CALENDAR: &str =
    "broker/broker_client/test_misc_routes::test_get_calendar::GET /v1/calendar#0";

// Built from the calendar day's per-property examples, with an unknown
// field.
const DAY: &str = r#"[{"close":"16:00","date":"2025-01-02","open":"09:30","session_close":"2000","session_open":"0400","settlement_date":"2025-01-03","unknown_member":true}]"#;

fn client(server: &Server) -> Client {
    let config = Config::new(
        Region::US,
        Environment::Paper,
        Credentials::api_key("PKSENTINELKEY0001", "SECRETSENTINELVALUE0001"),
    )
    .with_base_url(&server.url)
    .with_max_read_attempts(1);
    Client::new(config).unwrap()
}

fn date(text: &str) -> Date {
    text.parse().unwrap()
}

async fn calendar(
    body: &[u8],
    query: &CalendarQuery,
) -> (Result<Vec<CalendarDay>, suri::Error>, Request) {
    let server = Server::start(vec![Step::Respond(response("200 OK", &[], body))]);
    let result = client(&server)
        .calendar(query)
        .await
        .map(|response| response.into_parts().0);
    (result, server.requests().remove(0))
}

/// The day's times as (hour, minute) pairs: open, close, session open,
/// session close.
fn times(day: &CalendarDay) -> [(u8, u8); 4] {
    [day.open, day.close, day.session_open, day.session_close]
        .map(|time| (time.hour(), time.minute()))
}

#[tokio::test]
async fn a_day_decodes_field_by_field() {
    let (days, request) = calendar(DAY.as_bytes(), &CalendarQuery::new()).await;
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, "/v2/calendar");
    assert_eq!(request.header("Content-Type"), None);
    assert!(request.body.is_empty());
    let days = days.unwrap();
    assert_eq!(days.len(), 1);
    let day = &days[0];
    assert_eq!(day.date, date("2025-01-02"));
    assert_eq!(times(day), [(9, 30), (16, 0), (4, 0), (20, 0)]);
    assert_eq!(day.open.to_string(), "09:30");
    assert_eq!(day.close.to_string(), "16:00");
    assert_eq!(day.session_open.to_string(), "04:00");
    assert_eq!(day.session_close.to_string(), "20:00");
    assert_eq!(day.settlement_date, Some(date("2025-01-03")));
}

#[tokio::test]
async fn a_day_without_its_settlement_date_decodes() {
    let body = DAY.replace(r#","settlement_date":"2025-01-03""#, "");
    let (days, _) = calendar(body.as_bytes(), &CalendarQuery::new()).await;
    assert_eq!(days.unwrap()[0].settlement_date, None);
}

#[tokio::test]
async fn the_broker_calendar_fixture_has_the_same_shape() {
    let body = fixture::upstream(BROKER_CALENDAR).body;
    let (days, _) = calendar(&body, &CalendarQuery::new()).await;
    let days = days.unwrap();
    assert_eq!(days.len(), 2);
    for (day, expected) in days.iter().zip(["2022-05-16", "2022-05-17"]) {
        assert_eq!(day.date, date(expected));
        assert_eq!(times(day), [(9, 30), (16, 0), (4, 0), (20, 0)]);
        assert_eq!(day.settlement_date, None);
    }
}

#[tokio::test]
async fn a_time_in_another_form_fails_the_decode() {
    for (field, good, bad) in [
        ("open", "09:30", "9:30"),
        ("open", "09:30", "0930"),
        ("open", "09:30", "24:00"),
        ("close", "16:00", "16:60"),
        ("close", "16:00", "16:00:00"),
        ("close", "16:00", "+6:00"),
        ("session_open", "0400", "0960"),
        ("session_open", "0400", "04:00"),
        ("session_open", "0400", "400"),
        ("session_close", "2000", "2400"),
        ("session_close", "2000", "20000"),
    ] {
        let from = format!(r#""{field}":"{good}""#);
        let body = DAY.replace(&from, &format!(r#""{field}":"{bad}""#));
        assert_ne!(body, DAY);
        let (result, _) = calendar(body.as_bytes(), &CalendarQuery::new()).await;
        assert_eq!(
            result.unwrap_err().kind(),
            ErrorKind::Decode,
            "{field}: {bad}"
        );
    }
}

#[tokio::test]
async fn the_edges_of_the_day_decode() {
    let body = DAY
        .replace(r#""open":"09:30""#, r#""open":"00:00""#)
        .replace(r#""close":"16:00""#, r#""close":"23:59""#)
        .replace(r#""session_open":"0400""#, r#""session_open":"0000""#)
        .replace(r#""session_close":"2000""#, r#""session_close":"2359""#);
    let (days, _) = calendar(body.as_bytes(), &CalendarQuery::new()).await;
    let day = &days.unwrap()[0];
    assert_eq!(times(day), [(0, 0), (23, 59), (0, 0), (23, 59)]);
    assert_eq!(day.open.to_string(), "00:00");
    assert_eq!(day.close.to_string(), "23:59");
}

#[tokio::test]
async fn every_query_parameter_goes_on_the_wire_in_order() {
    let mut query = CalendarQuery::new();
    query.start = Some(date("2025-01-02"));
    query.end = Some(date("2025-01-31"));
    query.date_type = Some(CalendarDateType::Settlement);
    let (_, request) = calendar(b"[]", &query).await;
    assert_eq!(
        request.path,
        "/v2/calendar?start=2025-01-02&end=2025-01-31&date_type=SETTLEMENT"
    );
}

#[test]
fn every_documented_value_has_its_wire_text() {
    for (text, value) in [
        ("TRADING", CalendarDateType::Trading),
        ("SETTLEMENT", CalendarDateType::Settlement),
    ] {
        assert_eq!(text.parse::<CalendarDateType>().unwrap(), value);
        assert_eq!(value.to_string(), text);
    }
}

//! Alpaca's error codes and messages, read from error responses and scrubbed
//! of credentials.

#![cfg(feature = "rest")]

#[path = "support/server.rs"]
mod server;

use server::{Server, Step, response};
use suri::{Client, Config, Credentials, Environment, Error, ErrorKind, Region, Stage};

const KEY: &str = "PKSENTINELKEY0001";
const SECRET: &str = "SECRETSENTINELVALUE0001";

fn config(url: &str) -> Config {
    Config::new(
        Region::US,
        Environment::Paper,
        Credentials::api_key(KEY, SECRET),
    )
    .with_base_url(url)
}

/// The error for one response with this status and body.
async fn error_for(status: &str, body: &[u8]) -> Error {
    let server = Server::start(vec![Step::Respond(response(status, &[], body))]);
    let error = Client::new(config(&server.url))
        .unwrap()
        .clock()
        .await
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Api);
    assert_eq!(error.stage(), Stage::ResponseReceived);
    error
}

#[tokio::test]
async fn code_and_message_are_read_independently() {
    let rows: &[(&str, Option<u64>, Option<&str>)] = &[
        (
            r#"{"code": 42210000, "message": "m"}"#,
            Some(42210000),
            Some("m"),
        ),
        (r#"{"message": "m"}"#, None, Some("m")),
        (r#"{"code": "x", "message": "m"}"#, None, Some("m")),
        (r#"{"code": -1, "message": "m"}"#, None, Some("m")),
        (r#"{"code": 1.5, "message": "m"}"#, None, Some("m")),
        (r#"{"code": 40010001}"#, Some(40010001), None),
        (r#"{"code": 40010001, "message": 7}"#, Some(40010001), None),
    ];
    for (body, code, message) in rows {
        let error = error_for("400 Bad Request", body.as_bytes()).await;
        assert_eq!(error.code(), *code, "{body}");
        assert_eq!(error.message(), *message, "{body}");
    }
}

#[tokio::test]
async fn a_body_with_neither_is_kept_as_text() {
    let rows: &[&[u8]] = &[
        b"{}",
        br#"{"code": null, "message": null}"#,
        br#"{"error": "e"}"#,
        b"[1, 2]",
        b"42",
        b"true",
        b"null",
        b"Bad Gateway",
    ];
    for body in rows {
        let error = error_for("502 Bad Gateway", body).await;
        let text = std::str::from_utf8(body).unwrap();
        assert_eq!(error.code(), None, "{text}");
        assert_eq!(error.message(), Some(text), "{text}");
    }
    let error = error_for("502 Bad Gateway", b"\xffoops").await;
    assert_eq!(error.message(), Some("\u{fffd}oops"));
}

#[tokio::test]
async fn a_json_string_body_is_its_decoded_text() {
    let error = error_for("403 Forbidden", br#""forbidden \"here\"""#).await;
    assert_eq!(error.code(), None);
    assert_eq!(error.message(), Some(r#"forbidden "here""#));
}

#[tokio::test]
async fn an_empty_body_has_neither() {
    let error = error_for("404 Not Found", b"").await;
    assert_eq!(error.code(), None);
    assert_eq!(error.message(), None);
    assert_eq!(
        error.to_string(),
        "Alpaca returned HTTP 404 [GET /v2/clock, attempt 1]"
    );
}

#[tokio::test]
async fn display_shows_the_status_code_and_message() {
    // The body Alpaca documents for a sub-penny limit price; the status is
    // this test's choice.
    let body = br#"{"code": 42210000, "message": "invalid limit_price 290.123. sub-penny increment does not fulfill minimum pricing criteria"}"#;
    let error = error_for("422 Unprocessable Entity", body).await;
    assert_eq!(error.status(), Some(422));
    assert_eq!(
        error.to_string(),
        "Alpaca returned HTTP 422 (code 42210000): invalid limit_price 290.123. sub-penny increment does not fulfill minimum pricing criteria [GET /v2/clock, attempt 1]"
    );
    let code_only = error_for("400 Bad Request", br#"{"code": 40010001}"#).await;
    assert_eq!(
        code_only.to_string(),
        "Alpaca returned HTTP 400 (code 40010001) [GET /v2/clock, attempt 1]"
    );
    let message_only = error_for("401 Unauthorized", br#"{"message": "unauthorized."}"#).await;
    assert_eq!(
        message_only.to_string(),
        "Alpaca returned HTTP 401: unauthorized. [GET /v2/clock, attempt 1]"
    );
}

#[tokio::test]
async fn a_body_over_the_limit_leaves_an_api_error_without_details() {
    let mut body = br#"{"code": 40010001, "message": "m"}"#.to_vec();
    body.resize(1025, b' ');
    let server = Server::start(vec![Step::Respond(response("400 Bad Request", &[], &body))]);
    let client = Client::new(config(&server.url).with_max_body_bytes(1024)).unwrap();
    let error = client.clock().await.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Api);
    assert_eq!(error.status(), Some(400));
    assert_eq!(error.code(), None);
    assert_eq!(error.message(), None);
}

#[tokio::test]
async fn an_error_body_cut_short_leaves_an_api_error_without_details() {
    let head = b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{\"message\":";
    let server = Server::start(vec![Step::Partial(head.to_vec())]);
    let error = Client::new(config(&server.url))
        .unwrap()
        .clock()
        .await
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Api);
    assert_eq!(error.stage(), Stage::ResponseReceived);
    assert_eq!(error.status(), Some(503));
    assert_eq!(error.message(), None);
}

#[tokio::test]
async fn credentials_echoed_by_the_server_are_redacted() {
    let echo = format!(
        r#"{{"message": "key {KEY} with secret {SECRET} rejected; secret starts {}"}}"#,
        &SECRET[..8]
    );
    let error = error_for("403 Forbidden", echo.as_bytes()).await;
    assert_eq!(
        error.message(),
        Some("key [REDACTED] with secret [REDACTED] rejected; secret starts [REDACTED]")
    );
    let as_text = format!("bad key {KEY}");
    let plain = error_for("403 Forbidden", as_text.as_bytes()).await;
    assert_eq!(plain.message(), Some("bad key [REDACTED]"));
    for error in [&error, &plain] {
        for text in [
            error.to_string(),
            format!("{error:?}"),
            error.message().unwrap().to_owned(),
        ] {
            assert!(!text.contains(KEY) && !text.contains(SECRET), "{text}");
            assert!(!text.contains(&SECRET[..8]), "{text}");
        }
    }
}

#[tokio::test]
async fn a_message_stays_on_one_line() {
    let error = error_for(
        "400 Bad Request",
        br#"{"message": "first line\nsecond\tline\r"}"#,
    )
    .await;
    assert_eq!(error.message(), Some("first line second line "));
    assert!(!error.to_string().contains('\n'));
}

#[tokio::test]
async fn a_long_message_is_cut_at_a_character_boundary() {
    // One ASCII byte, then two-byte characters: byte 1,024 falls inside one,
    // so the cut is at 1,023.
    let long = format!("a{}", "é".repeat(2_500));
    let body = format!(r#"{{"message": "{long}"}}"#);
    let error = error_for("400 Bad Request", body.as_bytes()).await;
    let message = error.message().unwrap();
    assert_eq!(message.len(), 1023);
    assert_eq!(message, format!("a{}", "é".repeat(511)));
}

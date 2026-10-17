//! Reads are retried on throttling, server errors and failed connections,
//! with backoff, never before a rate-limit reset, and within a deadline.
//!
// real-time: every test here waits between attempts on real loopback
// sockets, which a paused clock cannot drive, and suri has no sleep hook;
// each bound keeps at least 200 ms of margin unless noted.

#![cfg(feature = "rest")]

#[path = "support/server.rs"]
mod server;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use server::{Server, Step, response};
use suri::{Client, Config, Credentials, Environment, ErrorKind, Region, Stage};

const CLOCK_BODY: &str = r#"{"timestamp":"2022-04-28T14:07:04.451420928-04:00","is_open":true,"next_open":"2022-04-29T09:30:00-04:00","next_close":"2022-04-28T16:00:00-04:00"}"#;

fn ms(milliseconds: u64) -> Duration {
    Duration::from_millis(milliseconds)
}

fn config(server: &Server) -> Config {
    Config::new(
        Region::US,
        Environment::Paper,
        Credentials::api_key("PKSENTINELKEY0001", "SECRETSENTINELVALUE0001"),
    )
    .with_base_url(&server.url)
    .with_retry_base(ms(10))
}

fn ok() -> Step {
    Step::Respond(response("200 OK", &[], CLOCK_BODY.as_bytes()))
}

fn status(line: &str, headers: &[(&str, &str)]) -> Step {
    Step::Respond(response(line, headers, b""))
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[tokio::test]
async fn a_server_error_then_success_succeeds() {
    let server = Server::start(vec![status("503 Service Unavailable", &[]), ok()]);
    let clock = Client::new(config(&server)).unwrap().clock().await;
    assert!(clock.is_ok(), "{clock:?}");
    assert_eq!(server.requests().len(), 2);
}

#[tokio::test]
async fn each_retried_status_is_retried() {
    for line in [
        "429 Too Many Requests",
        "500 Internal Server Error",
        "502 Bad Gateway",
        "503 Service Unavailable",
        "504 Gateway Timeout",
    ] {
        let server = Server::start(vec![status(line, &[]), ok()]);
        let clock = Client::new(config(&server)).unwrap().clock().await;
        assert!(clock.is_ok(), "{line}: {clock:?}");
        assert_eq!(server.requests().len(), 2, "{line}");
    }
}

#[tokio::test]
async fn other_failures_are_not_retried() {
    let too_large = {
        let mut body = CLOCK_BODY.as_bytes().to_vec();
        body.resize(1025, b' ');
        Step::Respond(response("200 OK", &[], &body))
    };
    for (step, kind) in [
        (status("400 Bad Request", &[]), ErrorKind::Api),
        (status("404 Not Found", &[]), ErrorKind::Api),
        (status("501 Not Implemented", &[]), ErrorKind::Api),
        (
            status("302 Found", &[("Location", "https://h/")]),
            ErrorKind::Redirect,
        ),
        (too_large, ErrorKind::BodyTooLarge),
        (
            Step::Respond(response("200 OK", &[], b"{}")),
            ErrorKind::Decode,
        ),
    ] {
        let server = Server::start(vec![step, ok()]);
        let client = Client::new(config(&server).with_max_body_bytes(1024)).unwrap();
        let error = client.clock().await.unwrap_err();
        assert_eq!(error.kind(), kind);
        assert_eq!(error.attempt(), 1);
        assert_eq!(server.requests().len(), 1, "{kind:?}");
    }
}

#[tokio::test]
async fn the_last_failure_is_returned_after_the_last_attempt() {
    let server = Server::start(vec![
        status("503 Service Unavailable", &[]),
        status("503 Service Unavailable", &[]),
    ]);
    let config = config(&server).with_max_read_attempts(2);
    let error = Client::new(config).unwrap().clock().await.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Api);
    assert_eq!(error.status(), Some(503));
    assert_eq!(error.attempt(), 2);
    assert_eq!(
        error.to_string(),
        "Alpaca returned HTTP 503 [GET /v2/clock, attempt 2]"
    );
    assert_eq!(server.requests().len(), 2);
}

#[tokio::test]
async fn one_attempt_means_no_retry() {
    let server = Server::start(vec![status("503 Service Unavailable", &[]), ok()]);
    let config = config(&server).with_max_read_attempts(1);
    let error = Client::new(config).unwrap().clock().await.unwrap_err();
    assert_eq!(error.attempt(), 1);
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn a_connection_closed_without_a_response_is_retried() {
    let server = Server::start(vec![Step::Drop, ok()]);
    let clock = Client::new(config(&server)).unwrap().clock().await;
    assert!(clock.is_ok(), "{clock:?}");
    assert_eq!(server.requests().len(), 2);
}

#[tokio::test]
async fn a_success_body_cut_short_is_retried() {
    let cut = b"HTTP/1.1 200 OK\r\nContent-Length: 160\r\nConnection: close\r\n\r\n{\"timestamp\":";
    let server = Server::start(vec![Step::Partial(cut.to_vec()), ok()]);
    let clock = Client::new(config(&server)).unwrap().clock().await;
    assert!(clock.is_ok(), "{clock:?}");
    assert_eq!(server.requests().len(), 2);
}

/// Timeouts of 1 s and a 5 s deadline, so a wait of up to 2 s fits.
fn reset_config(server: &Server) -> Config {
    config(server)
        .with_connect_timeout(Duration::from_secs(1))
        .with_request_timeout(Duration::from_secs(1))
        .with_read_deadline(Duration::from_secs(5))
}

#[tokio::test]
async fn a_throttled_read_waits_for_the_rate_limit_reset() {
    // The current second plus 2 is at least a full second ahead.
    let reset = (unix_now() + 2).to_string();
    let server = Server::start(vec![
        status(
            "429 Too Many Requests",
            &[
                ("X-RateLimit-Limit", "200"),
                ("X-RateLimit-Remaining", "0"),
                ("X-RateLimit-Reset", &reset),
            ],
        ),
        ok(),
    ]);
    let clock = Client::new(reset_config(&server)).unwrap().clock().await;
    assert!(clock.is_ok(), "{clock:?}");
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    let reset_at = UNIX_EPOCH + Duration::from_secs(reset.parse().unwrap());
    assert!(requests[1].wall >= reset_at);
}

#[tokio::test]
async fn a_reset_sent_alone_is_still_waited_for() {
    let reset = (unix_now() + 2).to_string();
    let server = Server::start(vec![
        status("429 Too Many Requests", &[("X-RateLimit-Reset", &reset)]),
        ok(),
    ]);
    let clock = Client::new(reset_config(&server)).unwrap().clock().await;
    assert!(clock.is_ok(), "{clock:?}");
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    let reset_at = UNIX_EPOCH + Duration::from_secs(reset.parse().unwrap());
    assert!(requests[1].wall >= reset_at);
}

#[tokio::test]
async fn a_passed_reset_leaves_the_backoff_with_added_jitter() {
    let reset = (unix_now() - 60).to_string();
    let server = Server::start(vec![
        status("429 Too Many Requests", &[("X-RateLimit-Reset", &reset)]),
        ok(),
    ]);
    let config = reset_config(&server).with_retry_base(ms(100));
    let clock = Client::new(config).unwrap().clock().await;
    assert!(clock.is_ok(), "{clock:?}");
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    // The backoff of 100 ms plus up to 50 ms of jitter, never less.
    let gap = requests[1].at - requests[0].at;
    assert!(gap >= ms(100), "{gap:?}");
    assert!(gap < ms(150 + 200), "{gap:?}");
}

#[tokio::test]
async fn a_reset_beyond_the_deadline_returns_the_throttling_at_once() {
    let reset = (unix_now() + 30).to_string();
    let server = Server::start(vec![
        status("429 Too Many Requests", &[("X-RateLimit-Reset", &reset)]),
        ok(),
    ]);
    let started = Instant::now();
    let error = Client::new(reset_config(&server))
        .unwrap()
        .clock()
        .await
        .unwrap_err();
    assert!(started.elapsed() < ms(1000));
    assert_eq!(error.status(), Some(429));
    assert_eq!(error.attempt(), 1);
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn attempts_stop_at_the_read_deadline() {
    // Attempts of 100 ms with waits of 10 and 20 ms: the third starts about
    // 230 ms in and is cut at the 300 ms deadline; a fourth would not fit.
    // The end bound keeps 200 ms of margin.
    let server = Server::start(vec![Step::Hold, Step::Hold, Step::Hold, Step::Hold]);
    let config = config(&server)
        .with_connect_timeout(ms(50))
        .with_request_timeout(ms(100))
        .with_read_deadline(ms(300));
    let started = Instant::now();
    let error = Client::new(config).unwrap().clock().await.unwrap_err();
    let elapsed = started.elapsed();
    assert!(elapsed < ms(300 + 200), "{elapsed:?}");
    assert_eq!(server.requests().len(), 3);
    assert_eq!(error.kind(), ErrorKind::Timeout);
    assert_eq!(error.stage(), Stage::Unknown);
    assert_eq!(error.attempt(), 3);
    assert_eq!(
        error.to_string(),
        "no response within request_timeout or read_deadline [GET /v2/clock, attempt 3]"
    );
}

#[tokio::test]
async fn the_last_attempt_is_cut_to_the_remaining_deadline() {
    // The first attempt times out at 1.5 s; the second starts about 1.51 s
    // in, about 490 ms before the 2 s deadline, and is cut there, where an
    // uncut attempt would run to about 3 s.
    let server = Server::start(vec![Step::Hold, Step::Hold, Step::Hold]);
    let config = config(&server)
        .with_connect_timeout(ms(500))
        .with_request_timeout(ms(1500))
        .with_read_deadline(ms(2000));
    let started = Instant::now();
    let error = Client::new(config).unwrap().clock().await.unwrap_err();
    let elapsed = started.elapsed();
    assert!(elapsed >= ms(2000), "{elapsed:?}");
    assert!(elapsed < ms(2400), "{elapsed:?}");
    assert_eq!(server.requests().len(), 2);
    assert_eq!(error.attempt(), 2);
    assert_eq!(error.kind(), ErrorKind::Timeout);
}

#[test]
fn retry_policy_values_are_checked_against_their_bounds() {
    let s = Duration::from_secs;
    let server = Server::start(vec![]);
    let base = || config(&server);
    let short = "read_deadline must not be shorter than request_timeout";
    let retry_base = "retry_base must be between 1ms and 10s";
    let cap = "retry_cap must be between 1ms and 60s";
    let attempts = "max_read_attempts must be between 1 and 10";
    for (config, message) in [
        (base().with_read_deadline(s(29)), short),
        (
            base().with_read_deadline(s(3601)),
            "read_deadline must be at most 1h",
        ),
        (base().with_retry_base(ms(0)), retry_base),
        (
            base().with_retry_base(s(11)).with_retry_cap(s(20)),
            retry_base,
        ),
        (base().with_retry_base(ms(1)).with_retry_cap(ms(0)), cap),
        (base().with_retry_cap(s(61)), cap),
        (
            base().with_retry_base(s(2)).with_retry_cap(s(1)),
            "retry_cap must not be smaller than retry_base",
        ),
        (base().with_max_read_attempts(0), attempts),
        (base().with_max_read_attempts(11), attempts),
        // Table order: the deadline is checked before the retry values.
        (
            base().with_read_deadline(s(1)).with_max_read_attempts(0),
            short,
        ),
    ] {
        assert_eq!(Client::new(config).unwrap_err().to_string(), message);
    }
    let lowest = config(&server)
        .with_read_deadline(s(30))
        .with_retry_base(ms(1))
        .with_retry_cap(ms(1))
        .with_max_read_attempts(1);
    let highest = config(&server)
        .with_read_deadline(s(3600))
        .with_retry_base(s(10))
        .with_retry_cap(s(60))
        .with_max_read_attempts(10);
    assert!(Client::new(lowest).is_ok());
    assert!(Client::new(highest).is_ok());
}

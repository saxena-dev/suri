//! A request that changes state reaches the server once, whatever goes
//! wrong: suri never retries it, while a read in the same situation is
//! retried.

#![cfg(feature = "rest")]

#[path = "support/server.rs"]
mod server;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use server::{Server, Step, refused_url, response};
use suri::trading::ConfigurationsUpdate;
use suri::{Client, Config, Credentials, Environment, ErrorKind, Region};

const CONFIGURATIONS: &[u8] = br#"{"suspend_trade":true}"#;

fn config(url: &str) -> Config {
    // The defaults allow four attempts within 60 seconds, so a read would be
    // retried in every case below.
    Config::new(
        Region::US,
        Environment::Paper,
        Credentials::api_key("PKSENTINELKEY0001", "SECRETSENTINELVALUE0001"),
    )
    .with_base_url(url)
    .with_retry_base(Duration::from_millis(10))
}

fn update() -> ConfigurationsUpdate {
    let mut update = ConfigurationsUpdate::new();
    update.suspend_trade = Some(true);
    update
}

/// Runs one configuration update against `first` followed by a spare
/// success, and returns the error and the number of requests received.
async fn one_update(first: Step, adjust: impl FnOnce(Config) -> Config) -> (suri::Error, usize) {
    let server = Server::start(vec![
        first,
        Step::Respond(response("200 OK", &[], CONFIGURATIONS)),
    ]);
    let client = Client::new(adjust(config(&server.url))).unwrap();
    let error = client
        .update_account_configurations(&update())
        .await
        .unwrap_err();
    (error, server.requests().len())
}

#[tokio::test]
async fn a_server_error_is_not_retried() {
    let (error, requests) = one_update(
        Step::Respond(response("503 Service Unavailable", &[], b"")),
        |c| c,
    )
    .await;
    assert_eq!(requests, 1);
    assert_eq!(error.kind(), ErrorKind::Api);
    assert_eq!(error.status(), Some(503));
    assert_eq!(error.attempt(), 1);
}

#[tokio::test]
async fn throttling_is_not_retried() {
    let reset = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 1)
    .to_string();
    let (error, requests) = one_update(
        Step::Respond(response(
            "429 Too Many Requests",
            &[("X-RateLimit-Reset", &reset)],
            b"",
        )),
        |c| c,
    )
    .await;
    assert_eq!(requests, 1);
    assert_eq!(error.status(), Some(429));
    assert_eq!(error.attempt(), 1);
}

#[tokio::test]
async fn a_dropped_connection_is_not_retried() {
    let (error, requests) = one_update(Step::Drop, |c| c).await;
    assert_eq!(requests, 1);
    assert_eq!(error.kind(), ErrorKind::Transport);
    assert!(error.may_have_reached_broker());
    assert_eq!(error.attempt(), 1);
}

// real-time: reqwest's request timeout runs on a real socket, which a paused
// clock cannot drive; a read would retry well inside the 60 s deadline.
#[tokio::test]
async fn a_timeout_is_not_retried() {
    let (error, requests) = one_update(Step::Hold, |c| {
        c.with_connect_timeout(Duration::from_secs(1))
            .with_request_timeout(Duration::from_secs(1))
    })
    .await;
    assert_eq!(requests, 1);
    assert_eq!(error.kind(), ErrorKind::Timeout);
    assert_eq!(error.attempt(), 1);
}

#[tokio::test]
async fn a_redirect_is_not_followed() {
    let (error, requests) = one_update(
        Step::Respond(response(
            "307 Temporary Redirect",
            &[("Location", "https://h/v2/account/configurations")],
            b"",
        )),
        |c| c,
    )
    .await;
    assert_eq!(requests, 1);
    assert_eq!(error.kind(), ErrorKind::Redirect);
    assert_eq!(error.attempt(), 1);
}

#[tokio::test]
async fn a_refused_connection_is_one_attempt() {
    let client = Client::new(config(&refused_url())).unwrap();
    let error = client
        .update_account_configurations(&update())
        .await
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Transport);
    assert!(!error.may_have_reached_broker());
    assert_eq!(error.attempt(), 1);
}

#[tokio::test]
async fn a_read_in_the_same_situation_is_retried() {
    let server = Server::start(vec![
        Step::Respond(response("503 Service Unavailable", &[], b"")),
        Step::Respond(response("200 OK", &[], br#"{"id":"a","status":"ACTIVE"}"#)),
    ]);
    let client = Client::new(config(&server.url)).unwrap();
    assert!(client.account().await.is_ok());
    assert_eq!(server.requests().len(), 2);
}

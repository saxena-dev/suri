//! The market clock, end to end against a loopback server: the request suri
//! sends, the response it reads, and every way the call can fail.

#![cfg(feature = "rest")]

#[path = "support/fixture.rs"]
mod fixture;
#[path = "support/server.rs"]
mod server;

use std::time::Duration;

use server::{Server, Step, refused_url, response};
use suri::{Client, Config, Credentials, Environment, Error, ErrorKind, Region, Stage, Timestamp};

const KEY: &str = "PKSENTINELKEY0001";
const SECRET: &str = "SECRETSENTINELVALUE0001";
const CLOCK_ID: &str = "common/test_rest::test_request_sends_user_agent_header::GET /v2/clock#0";
const CLOCK_BODY: &str = r#"{"timestamp":"2022-04-28T14:07:04.451420928-04:00","is_open":true,"next_open":"2022-04-29T09:30:00-04:00","next_close":"2022-04-28T16:00:00-04:00"}"#;

fn config(url: &str) -> Config {
    Config::new(
        Region::US,
        Environment::Paper,
        Credentials::api_key(KEY, SECRET),
    )
    .with_base_url(url)
    // One attempt: these tests name the outcome of a single request.
    .with_max_read_attempts(1)
}

fn client(url: &str) -> Client {
    Client::new(config(url)).unwrap()
}

fn at(text: &str) -> Timestamp {
    text.parse().unwrap()
}

async fn fail(server: &Server, adjust: impl FnOnce(Config) -> Config) -> Error {
    let client = Client::new(adjust(config(&server.url))).unwrap();
    client.clock().await.unwrap_err()
}

#[tokio::test]
async fn the_upstream_clock_fixture_decodes_to_the_documented_instants() {
    let fixture = fixture::upstream(CLOCK_ID);
    let server = Server::start(vec![Step::Respond(response("200 OK", &[], &fixture.body))]);
    let clock = client(&server.url).clock().await.unwrap();
    let clock = clock.data();
    assert_eq!(clock.timestamp, at("2022-04-28T18:07:04.451420928Z"));
    assert!(clock.is_open);
    assert_eq!(clock.next_open, at("2022-04-29T13:30:00Z"));
    assert_eq!(clock.next_close, at("2022-04-28T20:00:00Z"));
}

#[tokio::test]
async fn unknown_fields_are_ignored() {
    let body = r#"{"timestamp":"2022-04-28T14:07:04.451420928-04:00","is_open":true,"next_open":"2022-04-29T09:30:00-04:00","next_close":"2022-04-28T16:00:00-04:00","phase":"core"}"#;
    let server = Server::start(vec![
        Step::Respond(response("200 OK", &[], CLOCK_BODY.as_bytes())),
        Step::Respond(response("200 OK", &[], body.as_bytes())),
    ]);
    let client = client(&server.url);
    let plain = client.clock().await.unwrap().into_parts().0;
    let extra = client.clock().await.unwrap().into_parts().0;
    assert_eq!(plain, extra);
}

#[tokio::test]
async fn the_request_carries_the_key_pair_user_agent_and_identity_encoding() {
    let server = Server::start(vec![Step::Respond(response(
        "200 OK",
        &[],
        CLOCK_BODY.as_bytes(),
    ))]);
    client(&server.url).clock().await.unwrap();
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, "/v2/clock");
    assert_eq!(request.header("APCA-API-KEY-ID"), Some(KEY));
    assert_eq!(request.header("APCA-API-SECRET-KEY"), Some(SECRET));
    assert_eq!(
        request.header("User-Agent"),
        Some(format!("suri/{}", env!("CARGO_PKG_VERSION")).as_str())
    );
    assert_eq!(request.header("Accept-Encoding"), Some("identity"));
    assert!(request.body.is_empty());
}

#[test]
fn debug_output_never_holds_a_credential() {
    let config = config("http://127.0.0.1:1");
    let client = Client::new(config.clone()).unwrap();
    let in_url = Config::new(
        Region::US,
        Environment::Live,
        Credentials::api_key(KEY, SECRET),
    )
    .with_base_url(format!("https://{KEY}:{SECRET}@api.alpaca.markets"));
    let error = Client::new(in_url.clone()).unwrap_err();
    for text in [
        format!("{:?}", Credentials::api_key(KEY, SECRET)),
        format!("{config:?}"),
        format!("{client:?}"),
        format!("{in_url:?}"),
        format!("{error:?}"),
        error.to_string(),
    ] {
        assert!(!text.contains(KEY) && !text.contains(SECRET), "{text}");
    }
    assert_eq!(
        format!("{:?}", Credentials::api_key(KEY, SECRET)),
        "Credentials { .. }"
    );
    assert_eq!(format!("{client:?}"), "Client { .. }");
    assert!(format!("{in_url:?}").contains("base_url: Some(..)"));
}

#[tokio::test]
async fn metadata_holds_the_request_id_and_rate_limit() {
    let body = CLOCK_BODY.as_bytes();
    let limits = [
        ("X-RateLimit-Limit", "200"),
        ("X-RateLimit-Remaining", "199"),
        ("X-RateLimit-Reset", "1700000000"),
    ];
    let mut with_id = limits.to_vec();
    with_id.push(("X-Request-ID", "req-0001"));
    let server = Server::start(vec![
        Step::Respond(response("200 OK", &with_id, body)),
        Step::Respond(response("200 OK", &limits[..2], body)),
        Step::Respond(response(
            "200 OK",
            &[limits[0], limits[1], ("X-RateLimit-Reset", "soon")],
            body,
        )),
    ]);
    let client = client(&server.url);

    let first = client.clock().await.unwrap();
    let meta = first.meta();
    assert_eq!(meta.status(), 200);
    assert_eq!(meta.request_id(), Some("req-0001"));
    let rate_limit = meta.rate_limit().unwrap();
    assert_eq!(rate_limit.limit(), 200);
    assert_eq!(rate_limit.remaining(), 199);
    assert_eq!(rate_limit.reset(), at("2023-11-14T22:13:20Z"));

    let missing = client.clock().await.unwrap();
    assert_eq!(missing.meta().request_id(), None);
    assert_eq!(missing.meta().rate_limit(), None);

    let not_a_number = client.clock().await.unwrap();
    assert_eq!(not_a_number.meta().rate_limit(), None);
}

#[tokio::test]
async fn an_error_status_is_an_api_error_with_its_metadata() {
    let server = Server::start(vec![Step::Respond(response(
        "500 Internal Server Error",
        &[("X-Request-ID", "req-0500")],
        b"{\"message\":\"internal server error\"}",
    ))]);
    let error = fail(&server, |c| c).await;
    assert_eq!(error.kind(), ErrorKind::Api);
    assert_eq!(error.stage(), Stage::ResponseReceived);
    assert!(error.may_have_reached_broker());
    assert_eq!(error.status(), Some(500));
    assert_eq!(error.meta().unwrap().request_id(), Some("req-0500"));
    assert_eq!(error.method(), "GET");
    assert_eq!(error.endpoint(), "/v2/clock");
    assert_eq!(error.attempt(), 1);
    assert_eq!(
        error.to_string(),
        "Alpaca returned HTTP 500: internal server error [GET /v2/clock, attempt 1] [request id req-0500]"
    );
}

#[tokio::test]
async fn a_redirect_is_never_followed_and_names_only_its_origin() {
    let server = Server::start(vec![
        Step::Respond(response(
            "302 Found",
            &[("Location", "https://u:p@h:8443/x?q")],
            b"",
        )),
        Step::Respond(response(
            "301 Moved Permanently",
            &[("Location", "https://u:p@h/x?q")],
            b"",
        )),
        Step::Respond(response("302 Found", &[("Location", "/v2/clock")], b"")),
        Step::Respond(response("302 Found", &[], b"")),
    ]);
    let client = client(&server.url);

    let absolute = client.clock().await.unwrap_err();
    assert_eq!(absolute.kind(), ErrorKind::Redirect);
    assert_eq!(absolute.stage(), Stage::ResponseReceived);
    assert_eq!(absolute.location(), Some("https://h:8443"));
    assert_eq!(
        absolute.to_string(),
        "Alpaca answered HTTP 302 with a redirect to https://h:8443; it was not followed [GET /v2/clock, attempt 1]"
    );
    let default_port = client.clock().await.unwrap_err();
    assert_eq!(default_port.location(), Some("https://h"));
    assert_eq!(
        default_port.to_string(),
        "Alpaca answered HTTP 301 with a redirect to https://h; it was not followed [GET /v2/clock, attempt 1]"
    );
    for error in [
        client.clock().await.unwrap_err(),
        client.clock().await.unwrap_err(),
    ] {
        assert_eq!(error.location(), None);
        assert_eq!(
            error.to_string(),
            "Alpaca answered HTTP 302 with a redirect; it was not followed [GET /v2/clock, attempt 1]"
        );
    }
    assert_eq!(server.requests().len(), 4);
}

#[tokio::test]
async fn a_refused_connection_never_reached_alpaca() {
    let error = client(&refused_url()).clock().await.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Transport);
    assert_eq!(error.stage(), Stage::NotStarted);
    assert!(!error.may_have_reached_broker());
    assert_eq!(error.status(), None);
    assert!(error.meta().is_none());
    assert_eq!(
        error.to_string(),
        "the connection could not be established [GET /v2/clock, attempt 1]"
    );
}

// real-time: the connect timeout runs inside reqwest's connector on real
// sockets, which a paused clock cannot drive; the handshake never completes,
// so the outcome does not depend on the margin.
#[tokio::test]
async fn a_handshake_that_never_completes_is_a_connect_timeout() {
    let server = Server::start(vec![Step::Hold]);
    let url = server.url.replace("http://", "https://");
    let config = config(&url).with_connect_timeout(Duration::from_millis(50));
    let started = std::time::Instant::now();
    let error = Client::new(config).unwrap().clock().await.unwrap_err();
    // The configured 50 ms applies, not the 10 s default.
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(error.kind(), ErrorKind::Timeout);
    assert_eq!(error.stage(), Stage::NotStarted);
    assert!(!error.may_have_reached_broker());
    assert_eq!(
        error.to_string(),
        "the connection was not established within connect_timeout [GET /v2/clock, attempt 1]"
    );
}

#[tokio::test]
async fn a_connection_closed_before_any_response_may_have_reached_alpaca() {
    let server = Server::start(vec![Step::Drop]);
    let error = fail(&server, |c| c).await;
    assert_eq!(error.kind(), ErrorKind::Transport);
    assert_eq!(error.stage(), Stage::Unknown);
    assert!(error.may_have_reached_broker());
    assert_eq!(
        error.to_string(),
        "the connection failed before a response arrived [GET /v2/clock, attempt 1]"
    );
}

const CUT_BODY: &[u8] =
    b"HTTP/1.1 200 OK\r\nContent-Length: 160\r\nConnection: close\r\n\r\n{\"timestamp\":";

#[tokio::test]
async fn a_body_cut_short_fails_after_the_response_arrived() {
    let server = Server::start(vec![Step::Partial(CUT_BODY.to_vec())]);
    let error = fail(&server, |c| c).await;
    assert_eq!(error.kind(), ErrorKind::Transport);
    assert_eq!(error.stage(), Stage::ResponseReceived);
    assert_eq!(error.status(), Some(200));
    assert_eq!(
        error.to_string(),
        "the connection failed while the response was read [GET /v2/clock, attempt 1]"
    );
}

// real-time: reqwest's request timeout runs on real sockets, which a paused
// clock cannot drive; a loopback connect has 500 ms of margin.
#[tokio::test]
async fn no_response_within_the_request_timeout() {
    let server = Server::start(vec![Step::Hold]);
    let error = fail(&server, |c| {
        c.with_connect_timeout(Duration::from_millis(500))
            .with_request_timeout(Duration::from_millis(500))
    })
    .await;
    assert_eq!(error.kind(), ErrorKind::Timeout);
    assert_eq!(error.stage(), Stage::Unknown);
    assert!(error.may_have_reached_broker());
    assert_eq!(
        error.to_string(),
        "no response within request_timeout or read_deadline [GET /v2/clock, attempt 1]"
    );
}

// real-time: as above; the response head has 500 ms of margin to arrive.
#[tokio::test]
async fn a_body_still_arriving_at_the_request_timeout() {
    let server = Server::start(vec![Step::PartialHold(CUT_BODY.to_vec())]);
    let error = fail(&server, |c| {
        c.with_connect_timeout(Duration::from_millis(500))
            .with_request_timeout(Duration::from_millis(500))
    })
    .await;
    assert_eq!(error.kind(), ErrorKind::Timeout);
    assert_eq!(error.stage(), Stage::ResponseReceived);
    assert_eq!(error.status(), Some(200));
    assert_eq!(
        error.to_string(),
        "the response was not complete within request_timeout or read_deadline [GET /v2/clock, attempt 1]"
    );
}

#[tokio::test]
async fn a_body_over_the_limit_is_not_read() {
    // The clock body padded with trailing spaces to exactly 1 KiB, then one
    // byte more.
    let padded = |length: usize| {
        let mut body = CLOCK_BODY.as_bytes().to_vec();
        body.resize(length, b' ');
        response("200 OK", &[], &body)
    };
    let server = Server::start(vec![
        Step::Respond(padded(1024)),
        Step::Respond(padded(1025)),
    ]);
    let client = Client::new(config(&server.url).with_max_body_bytes(1024)).unwrap();
    assert!(client.clock().await.is_ok());
    let error = client.clock().await.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::BodyTooLarge);
    assert_eq!(error.stage(), Stage::ResponseReceived);
    assert_eq!(error.status(), Some(200));
    assert_eq!(
        error.to_string(),
        "the response body exceeded max_body_bytes [GET /v2/clock, attempt 1]"
    );
}

#[tokio::test]
async fn a_body_without_a_required_field_does_not_decode() {
    let body = r#"{"timestamp":"2022-04-28T14:07:04-04:00","next_open":"2022-04-29T09:30:00-04:00","next_close":"2022-04-28T16:00:00-04:00"}"#;
    let server = Server::start(vec![Step::Respond(response(
        "200 OK",
        &[],
        body.as_bytes(),
    ))]);
    let error = fail(&server, |c| c).await;
    assert_eq!(error.kind(), ErrorKind::Decode);
    assert_eq!(error.stage(), Stage::ResponseReceived);
    assert_eq!(error.status(), Some(200));
    assert_eq!(
        error.to_string(),
        "the response did not match the documented shape [GET /v2/clock, attempt 1]"
    );
}

fn config_error(config: Config) -> String {
    Client::new(config).unwrap_err().to_string()
}

#[test]
fn credentials_must_be_non_empty_printable_ascii() {
    let with = |key: &str, secret: &str| {
        Config::new(
            Region::US,
            Environment::Paper,
            Credentials::api_key(key, secret),
        )
    };
    assert_eq!(
        config_error(with("", SECRET)),
        "credentials must not be empty"
    );
    assert_eq!(config_error(with(KEY, "")), "credentials must not be empty");
    let ascii = "credentials must be printable ASCII";
    assert_eq!(config_error(with("PKKEY\n", SECRET)), ascii);
    assert_eq!(config_error(with(KEY, "SEC RET")), ascii);
    assert_eq!(config_error(with(KEY, "SECRÉT")), ascii);
    // Credentials are checked before the base URL.
    assert_eq!(
        config_error(with("", SECRET).with_base_url("not a url")),
        "credentials must not be empty"
    );
}

#[test]
fn the_base_url_must_be_https_or_loopback_with_nothing_after_the_host() {
    for (url, message) in [
        ("not a url", "base URL is not a valid URL"),
        (
            "http://example.com",
            "base URL must use https unless the host is loopback",
        ),
        (
            "ftp://127.0.0.1",
            "base URL must use https unless the host is loopback",
        ),
        (
            "https://h/v2",
            "base URL must not have a path, query or fragment",
        ),
        (
            "https://h/?q",
            "base URL must not have a path, query or fragment",
        ),
        (
            "https://h/#f",
            "base URL must not have a path, query or fragment",
        ),
        ("https://u:p@h", "base URL must not contain credentials"),
    ] {
        assert_eq!(config_error(config(url)), message, "{url}");
    }
    for url in [
        "http://127.0.0.1:1",
        "http://127.9.9.9:1/",
        "http://localhost:1",
        "http://[::1]:1",
        "https://h",
        "https://h:8443/",
    ] {
        assert!(Client::new(config(url)).is_ok(), "{url}");
    }
}

#[test]
fn policy_values_are_checked_against_their_bounds_in_order() {
    let ms = Duration::from_millis;
    let s = Duration::from_secs;
    let base = || config("http://127.0.0.1:1");
    let connect = "connect_timeout must be between 1ms and 60s";
    let request = "request_timeout must be between 1ms and 10min";
    let body = "max_body_bytes must be between 1KiB and 1GiB";
    for (config, message) in [
        (base().with_connect_timeout(ms(0)), connect),
        (
            base()
                .with_connect_timeout(s(61))
                .with_request_timeout(s(120)),
            connect,
        ),
        (base().with_request_timeout(ms(0)), request),
        (base().with_request_timeout(s(601)), request),
        (
            base()
                .with_connect_timeout(ms(200))
                .with_request_timeout(ms(100)),
            "connect_timeout must not be longer than request_timeout",
        ),
        (base().with_max_body_bytes(1023), body),
        (base().with_max_body_bytes((1 << 30) + 1), body),
        // The first failure in table order is reported.
        (
            base().with_connect_timeout(ms(0)).with_max_body_bytes(0),
            connect,
        ),
    ] {
        assert_eq!(config_error(config), message);
    }
    for config in [
        base()
            .with_connect_timeout(ms(1))
            .with_request_timeout(ms(1))
            .with_max_body_bytes(1024),
        base()
            .with_connect_timeout(s(60))
            .with_request_timeout(s(600))
            .with_read_deadline(s(600))
            .with_max_body_bytes(1 << 30),
    ] {
        assert!(Client::new(config).is_ok());
    }
}

/// Re-run in a child process with Alpaca's and the proxy environment
/// variables pointing at a decoy server: suri reads none of them.
#[tokio::test]
async fn no_environment_variable_is_read() {
    const CHILD: &str = "SURI_TEST_ENVIRONMENT_CHILD";
    if let Some(url) = std::env::var_os(CHILD) {
        let clock = client(url.to_str().unwrap()).clock().await;
        assert!(clock.is_ok(), "{clock:?}");
        return;
    }
    let decoy = Server::start(vec![Step::Respond(response(
        "200 OK",
        &[],
        CLOCK_BODY.as_bytes(),
    ))]);
    let server = Server::start(vec![Step::Respond(response(
        "200 OK",
        &[],
        CLOCK_BODY.as_bytes(),
    ))]);
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["no_environment_variable_is_read", "--exact"])
        .env(CHILD, &server.url)
        .env("APCA_API_KEY_ID", "DECOYKEY")
        .env("APCA_API_SECRET_KEY", "DECOYSECRET")
        .env("APCA_API_BASE_URL", &decoy.url)
        .env("HTTP_PROXY", &decoy.url)
        .env("HTTPS_PROXY", &decoy.url)
        .env("ALL_PROXY", &decoy.url)
        .env("http_proxy", &decoy.url)
        .env("https_proxy", &decoy.url)
        .env("all_proxy", &decoy.url)
        .env_remove("NO_PROXY")
        .env_remove("no_proxy")
        .status()
        .unwrap();
    assert!(status.success());
    assert!(decoy.requests().is_empty());
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].header("APCA-API-KEY-ID"), Some(KEY));
}

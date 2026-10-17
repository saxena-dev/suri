//! Each call is one `suri.request` span with documented fields, and emits
//! documented `retry` and `error` events. One test in its own binary: it
//! installs a global subscriber.

#![cfg(feature = "rest")]

#[path = "support/server.rs"]
mod server;

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;
use std::time::Duration;

use server::{Server, Step, refused_url, response};
use suri::{Client, Config, Credentials, Environment, Region};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;

const KEY: &str = "PKSENTINELKEY0001";
const SECRET: &str = "SECRETSENTINELVALUE0001";
const CLOCK_BODY: &str = r#"{"timestamp":"2022-04-28T14:07:04.451420928-04:00","is_open":true,"next_open":"2022-04-29T09:30:00-04:00","next_close":"2022-04-28T16:00:00-04:00"}"#;

type Fields = Vec<(String, String)>;

#[derive(Debug)]
struct SpanRecord {
    name: &'static str,
    level: Level,
    /// The fields given at creation, then each recorded value, in order.
    fields: Fields,
}

#[derive(Debug)]
struct EventRecord {
    name: &'static str,
    level: Level,
    /// The index of the suri span the event happened in.
    span: Option<usize>,
    fields: Fields,
}

#[derive(Default)]
struct Captured {
    spans: Vec<SpanRecord>,
    /// The span record each live span ID belongs to; the registry reuses a
    /// closed span's ID, so this maps to the latest span created with it.
    live: HashMap<u64, usize>,
    events: Vec<EventRecord>,
}

static CAPTURED: Mutex<Option<Captured>> = Mutex::new(None);

struct Collect<'a>(&'a mut Fields);

impl Visit for Collect<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.push((field.name().to_owned(), value.to_owned()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.0.push((field.name().to_owned(), value.to_string()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.0.push((field.name().to_owned(), format!("{value:?}")));
    }
}

/// Captures suri's own spans and events (target `suri`).
struct Capture;

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Capture {
    fn on_new_span(&self, attributes: &Attributes<'_>, id: &Id, _: Context<'_, S>) {
        if attributes.metadata().target() != "suri" {
            return;
        }
        let mut fields = Vec::new();
        attributes.record(&mut Collect(&mut fields));
        let mut guard = CAPTURED.lock().unwrap();
        let captured = guard.get_or_insert_with(Captured::default);
        captured.spans.push(SpanRecord {
            name: attributes.metadata().name(),
            level: *attributes.metadata().level(),
            fields,
        });
        let index = captured.spans.len() - 1;
        captured.live.insert(id.into_u64(), index);
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, _: Context<'_, S>) {
        let mut guard = CAPTURED.lock().unwrap();
        let captured = guard.get_or_insert_with(Captured::default);
        if let Some(&index) = captured.live.get(&id.into_u64()) {
            values.record(&mut Collect(&mut captured.spans[index].fields));
        }
    }

    fn on_event(&self, event: &Event<'_>, context: Context<'_, S>) {
        if event.metadata().target() != "suri" {
            return;
        }
        let mut fields = Vec::new();
        event.record(&mut Collect(&mut fields));
        let mut guard = CAPTURED.lock().unwrap();
        let captured = guard.get_or_insert_with(Captured::default);
        let span = context
            .event_span(event)
            .and_then(|span| captured.live.get(&span.id().into_u64()).copied());
        captured.events.push(EventRecord {
            name: event.metadata().name(),
            level: *event.metadata().level(),
            span,
            fields,
        });
    }

    fn on_close(&self, id: Id, _: Context<'_, S>) {
        let mut guard = CAPTURED.lock().unwrap();
        if let Some(captured) = guard.as_mut() {
            captured.live.remove(&id.into_u64());
        }
    }
}

fn pairs(fields: &[(&str, &str)]) -> Fields {
    fields
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

/// The fields every span starts with.
fn opening() -> Fields {
    pairs(&[("method", "GET"), ("endpoint", "/v2/clock")])
}

fn span_fields(recorded: &[(&str, &str)]) -> Fields {
    let mut fields = opening();
    fields.extend(pairs(recorded));
    fields
}

#[tokio::test]
async fn each_call_is_one_span_with_documented_fields_and_events() {
    tracing::subscriber::set_global_default(tracing_subscriber::registry().with(Capture)).unwrap();
    let mut too_large = CLOCK_BODY.as_bytes().to_vec();
    too_large.resize(1100, b' ');
    let server = Server::start(vec![
        Step::Respond(response("503 Service Unavailable", &[], b"")),
        Step::Respond(response("200 OK", &[], CLOCK_BODY.as_bytes())),
        Step::Respond(response("302 Found", &[("Location", "https://h/x")], b"")),
        Step::Drop,
        Step::Respond(response("429 Too Many Requests", &[], b"")),
        Step::Respond(response("404 Not Found", &[], b"")),
        Step::Respond(response("200 OK", &[], b"{}")),
        Step::Respond(response("200 OK", &[], &too_large)),
        Step::Hold,
        Step::Hold,
    ]);
    let ms = Duration::from_millis;
    let config = |url: &str| {
        Config::new(
            Region::US,
            Environment::Paper,
            Credentials::api_key(KEY, SECRET),
        )
        .with_base_url(url)
        .with_retry_base(ms(10))
        .with_max_body_bytes(1024)
    };
    let retrying = Client::new(config(&server.url)).unwrap();
    let once = Client::new(config(&server.url).with_max_read_attempts(1)).unwrap();
    // real-time: the deadline path needs reqwest's timeouts on real sockets.
    // Attempts of 50 ms with waits of at least 100 and 200 ms: the first
    // wait ends by 200 ms, 200 ms before the 400 ms deadline; the second
    // would end at or after it, so the call ends there.
    let deadline = Client::new(
        config(&server.url)
            .with_connect_timeout(ms(50))
            .with_request_timeout(ms(50))
            .with_retry_base(ms(100))
            .with_read_deadline(ms(400)),
    )
    .unwrap();
    let refused = Client::new(config(&refused_url()).with_max_read_attempts(2)).unwrap();

    retrying.clock().await.unwrap();
    once.clock().await.unwrap_err();
    once.clock().await.unwrap_err();
    retrying.clock().await.unwrap_err();
    once.clock().await.unwrap_err();
    once.clock().await.unwrap_err();
    deadline.clock().await.unwrap_err();
    refused.clock().await.unwrap_err();

    let guard = CAPTURED.lock().unwrap();
    let captured = guard.as_ref().unwrap();
    for span in &captured.spans {
        assert_eq!(span.name, "suri.request");
        assert_eq!(span.level, Level::INFO);
    }
    let spans: Vec<&Fields> = captured.spans.iter().map(|span| &span.fields).collect();
    let expected = [
        // A 503, retried, then a 200: no stage, no error event.
        span_fields(&[
            ("attempt", "1"),
            ("status", "503"),
            ("attempt", "2"),
            ("status", "200"),
        ]),
        // A redirect, not followed.
        span_fields(&[
            ("attempt", "1"),
            ("status", "302"),
            ("stage", "response_received"),
        ]),
        // A connection closed before any response: no status.
        span_fields(&[("attempt", "1"), ("stage", "unknown")]),
        // A 429, retried, then a 404, which is not.
        span_fields(&[
            ("attempt", "1"),
            ("status", "429"),
            ("attempt", "2"),
            ("status", "404"),
            ("stage", "response_received"),
        ]),
        // A body that does not decode, and one over the limit.
        span_fields(&[
            ("attempt", "1"),
            ("status", "200"),
            ("stage", "response_received"),
        ]),
        span_fields(&[
            ("attempt", "1"),
            ("status", "200"),
            ("stage", "response_received"),
        ]),
        // Two timed-out attempts, then the deadline.
        span_fields(&[("attempt", "1"), ("attempt", "2"), ("stage", "unknown")]),
        // Two refused connections: never sent.
        span_fields(&[("attempt", "1"), ("attempt", "2"), ("stage", "not_started")]),
    ];
    assert_eq!(spans, expected.iter().collect::<Vec<_>>());

    // Each event: its name, its span, and its fields other than wait_ms.
    let events: Vec<(&str, Level, Option<usize>, Fields)> = captured
        .events
        .iter()
        .map(|event| {
            let fields = event
                .fields
                .iter()
                .filter(|(name, _)| name != "wait_ms")
                .cloned()
                .collect();
            (event.name, event.level, event.span, fields)
        })
        .collect();
    let retry = |span, reason| {
        (
            "retry",
            Level::DEBUG,
            Some(span),
            pairs(&[
                ("message", "retrying"),
                ("attempt", "1"),
                ("reason", reason),
            ]),
        )
    };
    let error = |span, kind| {
        (
            "error",
            Level::DEBUG,
            Some(span),
            pairs(&[("message", "request failed"), ("kind", kind)]),
        )
    };
    assert_eq!(
        events,
        [
            retry(0, "status_5xx"),
            error(1, "redirect"),
            error(2, "transport"),
            retry(3, "status_429"),
            error(3, "api"),
            error(4, "decode"),
            error(5, "body_too_large"),
            retry(6, "timeout"),
            error(6, "timeout"),
            retry(7, "transport"),
            error(7, "transport"),
        ]
    );
    // The retry event's wait: the 10 ms backoff plus up to 5 ms of jitter.
    let wait_ms = &captured.events[0].fields[3];
    assert_eq!(wait_ms.0, "wait_ms");
    let wait_ms: u64 = wait_ms.1.parse().unwrap();
    assert!((10..15).contains(&wait_ms), "{wait_ms}");
    for event in captured.events.iter().filter(|event| event.name == "retry") {
        assert_eq!(event.fields.len(), 4);
    }

    let address = server.url.trim_start_matches("http://");
    let values = captured
        .spans
        .iter()
        .flat_map(|span| &span.fields)
        .chain(captured.events.iter().flat_map(|event| &event.fields));
    for (_, value) in values {
        for forbidden in [address, KEY, SECRET] {
            assert!(!value.contains(forbidden), "{value}");
        }
    }
}

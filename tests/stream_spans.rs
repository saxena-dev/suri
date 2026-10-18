//! Each stream is one `suri.stream` span with its documented field, and
//! emits documented events. One test in its own binary: it installs a
//! global subscriber.

#![cfg(feature = "stream")]

#[path = "support/ws_server.rs"]
mod ws_server;

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;
use std::time::Duration;

use suri::{Client, Config, Credentials, Environment, Region};
use tokio_tungstenite::tungstenite::Message;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use ws_server::{Act, Step, WsServer, opened};

const KEY: &str = "PKSENTINELKEY0001";
const SECRET: &str = "SECRETSENTINELVALUE0001";
const UPDATE: &str = r#"{"stream":"trade_updates","data":{"event":"new"}}"#;

type Fields = Vec<(String, String)>;

#[derive(Debug)]
struct SpanRecord {
    name: &'static str,
    level: Level,
    fields: Fields,
}

#[derive(Debug)]
struct EventRecord {
    name: &'static str,
    level: Level,
    span: Option<usize>,
    fields: Fields,
}

#[derive(Default)]
struct Captured {
    spans: Vec<SpanRecord>,
    live: HashMap<u64, usize>,
    events: Vec<EventRecord>,
}

static CAPTURED: Mutex<Option<Captured>> = Mutex::new(None);

struct Collect<'a>(&'a mut Fields);

impl Visit for Collect<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.push((field.name().to_owned(), value.to_owned()));
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

fn client(server: &WsServer, timeout: Duration) -> Client {
    Client::new(
        Config::new(
            Region::US,
            Environment::Paper,
            Credentials::api_key(KEY, SECRET),
        )
        .with_base_url(&server.url)
        .with_connect_timeout(timeout)
        .with_request_timeout(timeout),
    )
    .unwrap()
}

/// The names of the events in span `index`, and the `kind` of its `ended`.
fn events_of(captured: &Captured, index: usize) -> (Vec<&'static str>, Option<String>) {
    let events: Vec<&EventRecord> = captured
        .events
        .iter()
        .filter(|event| event.span == Some(index))
        .collect();
    for event in &events {
        assert_eq!(event.level, Level::DEBUG, "{event:?}");
    }
    let kind = events
        .iter()
        .find(|event| event.name == "ended")
        .and_then(|event| event.fields.iter().find(|(name, _)| name == "kind"))
        .map(|(_, value)| value.clone());
    (events.iter().map(|event| event.name).collect(), kind)
}

#[tokio::test]
async fn each_stream_is_one_span_with_documented_events() {
    tracing::subscriber::set_global_default(tracing_subscriber::registry().with(Capture)).unwrap();
    let second = Duration::from_secs(5);

    // Opens, delivers one message, is closed by the server.
    let server = WsServer::start(vec![opened(vec![
        Act::Send(Message::text(UPDATE)),
        Act::Close(1000, String::new()),
        Act::ReadAll,
    ])]);
    let mut stream = client(&server, second).trade_updates().await.unwrap();
    stream.next().await.unwrap();
    stream.next().await.unwrap_err();
    drop(stream);

    // real-time: a short request timeout draws a liveness Ping.
    let interval = Duration::from_millis(300);
    let server = WsServer::start(vec![opened(vec![
        Act::ReadUntilPing,
        Act::Send(Message::text(UPDATE)),
    ])]);
    let mut stream = client(&server, interval).trade_updates().await.unwrap();
    stream.next().await.unwrap();
    drop(stream);

    // Closed by the caller.
    let server = WsServer::start(vec![opened(vec![Act::ReadAll])]);
    let stream = client(&server, second).trade_updates().await.unwrap();
    stream.close().await.1.unwrap();

    // Rejected while opening.
    let unauthorized =
        r#"{"stream":"authorization","data":{"status":"unauthorized","action":"authenticate"}}"#;
    let server = WsServer::start(vec![Step::Upgrade(vec![
        Act::Read,
        Act::Send(Message::text(unauthorized)),
    ])]);
    client(&server, second).trade_updates().await.unwrap_err();

    let guard = CAPTURED.lock().unwrap();
    let captured = guard.as_ref().unwrap();
    assert_eq!(captured.spans.len(), 4, "{:?}", captured.spans);
    for span in &captured.spans {
        assert_eq!(span.name, "suri.stream");
        assert_eq!(span.level, Level::INFO);
        assert_eq!(
            span.fields,
            [("stream".to_owned(), "trade_updates".to_owned())]
        );
    }
    let opened_events = ["connected", "authenticated", "listening"];
    let (events, kind) = events_of(captured, 0);
    assert_eq!(events, [&opened_events[..], &["ended"]].concat());
    assert_eq!(kind.as_deref(), Some("closed"));
    let (events, _) = events_of(captured, 1);
    assert_eq!(events, [&opened_events[..], &["ping"]].concat());
    let (events, kind) = events_of(captured, 2);
    assert_eq!(events, [&opened_events[..], &["ended"]].concat());
    assert_eq!(kind.as_deref(), Some("closed_by_caller"));
    let (events, kind) = events_of(captured, 3);
    assert_eq!(events, ["connected", "ended"]);
    assert_eq!(kind.as_deref(), Some("unauthorized"));

    for record in captured
        .spans
        .iter()
        .flat_map(|span| span.fields.iter())
        .chain(captured.events.iter().flat_map(|event| event.fields.iter()))
    {
        let text = format!("{record:?}");
        assert!(
            !text.contains("127.0.0.1") && !text.contains(KEY) && !text.contains(SECRET),
            "{text}"
        );
    }
}

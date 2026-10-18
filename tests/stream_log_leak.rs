//! On a stream, credentials reach only the WebSocket library's documented
//! TRACE and DEBUG records, and none survives the documented filter. One
//! test in its own binary: it installs a global logger and subscriber.

#![cfg(feature = "stream")]

#[path = "support/ws_server.rs"]
mod ws_server;

use std::fmt;
use std::fmt::Write as _;
use std::sync::Mutex;

use suri::data::options::Feed;
use suri::data::stocks::StreamFeed;
use suri::{Client, Config, Credentials, Environment, Region};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::{Context, SubscriberExt};
use ws_server::{Act, WsServer, bounded, data_opened, msgpack_opened, opened};

const KEY: &str = "PKSENTINELKEY0001";
const SECRET: &str = "SECRETSENTINELVALUE0001";

/// One captured record: where it came from and what it said.
#[derive(Debug)]
struct Line {
    thread: String,
    source: &'static str,
    level: String,
    target: String,
    text: String,
}

static LINES: Mutex<Vec<Line>> = Mutex::new(Vec::new());

fn thread() -> String {
    std::thread::current()
        .name()
        .unwrap_or("unnamed")
        .to_owned()
}

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        LINES.lock().unwrap().push(Line {
            thread: thread(),
            source: "log",
            level: record.level().to_string(),
            target: record.target().to_owned(),
            text: record.args().to_string(),
        });
    }

    fn flush(&self) {}
}

struct Text(String);

impl Visit for Text {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        let _ = write!(self.0, " {}={value:?}", field.name());
    }
}

struct Capture;

impl Capture {
    fn push(&self, level: String, target: &str, text: String) {
        LINES.lock().unwrap().push(Line {
            thread: thread(),
            source: "tracing",
            level,
            target: target.to_owned(),
            text,
        });
    }
}

impl<S: Subscriber> Layer<S> for Capture {
    fn on_new_span(&self, attributes: &Attributes<'_>, _: &Id, _: Context<'_, S>) {
        let mut text = Text(String::new());
        attributes.record(&mut text);
        let metadata = attributes.metadata();
        self.push(metadata.level().to_string(), metadata.target(), text.0);
    }

    fn on_record(&self, _: &Id, values: &Record<'_>, _: Context<'_, S>) {
        let mut text = Text(String::new());
        values.record(&mut text);
        self.push("RECORD".to_owned(), "", text.0);
    }

    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let mut text = Text(String::new());
        event.record(&mut text);
        let metadata = event.metadata();
        self.push(metadata.level().to_string(), metadata.target(), text.0);
    }
}

fn hex(text: &str) -> String {
    text.bytes().map(|byte| format!("{byte:02x}")).collect()
}

/// Whether the line holds a sentinel, as text or as lowercase hex.
fn leaks(text: &str) -> bool {
    [KEY, SECRET]
        .iter()
        .any(|sentinel| text.contains(sentinel) || text.contains(&hex(sentinel)))
}

/// The record kinds the contract lists as carrying stream content.
const DOCUMENTED: [&str; 7] = [
    "Request:",
    "Sending frame",
    "writing frame",
    "received frame",
    "Received message",
    "Received close frame",
    "Replying to close",
];

#[tokio::test]
async fn credentials_reach_only_the_documented_records() {
    bounded(async {
        log::set_logger(&Logger).unwrap();
        log::set_max_level(log::LevelFilter::Trace);
        tracing::subscriber::set_global_default(tracing_subscriber::registry().with(Capture))
            .unwrap();

        let reason = format!("bad key {KEY} secret {SECRET}");
        let server = WsServer::start(vec![opened(vec![Act::Close(1008, reason), Act::ReadAll])]);
        let client = Client::new(
            Config::new(
                Region::US,
                Environment::Paper,
                Credentials::api_key(KEY, SECRET),
            )
            .with_base_url(&server.url),
        )
        .unwrap();
        let mut stream = client.trade_updates().await.unwrap();
        let error = stream.next().await.unwrap_err();
        for text in [error.to_string(), format!("{error:?}")] {
            assert!(!leaks(&text), "{text}");
        }
        server.wait_for_frames(0, 3).await;

        // A market-data stream sends the key pair in the upgrade's headers; so
        // does the options stream, which speaks MessagePack.
        let server = WsServer::start(vec![
            data_opened(vec![Act::ReadAll]),
            msgpack_opened(vec![Act::ReadAll]),
        ]);
        let data_client = Client::new(
            Config::new(
                Region::US,
                Environment::Paper,
                Credentials::api_key(KEY, SECRET),
            )
            .with_base_url(&server.url),
        )
        .unwrap();
        let stream = data_client.stock_stream(StreamFeed::Iex).await.unwrap();
        stream.close().await.1.unwrap();
        let stream = data_client.option_stream(Feed::Opra).await.unwrap();
        stream.close().await.1.unwrap();

        let lines = LINES.lock().unwrap();
        let client_lines: Vec<&Line> = lines
            .iter()
            .filter(|line| line.thread != "ws-server")
            .collect();
        // The capture saw suri's records and the WebSocket library's, so an
        // empty capture cannot pass.
        assert!(client_lines.iter().any(|line| line.target == "suri"));
        for line in &client_lines {
            if leaks(&line.text) {
                assert!(line.target.starts_with("tungstenite::"), "{line:?}");
                assert!(
                    DOCUMENTED.iter().any(|kind| line.text.starts_with(kind)),
                    "{line:?}"
                );
                assert!(line.level == "TRACE" || line.level == "DEBUG", "{line:?}");
            }
        }
        let present = |kind: &str| {
            client_lines
                .iter()
                .any(|line| line.text.starts_with(kind) && line.text.contains(SECRET))
        };
        assert!(present("Sending frame"), "the authentication frame");
        assert!(present("Request:"), "the market-data upgrade's headers");
        assert!(present("Received close frame"), "the close reason");

        // Under the documented filter, `tungstenite` at INFO, nothing leaks on
        // any thread.
        for line in lines.iter() {
            // tungstenite logs through `log`.
            let filtered = line.source == "log"
                && line.target.starts_with("tungstenite")
                && (line.level == "TRACE" || line.level == "DEBUG");
            if !filtered {
                assert!(!leaks(&line.text), "{line:?}");
            }
        }
    })
    .await
}

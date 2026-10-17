//! No credential reaches a log record or a tracing span or event, from suri
//! or from any library it uses, at the most detailed level. One test in its
//! own binary: it installs a global logger and subscriber.

#![cfg(feature = "rest")]

#[path = "support/server.rs"]
mod server;

use std::fmt;
use std::fmt::Write as _;
use std::sync::Mutex;
use std::time::Duration;

use server::{Server, Step, response};
use suri::{Client, Config, Credentials, Environment, Region};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::{Context, SubscriberExt};

const KEY: &str = "PKSENTINELKEY0001";
const SECRET: &str = "SECRETSENTINELVALUE0001";
const CLOCK_BODY: &str = r#"{"timestamp":"2022-04-28T14:07:04-04:00","is_open":true,"next_open":"2022-04-29T09:30:00-04:00","next_close":"2022-04-28T16:00:00-04:00"}"#;

/// Every captured line, prefixed with its source and target.
static LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn capture(line: String) {
    LINES.lock().unwrap().push(line);
}

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        capture(format!(
            "log {} {}: {}",
            record.level(),
            record.target(),
            record.args()
        ));
    }

    fn flush(&self) {}
}

struct Line(String);

impl Visit for Line {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        let _ = write!(self.0, " {}={value:?}", field.name());
    }
}

struct Capture;

impl<S: Subscriber> Layer<S> for Capture {
    fn on_new_span(&self, attributes: &Attributes<'_>, _: &Id, _: Context<'_, S>) {
        let metadata = attributes.metadata();
        let mut line = Line(format!("span {} {}:", metadata.level(), metadata.target()));
        attributes.record(&mut line);
        capture(line.0);
    }

    fn on_record(&self, _: &Id, values: &Record<'_>, _: Context<'_, S>) {
        let mut line = Line("record:".to_owned());
        values.record(&mut line);
        capture(line.0);
    }

    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let metadata = event.metadata();
        let mut line = Line(format!("event {} {}:", metadata.level(), metadata.target()));
        event.record(&mut line);
        capture(line.0);
    }
}

#[tokio::test]
async fn no_credential_reaches_any_log_or_trace() {
    log::set_logger(&Logger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    tracing::subscriber::set_global_default(tracing_subscriber::registry().with(Capture)).unwrap();

    let echo = format!(r#"{{"code": 40110000, "message": "bad key {KEY} secret {SECRET}"}}"#);
    let server = Server::start(vec![
        Step::Respond(response("200 OK", &[], CLOCK_BODY.as_bytes())),
        Step::Respond(response("422 Unprocessable Entity", &[], echo.as_bytes())),
        Step::Drop,
        // A retried read, so the retry path is captured too.
        Step::Respond(response("503 Service Unavailable", &[], echo.as_bytes())),
        Step::Respond(response("200 OK", &[], CLOCK_BODY.as_bytes())),
    ]);
    let config = Config::new(
        Region::US,
        Environment::Paper,
        Credentials::api_key(KEY, SECRET),
    )
    .with_base_url(&server.url)
    .with_retry_base(Duration::from_millis(10));
    let once = Client::new(config.clone().with_max_read_attempts(1)).unwrap();
    once.clock().await.unwrap();
    once.clock().await.unwrap_err();
    once.clock().await.unwrap_err();
    Client::new(config).unwrap().clock().await.unwrap();

    let lines = LINES.lock().unwrap();
    // The capture saw suri's records and its HTTP library's, so an empty
    // capture cannot pass.
    assert!(lines.iter().any(|line| line.contains(" suri:")));
    assert!(lines.iter().any(|line| line.contains(" hyper_util")));
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("log ") && line.contains("reqwest::connect"))
    );
    for line in lines.iter() {
        assert!(!line.contains(KEY) && !line.contains(SECRET), "{line}");
    }
}

//! The market-data streams: one generic stream over the shared WebSocket
//! core, its opening, its subscriptions and the reading of its frames.

use std::fmt;
use std::marker::PhantomData;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use tokio_tungstenite::tungstenite::Message as Frame;
use tracing::Instrument;

use super::sealed::Sealed;
use crate::Client;
use crate::rest::error::sanitise;
use crate::stream::{
    Connection, Phase, Socket, StreamError, StreamErrorKind, connect, ended, read_opening,
};

/// Implemented by suri's market-data message types (`stocks::Message`,
/// `crypto::Message`, `news::Message`); it cannot be implemented outside
/// suri.
pub trait DataMessage: Sealed + Sized {}

/// The channels of a subscription, each a list of symbols (`"*"` for every
/// symbol). One type serves every stream: a channel a stream does not offer
/// is refused by Alpaca with an `error` message.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Channels {
    /// Trades.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trades: Vec<String>,
    /// Quotes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quotes: Vec<String>,
    /// Minute bars.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bars: Vec<String>,
    /// Updated minute bars, for trades that arrive late.
    #[serde(rename = "updatedBars", default, skip_serializing_if = "Vec::is_empty")]
    pub updated_bars: Vec<String>,
    /// Daily bars.
    #[serde(rename = "dailyBars", default, skip_serializing_if = "Vec::is_empty")]
    pub daily_bars: Vec<String>,
    /// Trading statuses (stocks).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub statuses: Vec<String>,
    /// Limit up-limit down bands (stocks).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lulds: Vec<String>,
    /// Trade corrections (stocks); Alpaca subscribes them with `trades`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corrections: Vec<String>,
    /// Trade cancels and errors (stocks); Alpaca subscribes them with
    /// `trades`.
    #[serde(
        rename = "cancelErrors",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub cancel_errors: Vec<String>,
    /// Order imbalances (stocks).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imbalances: Vec<String>,
    /// Orderbooks (crypto).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub orderbooks: Vec<String>,
    /// News.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub news: Vec<String>,
}

impl Channels {
    /// No channel; set the fields to subscribe.
    pub fn new() -> Channels {
        Channels::default()
    }
}

/// A subscribe or unsubscribe message: the action, then the non-empty
/// channels in field order.
#[derive(Serialize)]
struct Request<'a> {
    action: &'static str,
    #[serde(flatten)]
    channels: &'a Channels,
}

/// A market-data stream: one WebSocket connection, read only while
/// [`next`](DataStream::next) or [`close`](DataStream::close) runs.
pub struct DataStream<M> {
    connection: Connection,
    messages: PhantomData<fn() -> M>,
}

impl<M> fmt::Debug for DataStream<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DataStream { .. }")
    }
}

impl<M: DataMessage> DataStream<M> {
    /// Adds these channels to the subscription. Returns once the message is
    /// sent, within `request_timeout`; Alpaca's reply, the whole current
    /// subscription or an `error`, arrives through [`next`](DataStream::next).
    /// A failure ends the stream; a message not sent in time ends it with
    /// `Timeout`. Not cancel-safe: dropping the future may or may not have
    /// sent the message.
    pub async fn subscribe(&mut self, channels: &Channels) -> Result<(), StreamError> {
        self.write("subscribe", channels).await
    }

    /// Removes these channels from the subscription, as
    /// [`subscribe`](DataStream::subscribe) adds them.
    pub async fn unsubscribe(&mut self, channels: &Channels) -> Result<(), StreamError> {
        self.write("unsubscribe", channels).await
    }

    async fn write(
        &mut self,
        action: &'static str,
        channels: &Channels,
    ) -> Result<(), StreamError> {
        let text = serde_json::to_string(&Request { action, channels })
            .expect("a subscription message serialises");
        let span = self.connection.span().clone();
        self.connection
            .write(Frame::text(text))
            .instrument(span)
            .await
    }

    /// The messages of the next frame, in the order sent; never empty.
    /// Ping and Pong frames are handled and never delivered. After
    /// `request_timeout` without any frame, a Ping is sent; if nothing
    /// arrives within another `request_timeout`, the stream ends with
    /// `Timeout`. Every end other than [`close`](DataStream::close) is an
    /// error saying later messages may be lost, and every later call
    /// returns `Ended`. Cancel-safe: dropping the future loses no message.
    pub async fn next(&mut self) -> Result<Vec<M>, StreamError> {
        let span = self.connection.span().clone();
        async {
            loop {
                let frame = self.connection.next_frame().await?;
                match messages(self.connection.client(), &frame, Phase::Next) {
                    Ok(messages) if messages.is_empty() => {}
                    Ok(messages) => return Ok(messages),
                    Err(error) => return Err(self.connection.end(error)),
                }
            }
        }
        .instrument(span)
        .await
    }

    /// Closes the stream: sends a Close frame and reads until Alpaca's Close
    /// arrives or `request_timeout` passes. Returns the messages of every
    /// frame read meanwhile, whatever the outcome. Not cancel-safe: dropping
    /// the future drops the connection and the messages read so far.
    pub async fn close(self) -> (Vec<M>, Result<(), StreamError>) {
        self.connection
            .close(|client, frame| messages(client, &frame, Phase::Close))
            .await
    }
}

/// Opens a market-data stream at `path` on the data stream host: the
/// upgrade with the key-pair headers, then Alpaca's `connected` and
/// `authenticated` messages.
pub(crate) async fn open<M: DataMessage>(
    client: &Client,
    path: &str,
    feed: Option<&'static str>,
) -> Result<DataStream<M>, StreamError> {
    let span = match feed {
        Some(feed) => tracing::info_span!(target: "suri", "suri.stream", stream = M::STREAM, feed),
        None => tracing::info_span!(
            target: "suri",
            "suri.stream",
            stream = M::STREAM,
            feed = tracing::field::Empty
        ),
    };
    match opening(client, path).instrument(span.clone()).await {
        Ok(socket) => Ok(DataStream {
            connection: Connection::new(socket, client, span),
            messages: PhantomData,
        }),
        Err(error) => {
            span.in_scope(|| ended(error.kind.as_str()));
            Err(error)
        }
    }
}

async fn opening(client: &Client, path: &str) -> Result<Socket, StreamError> {
    let mut url = client.data_stream_url().clone();
    url.set_path(path);
    let [key, secret] = client.key_headers();
    let headers = [("apca-api-key-id", key), ("apca-api-secret-key", secret)];
    let steps = async {
        let mut socket = connect(client, &url, &headers).await?;
        for expected in ["connected", "authenticated"] {
            let frame = read_opening(client, &mut socket).await?;
            opened(client, &frame, expected)?;
        }
        tracing::debug!(name: "authenticated", target: "suri", "stream authenticated");
        Ok(socket)
    };
    tokio::time::timeout(client.request_timeout(), steps)
        .await
        .map_err(|_| StreamError::new(StreamErrorKind::Timeout, Phase::Opening))?
}

#[derive(Deserialize)]
struct Success {
    msg: String,
}

#[derive(Deserialize, Default)]
struct OpeningError {
    code: Option<u16>,
    msg: Option<String>,
}

/// Checks an opening frame: exactly one `success` whose `msg` is
/// `expected`. An `error` ends the opening, `Unauthorized` for code 402 and
/// `Server` otherwise; anything else is `Decode`.
fn opened(client: &Client, frame: &Frame, expected: &str) -> Result<(), StreamError> {
    let decode = || StreamError::new(StreamErrorKind::Decode, Phase::Opening);
    let elements = elements(frame, Phase::Opening)?;
    let [element] = elements.as_slice() else {
        return Err(decode());
    };
    let element = element.get();
    match head(element).ok_or_else(decode)?.kind.as_str() {
        "success" => match serde_json::from_str::<Success>(element) {
            Ok(success) if success.msg == expected => Ok(()),
            _ => Err(decode()),
        },
        "error" => {
            let fields: OpeningError = serde_json::from_str(element).unwrap_or_default();
            let kind = if fields.code == Some(402) {
                StreamErrorKind::Unauthorized
            } else {
                StreamErrorKind::Server
            };
            let mut error = StreamError::new(kind, Phase::Opening);
            error.code = fields.code;
            error.message = fields
                .msg
                .map(|text| sanitise(&text, &client.credentials()));
            Err(error)
        }
        _ => Err(decode()),
    }
}

/// A frame's elements: a text or binary frame holding one JSON array.
fn elements(frame: &Frame, phase: Phase) -> Result<Vec<&RawValue>, StreamError> {
    let decode = || StreamError::new(StreamErrorKind::Decode, phase);
    let bytes = match frame {
        Frame::Text(text) => text.as_bytes(),
        Frame::Binary(bytes) => bytes,
        _ => return Err(decode()),
    };
    serde_json::from_slice(bytes).map_err(|_| decode())
}

/// A frame's messages, one per element, in order.
fn messages<M: DataMessage>(
    client: &Client,
    frame: &Frame,
    phase: Phase,
) -> Result<Vec<M>, StreamError> {
    let credentials = client.credentials();
    Ok(elements(frame, phase)?
        .into_iter()
        .map(|element| M::decode(element.get(), &credentials))
        .collect())
}

/// What a message is: its `T`, and its `S` when it has one.
#[derive(Deserialize)]
pub(crate) struct Head {
    #[serde(rename = "T")]
    pub(crate) kind: String,
    #[serde(rename = "S")]
    pub(crate) symbol: Option<String>,
}

/// The head of an element that is an object whose `T`, and `S` if present,
/// are text; `None` for anything else.
pub(crate) fn head(element: &str) -> Option<Head> {
    if !element.starts_with('{') {
        return None;
    }
    serde_json::from_str(element).ok()
}

/// A data point: its symbol and its model; `None` when either is missing or
/// does not read.
pub(crate) fn point<T: DeserializeOwned>(
    element: &str,
    symbol: Option<String>,
) -> Option<(String, T)> {
    let symbol = symbol?;
    Some((symbol, serde_json::from_str(element).ok()?))
}

#[derive(Deserialize)]
struct ErrorMessage {
    code: u16,
    msg: String,
}

/// An `error` message's code and scrubbed text; `None` when either is
/// missing or does not read.
pub(crate) fn error(element: &str, credentials: &[&str; 2]) -> Option<(u16, String)> {
    let error: ErrorMessage = serde_json::from_str(element).ok()?;
    Some((error.code, sanitise(&error.msg, credentials)))
}

/// A `subscription` message's channels.
pub(crate) fn subscription(element: &str) -> Option<Channels> {
    serde_json::from_str(element).ok()
}

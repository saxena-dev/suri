//! The market-data streams: one generic stream over the shared WebSocket
//! core, its opening, its subscriptions and the reading of its frames.

use std::fmt;
use std::io::Cursor;
use std::marker::PhantomData;

use serde::de::{DeserializeOwned, IgnoredAny};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use tokio_tungstenite::tungstenite::Message as Frame;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tracing::Instrument;

use super::sealed::{Codec, Sealed};
use crate::Client;
use crate::rest::error::sanitise;
use crate::stream::{
    Connection, Phase, Socket, StreamError, StreamErrorKind, connect, ended, read_opening,
};

/// Implemented by suri's market-data message types (`stocks::Message`,
/// `crypto::Message`, `options::Message`, `news::Message`); it cannot be
/// implemented outside suri.
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

/// A market-data stream: live trades, quotes, bars or news, on one WebSocket
/// connection.
///
/// It opens subscribed to nothing: [`subscribe`](DataStream::subscribe) to
/// [`Channels`], and each [`next`](DataStream::next) returns the messages of
/// one frame, in the order sent. The value owns its connection and reads it
/// only while `next` or [`close`](DataStream::close) runs, so nothing piles
/// up inside suri or is dropped behind your back. A stream that ends
/// without your asking returns a [`StreamError`](crate::StreamError) saying
/// later messages may be lost.
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
        let request = Request { action, channels };
        let frame = match M::CODEC {
            Codec::Json => Frame::text(
                serde_json::to_string(&request).expect("a subscription message serialises"),
            ),
            // A map with named keys, as Alpaca's examples are.
            Codec::MessagePack => Frame::binary(
                rmp_serde::to_vec_named(&request).expect("a subscription message serialises"),
            ),
        };
        let span = self.connection.span().clone();
        self.connection.write(frame).instrument(span).await
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
                match messages::<M>(self.connection.client(), &frame, Phase::Next) {
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
            .close(|client, frame| messages::<M>(client, &frame, Phase::Close))
            .await
    }
}

/// Opens a market-data stream at `path` on the data stream host: the
/// upgrade with the key-pair headers (and, for MessagePack, its content
/// type), then Alpaca's `connected` and `authenticated` messages.
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
    match opening(client, path, M::CODEC)
        .instrument(span.clone())
        .await
    {
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

async fn opening(client: &Client, path: &str, codec: Codec) -> Result<Socket, StreamError> {
    let mut url = client.data_stream_url().clone();
    url.set_path(path);
    let [key, secret] = client.key_headers();
    let msgpack = HeaderValue::from_static("application/msgpack");
    let mut headers = vec![("apca-api-key-id", key), ("apca-api-secret-key", secret)];
    if codec == Codec::MessagePack {
        headers.push(("content-type", &msgpack));
    }
    let steps = async {
        let mut socket = connect(client, &url, &headers).await?;
        for expected in ["connected", "authenticated"] {
            let frame = read_opening(client, &mut socket).await?;
            opened(client, codec, &frame, expected)?;
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
fn opened(client: &Client, codec: Codec, frame: &Frame, expected: &str) -> Result<(), StreamError> {
    let decode = || StreamError::new(StreamErrorKind::Decode, Phase::Opening);
    let elements = elements(codec, frame, Phase::Opening)?;
    let [element] = elements.as_slice() else {
        return Err(decode());
    };
    match head(codec, element).ok_or_else(decode)?.kind.as_str() {
        "success" => match read::<Success>(codec, element) {
            Some(success) if success.msg == expected => Ok(()),
            _ => Err(decode()),
        },
        "error" => {
            let fields: OpeningError = read(codec, element).unwrap_or_default();
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

/// A frame's elements, each its exact bytes: on a JSON stream a text or
/// binary frame holding one array; on a MessagePack stream a binary frame
/// holding one array. Anything else, bytes after the array included, is
/// `Decode`.
fn elements(codec: Codec, frame: &Frame, phase: Phase) -> Result<Vec<&[u8]>, StreamError> {
    let decode = || StreamError::new(StreamErrorKind::Decode, phase);
    match (codec, frame) {
        (Codec::Json, Frame::Text(text)) => json_elements(text.as_bytes()),
        (Codec::Json, Frame::Binary(bytes)) => json_elements(bytes),
        (Codec::MessagePack, Frame::Binary(bytes)) => messagepack_elements(bytes),
        _ => None,
    }
    .ok_or_else(decode)
}

fn json_elements(bytes: &[u8]) -> Option<Vec<&[u8]>> {
    let elements: Vec<&RawValue> = serde_json::from_slice(bytes).ok()?;
    Some(
        elements
            .into_iter()
            .map(|element| element.get().as_bytes())
            .collect(),
    )
}

/// Splits a MessagePack array into its elements without decoding them. No
/// vector is sized from the header, which may claim any count, and nesting
/// is limited to JSON's depth, 128, so a deep frame is an error rather than
/// a stack overflow.
fn messagepack_elements(bytes: &[u8]) -> Option<Vec<&[u8]>> {
    let mut deserializer = rmp_serde::Deserializer::new(Cursor::new(bytes));
    deserializer.set_max_depth(128);
    let count = rmp::decode::read_array_len(deserializer.get_mut()).ok()?;
    let mut elements = Vec::new();
    for _ in 0..count {
        let start = usize::try_from(deserializer.position()).ok()?;
        IgnoredAny::deserialize(&mut deserializer).ok()?;
        let end = usize::try_from(deserializer.position()).ok()?;
        elements.push(bytes.get(start..end)?);
    }
    (usize::try_from(deserializer.position()).ok()? == bytes.len()).then_some(elements)
}

/// A frame's messages, one per element, in order.
fn messages<M: DataMessage>(
    client: &Client,
    frame: &Frame,
    phase: Phase,
) -> Result<Vec<M>, StreamError> {
    let credentials = client.credentials();
    Ok(elements(M::CODEC, frame, phase)?
        .into_iter()
        .map(|element| M::decode(element, &credentials))
        .collect())
}

/// An element read as `T`; `None` when it does not read.
pub(crate) fn read<T: DeserializeOwned>(codec: Codec, element: &[u8]) -> Option<T> {
    match codec {
        Codec::Json => serde_json::from_slice(element).ok(),
        Codec::MessagePack => rmp_serde::from_slice(element).ok(),
    }
}

/// A JSON element's text, as sent.
pub(crate) fn text(element: &[u8]) -> String {
    String::from_utf8_lossy(element).into_owned()
}

/// What a message is: its `T`, and its `S` when it has one.
#[derive(Deserialize)]
pub(crate) struct Head {
    #[serde(rename = "T")]
    pub(crate) kind: String,
    #[serde(rename = "S")]
    pub(crate) symbol: Option<String>,
}

/// The head of an element that is an object (a JSON object, a MessagePack
/// map) whose `T`, and `S` if present, are text; `None` for anything else,
/// since a derived model also reads an array.
pub(crate) fn head(codec: Codec, element: &[u8]) -> Option<Head> {
    let object = match codec {
        Codec::Json => element.first() == Some(&b'{'),
        Codec::MessagePack => matches!(element.first(), Some(0x80..=0x8f | 0xde | 0xdf)),
    };
    if !object {
        return None;
    }
    read(codec, element)
}

/// A data point: its symbol and its model; `None` when either is missing or
/// does not read.
pub(crate) fn point<T: DeserializeOwned>(
    codec: Codec,
    element: &[u8],
    symbol: Option<String>,
) -> Option<(String, T)> {
    let symbol = symbol?;
    Some((symbol, read(codec, element)?))
}

#[derive(Deserialize)]
struct ErrorMessage {
    code: u16,
    msg: String,
}

/// An `error` message's code and scrubbed text; `None` when either is
/// missing or does not read.
pub(crate) fn error(
    codec: Codec,
    element: &[u8],
    credentials: &[&str; 2],
) -> Option<(u16, String)> {
    let error: ErrorMessage = read(codec, element)?;
    Some((error.code, sanitise(&error.msg, credentials)))
}

/// A `subscription` message's channels.
pub(crate) fn subscription(codec: Codec, element: &[u8]) -> Option<Channels> {
    read(codec, element)
}

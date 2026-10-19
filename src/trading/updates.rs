//! The `trade_updates` stream: its opening on the Trading host, its raw
//! messages, and those messages typed: an event and the order it is about.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::value::RawValue;
use tokio_tungstenite::tungstenite::Message;
use tracing::Instrument;

use super::Order;
use crate::rest::config::websocket;
use crate::rest::error::sanitise;
use crate::stream::{
    Connection, Phase, Socket, StreamError, StreamErrorKind, connect, ended, read_opening, send,
};
use crate::value::wire::wire_enum;
use crate::{Client, Decimal, Inbound, Timestamp};

wire_enum! {
    /// What happened to an order.
    TradeEvent {
        /// The order was routed to the exchanges.
        New => "new",
        /// The order filled completely.
        Fill => "fill",
        /// Part of the order filled.
        PartialFill => "partial_fill",
        /// A requested cancellation was processed.
        Canceled => "canceled",
        /// The order reached the end of its time in force.
        Expired => "expired",
        /// The order is done for the day.
        DoneForDay => "done_for_day",
        /// A requested replacement was processed.
        Replaced => "replaced",
        /// Alpaca received the order but has not routed it yet.
        Accepted => "accepted",
        /// The order was rejected.
        Rejected => "rejected",
        /// The order was routed but not yet accepted for execution.
        PendingNew => "pending_new",
        /// A trade is guaranteed for the order but has not occurred.
        Stopped => "stopped",
        /// The order is awaiting cancellation.
        PendingCancel => "pending_cancel",
        /// The order is awaiting replacement.
        PendingReplace => "pending_replace",
        /// The order is complete for the day; settlement calculations are
        /// pending.
        Calculated => "calculated",
        /// The order is suspended and not eligible for trading.
        Suspended => "suspended",
        /// A replacement was rejected.
        OrderReplaceRejected => "order_replace_rejected",
        /// A cancellation was rejected.
        OrderCancelRejected => "order_cancel_rejected",
    }
}

/// One `trade_updates` message: the event and the order, with the fields
/// that only some events carry.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TradeUpdate {
    /// What happened.
    pub event: Inbound<TradeEvent>,
    /// The order, as the REST API reports it.
    pub order: Order,
    /// When it happened; sent with a fill, a partial fill, a cancellation,
    /// an expiry, a replacement and a rejection.
    pub timestamp: Option<Timestamp>,
    /// The fill's execution ID.
    pub execution_id: Option<String>,
    /// The price per share of a fill or partial fill.
    pub price: Option<Decimal>,
    /// The quantity of a fill or partial fill.
    pub qty: Option<Decimal>,
    /// The position's size after the event: positive long, negative short.
    pub position_qty: Option<Decimal>,
    /// The `at` time of a multi-leg fill's update, as sent (the docs do not
    /// say what it marks).
    pub at: Option<Timestamp>,
    /// The update's ID; seen on multi-leg fills.
    pub event_id: Option<String>,
    /// Each leg's position size after a multi-leg fill, by symbol.
    pub position_qtys: Option<BTreeMap<String, Decimal>>,
    /// The fills of a multi-leg order's legs.
    pub legs: Option<Vec<LegFill>>,
}

/// The fill of one leg of a multi-leg order.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LegFill {
    /// The fill's execution ID.
    pub execution_id: Option<String>,
    /// The leg's order ID.
    pub order_id: Option<String>,
    /// The leg's symbol.
    pub symbol: Option<String>,
    /// The quantity filled.
    pub qty: Option<Decimal>,
    /// The price per unit.
    pub price: Option<Decimal>,
    /// When the leg filled.
    pub timestamp: Option<Timestamp>,
}

/// A message of the stream that [`Client::trade_updates`](crate::Client::trade_updates)
/// opens.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "each message is moved once, to the caller; a box would cost an allocation per update"
)]
pub enum TradeStreamMessage {
    /// A trade update.
    Update(TradeUpdate),
    /// A `trade_updates` message suri could not read as a trade update, kept
    /// as sent: its order ID is still there to reconcile with.
    Undecoded(StreamMessage),
    /// A message of another stream, as sent.
    Other(StreamMessage),
}

/// Types a stream message: a `trade_updates` message whose `data` reads as
/// a trade update, else the message as sent.
fn typed(message: StreamMessage) -> TradeStreamMessage {
    if message.stream != "trade_updates" {
        return TradeStreamMessage::Other(message);
    }
    match serde_json::from_str(&message.data) {
        Ok(update) => TradeStreamMessage::Update(update),
        Err(_) => TradeStreamMessage::Undecoded(message),
    }
}

/// One message of a stream, as Alpaca sent it: the stream's name and its
/// `data`, the raw JSON text.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct StreamMessage {
    /// The stream the message belongs to, such as `trade_updates`.
    pub stream: String,
    /// The message's `data` member, exactly as sent.
    pub data: String,
}

/// A frame as sent: a stream message, or the `error` action.
#[derive(Deserialize)]
struct WireFrame {
    stream: Option<String>,
    action: Option<String>,
    /// Present whenever the member is, `null` included.
    #[serde(default, deserialize_with = "present")]
    data: Option<Box<RawValue>>,
}

/// Reads a member that is present, whatever its value, as `Some`.
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Box<RawValue>>, D::Error> {
    Box::<RawValue>::deserialize(deserializer).map(Some)
}

#[derive(Deserialize)]
struct ServerError {
    error_message: Option<String>,
}

#[derive(Deserialize)]
struct Authorization {
    status: String,
}

#[derive(Deserialize)]
struct Listening {
    streams: Vec<String>,
}

/// The authentication message, its members in the documented order.
#[derive(Serialize)]
struct Auth<'a> {
    action: &'static str,
    key: &'a str,
    secret: &'a str,
}

const LISTEN: &str = r#"{"action":"listen","data":{"streams":["trade_updates"]}}"#;

/// The trade-updates stream: one WebSocket connection, read only while
/// [`next`](TradeUpdates::next) or [`close`](TradeUpdates::close) runs.
pub struct TradeUpdates {
    connection: Connection,
}

impl fmt::Debug for TradeUpdates {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TradeUpdates { .. }")
    }
}

impl Client {
    /// Opens the trade-updates stream: connects to the Trading host's
    /// `/stream`, authenticates with the client's key pair and listens to
    /// `trade_updates`. The returned value owns the connection and reads it
    /// only while [`TradeUpdates::next`] or [`TradeUpdates::close`] runs.
    ///
    /// Needs the `stream` feature.
    pub async fn trade_updates(&self) -> Result<TradeUpdates, StreamError> {
        let span = tracing::info_span!(target: "suri", "suri.stream", stream = "trade_updates");
        match self.open_trade_updates().instrument(span.clone()).await {
            Ok(socket) => Ok(TradeUpdates {
                connection: Connection::new(socket, self, span),
            }),
            Err(error) => {
                span.in_scope(|| ended(error.kind.as_str()));
                Err(error)
            }
        }
    }

    async fn open_trade_updates(&self) -> Result<Socket, StreamError> {
        let opening = |kind| StreamError::new(kind, Phase::Opening);
        let mut url = websocket(self.base_url());
        url.set_path("/stream");
        let steps = async {
            let mut socket = connect(self, &url, &[]).await?;
            let [key, secret] = self.credentials();
            let auth = serde_json::to_string(&Auth {
                action: "auth",
                key,
                secret,
            })
            .expect("the authentication message serialises");
            send(&mut socket, Message::text(auth), Phase::Opening).await?;
            let reply =
                stream_message(self, read_opening(self, &mut socket).await?, Phase::Opening)?;
            match authorization(&reply) {
                Some(true) => {}
                Some(false) => return Err(opening(StreamErrorKind::Unauthorized)),
                None => return Err(opening(StreamErrorKind::Decode)),
            }
            tracing::debug!(name: "authenticated", target: "suri", "stream authenticated");

            send(&mut socket, Message::text(LISTEN), Phase::Opening).await?;
            let reply =
                stream_message(self, read_opening(self, &mut socket).await?, Phase::Opening)?;
            if authorization(&reply) == Some(false) {
                return Err(opening(StreamErrorKind::Unauthorized));
            }
            if reply.stream != "listening" {
                return Err(opening(StreamErrorKind::Decode));
            }
            let listening: Listening =
                serde_json::from_str(&reply.data).map_err(|_| opening(StreamErrorKind::Decode))?;
            if !listening.streams.iter().any(|name| name == "trade_updates") {
                return Err(opening(StreamErrorKind::NotListening));
            }
            tracing::debug!(name: "listening", target: "suri", "stream listening");
            Ok(socket)
        };
        tokio::time::timeout(self.request_timeout(), steps)
            .await
            .map_err(|_| opening(StreamErrorKind::Timeout))?
    }
}

impl TradeUpdates {
    /// The next message: a trade update, a `trade_updates` message suri
    /// could not read (kept as sent), or a message of another stream. Ping
    /// and Pong frames are handled and never delivered. After `request_timeout` without any frame, a Ping is sent;
    /// if nothing arrives within another `request_timeout`, the stream ends
    /// with `Timeout`. Every end other than [`close`](TradeUpdates::close)
    /// is an error saying later messages may be lost, and every later call
    /// returns `Ended`. Cancel-safe: dropping the future loses no message.
    pub async fn next(&mut self) -> Result<TradeStreamMessage, StreamError> {
        let span = self.connection.span().clone();
        async {
            let frame = self.connection.next_frame().await?;
            match stream_message(self.connection.client(), frame, Phase::Next) {
                Ok(message) => Ok(typed(message)),
                Err(error) => Err(self.connection.end(error)),
            }
        }
        .instrument(span)
        .await
    }

    /// Closes the stream: sends a Close frame and reads until Alpaca's Close
    /// arrives or `request_timeout` passes. Returns every message read
    /// meanwhile, whatever the outcome. Not cancel-safe: dropping the future
    /// drops the connection and the messages read so far.
    pub async fn close(self) -> (Vec<TradeStreamMessage>, Result<(), StreamError>) {
        self.connection
            .close(|client, frame| Ok(vec![typed(stream_message(client, frame, Phase::Close)?)]))
            .await
    }
}

/// A data frame as a stream message, or the error it ends the stream with.
fn stream_message(
    client: &Client,
    message: Message,
    phase: Phase,
) -> Result<StreamMessage, StreamError> {
    let decode = || StreamError::new(StreamErrorKind::Decode, phase);
    let bytes = match &message {
        Message::Text(text) => text.as_bytes(),
        Message::Binary(bytes) => bytes,
        _ => return Err(decode()),
    };
    let frame: WireFrame = serde_json::from_slice(bytes).map_err(|_| decode())?;
    if frame.action.as_deref() == Some("error") {
        let text = frame
            .data
            .and_then(|data| serde_json::from_str::<ServerError>(data.get()).ok())
            .and_then(|data| data.error_message);
        let mut error = StreamError::new(StreamErrorKind::Server, phase);
        error.message = text.map(|text| sanitise(&text, &client.credentials()));
        return Err(error);
    }
    match (frame.stream, frame.data) {
        (Some(stream), Some(data)) => Ok(StreamMessage {
            stream,
            data: data.get().to_owned(),
        }),
        _ => Err(decode()),
    }
}

/// Whether a message is the authorization reply, and if so whether it
/// authorised the connection.
fn authorization(message: &StreamMessage) -> Option<bool> {
    if message.stream != "authorization" {
        return None;
    }
    let reply: Authorization = serde_json::from_str(&message.data).ok()?;
    match reply.status.as_str() {
        "authorized" => Some(true),
        "unauthorized" => Some(false),
        _ => None,
    }
}

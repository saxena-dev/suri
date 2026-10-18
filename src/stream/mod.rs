//! The Trading stream over WebSocket: one connection, owned by the value the
//! caller polls, that never ends without saying so.

use std::fmt;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::value::RawValue;
use tokio::net::TcpStream;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::http::header::USER_AGENT;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, WebSocketConfig};
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::{Connector, MaybeTlsStream, WebSocketStream};
use tracing::{Instrument, Span};

use crate::Client;
use crate::rest::error::sanitise;
use crate::trading::TradeStreamMessage;
use crate::trading::updates::typed;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

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

/// What ended a stream, or kept it from opening.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamErrorKind {
    /// The stream's TLS configuration could not be built.
    Tls,
    /// The WebSocket upgrade failed: an HTTP status other than 101, or an
    /// extension suri did not offer.
    Handshake,
    /// Alpaca rejected the credentials.
    Unauthorized,
    /// Alpaca did not confirm the `trade_updates` stream.
    NotListening,
    /// Alpaca closed the stream.
    Closed,
    /// Alpaca reported an error on the stream before closing it.
    Server,
    /// The connection failed.
    Transport,
    /// A time bound passed: the opening, a liveness Ping or the close.
    Timeout,
    /// A message was larger than `max_body_bytes`.
    TooLarge,
    /// A frame was not a stream message.
    Decode,
    /// The stream had already ended.
    Ended,
}

impl StreamErrorKind {
    fn as_str(self) -> &'static str {
        match self {
            StreamErrorKind::Tls => "tls",
            StreamErrorKind::Handshake => "handshake",
            StreamErrorKind::Unauthorized => "unauthorized",
            StreamErrorKind::NotListening => "not_listening",
            StreamErrorKind::Closed => "closed",
            StreamErrorKind::Server => "server",
            StreamErrorKind::Transport => "transport",
            StreamErrorKind::Timeout => "timeout",
            StreamErrorKind::TooLarge => "too_large",
            StreamErrorKind::Decode => "decode",
            StreamErrorKind::Ended => "ended",
        }
    }
}

/// Where an error arose, which decides what its message says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Opening,
    Next,
    Close,
}

/// The error of a stream: why it ended or did not open.
pub struct StreamError {
    kind: StreamErrorKind,
    phase: Phase,
    status: Option<u16>,
    close_code: Option<u16>,
    message: Option<String>,
}

impl StreamError {
    fn new(kind: StreamErrorKind, phase: Phase) -> StreamError {
        StreamError {
            kind,
            phase,
            status: None,
            close_code: None,
            message: None,
        }
    }

    /// What went wrong.
    pub fn kind(&self) -> StreamErrorKind {
        self.kind
    }

    /// For `Handshake`, the upgrade response's HTTP status.
    pub fn status(&self) -> Option<u16> {
        self.status
    }

    /// For `Closed`, the Close frame's code, when it had one.
    pub fn close_code(&self) -> Option<u16> {
        self.close_code
    }

    /// For `Closed`, the Close frame's reason; for `Server`, Alpaca's error
    /// message. Credentials are replaced with `[REDACTED]`, control
    /// characters with spaces, and the text is cut to 1,024 bytes, as for
    /// [`crate::Error::message`].
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}

impl fmt::Debug for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamError")
            .field("kind", &self.kind)
            .field("status", &self.status)
            .field("close_code", &self.close_code)
            .field("message", &self.message)
            .finish()
    }
}

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.kind, self.phase) {
            (StreamErrorKind::Tls, _) => {
                f.write_str("the stream's TLS configuration could not be built")?
            }
            (StreamErrorKind::Handshake, _) => match self.status {
                Some(101) => {
                    f.write_str("the stream's upgrade accepted an extension suri did not offer")?
                }
                Some(status) => write!(f, "the stream's upgrade was answered with HTTP {status}")?,
                None => f.write_str("the stream's upgrade failed")?,
            },
            (StreamErrorKind::Unauthorized, _) => {
                f.write_str("Alpaca rejected the stream's credentials")?
            }
            (StreamErrorKind::NotListening, _) => {
                f.write_str("Alpaca did not confirm the trade_updates stream")?
            }
            (StreamErrorKind::Closed, _) => match self.close_code {
                Some(code) => write!(f, "Alpaca closed the stream with code {code}")?,
                None => f.write_str("Alpaca closed the stream")?,
            },
            (StreamErrorKind::Server, _) => f.write_str("Alpaca reported a stream error")?,
            (StreamErrorKind::Transport, _) => f.write_str("the stream's connection failed")?,
            (StreamErrorKind::Timeout, Phase::Opening) => {
                f.write_str("the stream did not open within its time bound")?
            }
            (StreamErrorKind::Timeout, Phase::Next) => {
                f.write_str("nothing arrived within request_timeout after a liveness ping")?
            }
            (StreamErrorKind::Timeout, Phase::Close) => {
                f.write_str("the server did not answer the close")?
            }
            (StreamErrorKind::TooLarge, _) => {
                f.write_str("a stream message was larger than max_body_bytes")?
            }
            (StreamErrorKind::Decode, _) => f.write_str("a stream frame was not a message")?,
            (StreamErrorKind::Ended, _) => f.write_str("the stream has already ended")?,
        }
        if let Some(message) = &self.message {
            write!(f, ": {message}")?;
        }
        match (self.kind, self.phase) {
            (StreamErrorKind::Ended, _) | (StreamErrorKind::Timeout, Phase::Close) => Ok(()),
            (_, Phase::Opening) => f.write_str("; the stream did not open"),
            _ => f.write_str("; later messages may be lost"),
        }
    }
}

impl std::error::Error for StreamError {}

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
    /// `None` once the stream has ended.
    socket: Option<Socket>,
    client: Client,
    span: Span,
    /// When the last frame of any kind arrived.
    last_frame: Instant,
    /// When the outstanding liveness Ping was written.
    ping_written: Option<Instant>,
    /// The error for a Close Alpaca sent, kept until the reply Close is
    /// flushed, so a cancelled `next` does not lose it.
    ending: Option<StreamError>,
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
    pub async fn trade_updates(&self) -> Result<TradeUpdates, StreamError> {
        let span = tracing::info_span!(target: "suri", "suri.stream", stream = "trade_updates");
        match self.open_stream().instrument(span.clone()).await {
            Ok(socket) => Ok(TradeUpdates {
                socket: Some(socket),
                client: self.clone(),
                span,
                last_frame: Instant::now(),
                ping_written: None,
                ending: None,
            }),
            Err(error) => {
                span.in_scope(|| ended(error.kind.as_str()));
                Err(error)
            }
        }
    }

    async fn open_stream(&self) -> Result<Socket, StreamError> {
        let opening = |kind| StreamError::new(kind, Phase::Opening);
        let tls = tls_config().map_err(|_| opening(StreamErrorKind::Tls))?;
        let mut url = self.base_url().clone();
        let scheme = if url.scheme() == "https" { "wss" } else { "ws" };
        url.set_scheme(scheme)
            .expect("http and https URLs take a ws or wss scheme");
        url.set_path("/stream");
        let mut request = url
            .as_str()
            .into_client_request()
            .expect("a checked base URL makes a valid request");
        request.headers_mut().insert(
            USER_AGENT,
            HeaderValue::from_static(crate::rest::SURI_USER_AGENT),
        );
        let max = usize::try_from(self.max_body_bytes())
            .expect("max_body_bytes is at most 1 GiB, which fits usize");
        let config = WebSocketConfig::default()
            .max_message_size(Some(max))
            .max_frame_size(Some(max));
        let steps = async {
            let connect = tokio_tungstenite::connect_async_tls_with_config(
                request,
                Some(config),
                true,
                Some(Connector::Rustls(Arc::new(tls))),
            );
            let (mut socket, response) = tokio::time::timeout(self.connect_timeout(), connect)
                .await
                .map_err(|_| opening(StreamErrorKind::Timeout))?
                .map_err(|error| failure(error, Phase::Opening))?;
            if response.headers().contains_key("sec-websocket-extensions") {
                let mut error = opening(StreamErrorKind::Handshake);
                error.status = Some(101);
                return Err(error);
            }
            tracing::debug!(name: "connected", target: "suri", "stream connected");

            let [key, secret] = self.credentials();
            let auth = serde_json::to_string(&Auth {
                action: "auth",
                key,
                secret,
            })
            .expect("the authentication message serialises");
            send(&mut socket, Message::text(auth), Phase::Opening).await?;
            let reply = self.read(&mut socket).await?;
            match authorization(&reply) {
                Some(true) => {}
                Some(false) => return Err(opening(StreamErrorKind::Unauthorized)),
                None => return Err(opening(StreamErrorKind::Decode)),
            }
            tracing::debug!(name: "authenticated", target: "suri", "stream authenticated");

            send(&mut socket, Message::text(LISTEN), Phase::Opening).await?;
            let reply = self.read(&mut socket).await?;
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

    /// Reads one stream message during the opening, skipping Ping and Pong.
    async fn read(&self, socket: &mut Socket) -> Result<StreamMessage, StreamError> {
        let phase = Phase::Opening;
        loop {
            match socket.next().await {
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
                Some(Ok(Message::Close(frame))) => {
                    let error = self.closed(frame, phase);
                    flush(socket, self.request_timeout()).await;
                    return Err(error);
                }
                Some(Ok(message)) => return self.message(message, phase),
                Some(Err(error)) => return Err(failure(error, phase)),
                None => return Err(StreamError::new(StreamErrorKind::Transport, phase)),
            }
        }
    }

    /// A data frame as a stream message, or the error it ends the stream with.
    fn message(&self, message: Message, phase: Phase) -> Result<StreamMessage, StreamError> {
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
            error.message = text.map(|text| sanitise(&text, &self.credentials()));
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

    /// The error for a server Close. The caller flushes the reply Close the
    /// library queued before the socket is dropped.
    fn closed(&self, frame: Option<CloseFrame>, phase: Phase) -> StreamError {
        let mut error = StreamError::new(StreamErrorKind::Closed, phase);
        if let Some(frame) = frame {
            error.close_code = Some(u16::from(frame.code));
            if !frame.reason.is_empty() {
                error.message = Some(sanitise(frame.reason.as_str(), &self.credentials()));
            }
        }
        error
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
        let span = self.span.clone();
        let message = self.next_message().instrument(span).await?;
        Ok(typed(message))
    }

    async fn next_message(&mut self) -> Result<StreamMessage, StreamError> {
        let wait = self.client.request_timeout();
        loop {
            let Some(socket) = self.socket.as_mut() else {
                return Err(StreamError::new(StreamErrorKind::Ended, Phase::Next));
            };
            if self.ending.is_some() {
                flush(socket, wait).await;
                let error = self.ending.take().expect("checked above");
                return Err(self.end(error));
            }
            let deadline = match self.ping_written {
                Some(written) => written + wait,
                None => self.last_frame + wait,
            };
            match tokio::time::timeout_at(deadline, socket.next()).await {
                Ok(item) => {
                    self.last_frame = Instant::now();
                    self.ping_written = None;
                    match item {
                        Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
                        Some(Ok(Message::Close(frame))) => {
                            // Kept before any await; the loop flushes the
                            // reply and returns it.
                            self.ending = Some(self.client.closed(frame, Phase::Next));
                        }
                        Some(Ok(message)) => {
                            return match self.client.message(message, Phase::Next) {
                                Ok(message) => Ok(message),
                                Err(error) => Err(self.end(error)),
                            };
                        }
                        Some(Err(error)) => return Err(self.end(failure(error, Phase::Next))),
                        None => {
                            let error = StreamError::new(StreamErrorKind::Transport, Phase::Next);
                            return Err(self.end(error));
                        }
                    }
                }
                Err(_) if self.ping_written.is_some() => {
                    let error = StreamError::new(StreamErrorKind::Timeout, Phase::Next);
                    return Err(self.end(error));
                }
                Err(_) => {
                    if let Err(error) =
                        send(socket, Message::Ping(Default::default()), Phase::Next).await
                    {
                        return Err(self.end(error));
                    }
                    self.ping_written = Some(Instant::now());
                    tracing::debug!(name: "ping", target: "suri", "liveness ping sent");
                }
            }
        }
    }

    /// Ends the stream: drops the socket and emits the `ended` event.
    fn end(&mut self, error: StreamError) -> StreamError {
        self.socket = None;
        ended(error.kind.as_str());
        error
    }

    /// Closes the stream: sends a Close frame and reads until Alpaca's Close
    /// arrives or `request_timeout` passes. Returns every message read
    /// meanwhile, whatever the outcome. Not cancel-safe: dropping the future
    /// drops the connection and the messages read so far.
    pub async fn close(mut self) -> (Vec<TradeStreamMessage>, Result<(), StreamError>) {
        let span = self.span.clone();
        async move {
            let Some(mut socket) = self.socket.take() else {
                return (
                    Vec::new(),
                    Err(StreamError::new(StreamErrorKind::Ended, Phase::Close)),
                );
            };
            // Alpaca's Close, read by a cancelled `next`, ends the stream
            // as `next` would have reported it.
            if let Some(error) = self.ending.take() {
                flush(&mut socket, self.client.request_timeout()).await;
                ended(error.kind.as_str());
                return (Vec::new(), Err(error));
            }
            let mut messages = Vec::new();
            let closing = async {
                let frame = CloseFrame {
                    code: CloseCode::Normal,
                    reason: "".into(),
                };
                send(&mut socket, Message::Close(Some(frame)), Phase::Close).await?;
                loop {
                    match socket.next().await {
                        Some(Ok(Message::Close(_))) => return Ok(()),
                        Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
                        Some(Ok(message)) => {
                            messages.push(typed(self.client.message(message, Phase::Close)?))
                        }
                        Some(Err(error)) => return Err(failure(error, Phase::Close)),
                        None => {
                            return Err(StreamError::new(StreamErrorKind::Transport, Phase::Close));
                        }
                    }
                }
            };
            let result = match tokio::time::timeout(self.client.request_timeout(), closing).await {
                Ok(result) => result,
                Err(_) => Err(StreamError::new(StreamErrorKind::Timeout, Phase::Close)),
            };
            match &result {
                Ok(()) => ended("closed_by_caller"),
                Err(error) => ended(error.kind.as_str()),
            }
            (messages, result)
        }
        .instrument(span)
        .await
    }
}

/// Writes one frame, mapping a failure to the stream's error.
async fn send(socket: &mut Socket, message: Message, phase: Phase) -> Result<(), StreamError> {
    socket
        .send(message)
        .await
        .map_err(|error| failure(error, phase))
}

/// Flushes the frames the library queued (a Pong, a reply Close), within
/// `wait`; the stream is ending or opening either way.
async fn flush(socket: &mut Socket, wait: std::time::Duration) {
    let _ = tokio::time::timeout(wait, socket.flush()).await;
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

/// The stream error for a WebSocket library error.
fn failure(error: tungstenite::Error, phase: Phase) -> StreamError {
    let kind = match &error {
        tungstenite::Error::Capacity(_) => StreamErrorKind::TooLarge,
        tungstenite::Error::Utf8(_) => StreamErrorKind::Decode,
        tungstenite::Error::Http(_) => StreamErrorKind::Handshake,
        _ => StreamErrorKind::Transport,
    };
    let mut stream_error = StreamError::new(kind, phase);
    if let tungstenite::Error::Http(response) = &error {
        stream_error.status = Some(response.status().as_u16());
    }
    stream_error
}

/// Emits the `ended` event, once per stream.
fn ended(kind: &'static str) {
    tracing::debug!(name: "ended", target: "suri", kind, "stream ended");
}

/// The WebSocket's TLS configuration: the process's installed provider, else
/// aws-lc-rs; the platform's certificate verifier; no ALPN; no key log.
fn tls_config() -> Result<rustls::ClientConfig, rustls::Error> {
    use rustls_platform_verifier::BuilderVerifierExt;
    let provider = rustls::crypto::CryptoProvider::get_default()
        .cloned()
        .unwrap_or_else(|| Arc::new(rustls::crypto::aws_lc_rs::default_provider()));
    Ok(rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_platform_verifier()?
        .with_no_client_auth())
}

#[cfg(test)]
mod tests {
    use super::tls_config;

    #[test]
    fn the_tls_configuration_offers_no_alpn() {
        let config = tls_config().unwrap();
        assert!(config.alpn_protocols.is_empty());
    }
}

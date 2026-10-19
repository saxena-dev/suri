//! The WebSocket core every stream shares: one connection, owned by the
//! value the caller polls, that never ends without saying so.

use std::fmt;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::USER_AGENT;
use tokio_tungstenite::tungstenite::http::{HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, WebSocketConfig};
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::{Connector, MaybeTlsStream, WebSocketStream};
use tracing::{Instrument, Span};

use crate::Client;
use crate::rest::error::sanitise;

pub(crate) type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

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
    /// Alpaca reported an error on the stream.
    Server,
    /// The connection failed.
    Transport,
    /// A time bound passed: the opening, a subscription, a liveness Ping or
    /// the close.
    Timeout,
    /// A message was larger than `max_body_bytes`.
    TooLarge,
    /// A frame was not a stream message.
    Decode,
    /// The stream had already ended.
    Ended,
}

impl StreamErrorKind {
    pub(crate) fn as_str(self) -> &'static str {
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
pub(crate) enum Phase {
    Opening,
    Subscribe,
    Next,
    Close,
}

/// The error of a stream: why it ended or did not open.
///
/// A stream that ends without your asking reports it once, as this error,
/// whose message says later messages may be lost; an error while opening
/// says the stream did not open. Every call after the end returns `Ended`.
/// There is no reconnection: open a new stream. Match on
/// [`kind`](StreamError::kind); a close reason or Alpaca's message is
/// scrubbed of your key pair.
pub struct StreamError {
    pub(crate) kind: StreamErrorKind,
    phase: Phase,
    pub(crate) status: Option<u16>,
    close_code: Option<u16>,
    pub(crate) code: Option<u16>,
    pub(crate) message: Option<String>,
}

impl StreamError {
    pub(crate) fn new(kind: StreamErrorKind, phase: Phase) -> StreamError {
        StreamError {
            kind,
            phase,
            status: None,
            close_code: None,
            code: None,
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

    /// For `Server` or `Unauthorized` ending a market-data stream's
    /// opening, the code of Alpaca's `error` message.
    pub fn code(&self) -> Option<u16> {
        self.code
    }

    /// For `Closed`, the Close frame's reason; for `Server`, and for
    /// `Unauthorized` on a market-data stream's opening, Alpaca's error
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
            .field("code", &self.code)
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
            (StreamErrorKind::Timeout, Phase::Subscribe) => {
                f.write_str("the subscription was not sent within request_timeout")?
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
        if let Some(code) = self.code {
            write!(f, " (code {code})")?;
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

/// Upgrades to `url` with the TLS configuration, the size limits, the
/// `User-Agent` and `headers`, within `connect_timeout`, and refuses an
/// upgrade that claims an extension. The caller bounds the whole opening.
pub(crate) async fn connect(
    client: &Client,
    url: &url::Url,
    headers: &[(&'static str, &HeaderValue)],
) -> Result<Socket, StreamError> {
    let opening = |kind| StreamError::new(kind, Phase::Opening);
    let tls = tls_config().map_err(|_| opening(StreamErrorKind::Tls))?;
    let mut request = url
        .as_str()
        .into_client_request()
        .expect("a checked base URL makes a valid request");
    request.headers_mut().insert(
        USER_AGENT,
        HeaderValue::from_static(crate::rest::SURI_USER_AGENT),
    );
    for &(name, value) in headers {
        request
            .headers_mut()
            .insert(HeaderName::from_static(name), value.clone());
    }
    let max = usize::try_from(client.max_body_bytes())
        .expect("max_body_bytes is at most 1 GiB, which fits usize");
    let config = WebSocketConfig::default()
        .max_message_size(Some(max))
        .max_frame_size(Some(max));
    let connect = tokio_tungstenite::connect_async_tls_with_config(
        request,
        Some(config),
        true,
        Some(Connector::Rustls(Arc::new(tls))),
    );
    let (socket, response) = tokio::time::timeout(client.connect_timeout(), connect)
        .await
        .map_err(|_| opening(StreamErrorKind::Timeout))?
        .map_err(|error| failure(error, Phase::Opening))?;
    if response.headers().contains_key("sec-websocket-extensions") {
        let mut error = opening(StreamErrorKind::Handshake);
        error.status = Some(101);
        return Err(error);
    }
    tracing::debug!(name: "connected", target: "suri", "stream connected");
    Ok(socket)
}

/// Reads the next data frame of the opening, skipping Ping and Pong.
pub(crate) async fn read_opening(
    client: &Client,
    socket: &mut Socket,
) -> Result<Message, StreamError> {
    let phase = Phase::Opening;
    loop {
        match socket.next().await {
            Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
            Some(Ok(Message::Close(frame))) => {
                let error = closed(client, frame, phase);
                flush(socket, client.request_timeout()).await;
                return Err(error);
            }
            Some(Ok(message)) => return Ok(message),
            Some(Err(error)) => return Err(failure(error, phase)),
            None => return Err(StreamError::new(StreamErrorKind::Transport, phase)),
        }
    }
}

/// An open stream's connection, read only while the caller polls it.
pub(crate) struct Connection {
    /// `None` once the stream has ended.
    socket: Option<Socket>,
    client: Client,
    span: Span,
    /// When the last frame of any kind arrived.
    last_frame: Instant,
    /// When the outstanding liveness Ping was written.
    ping_written: Option<Instant>,
    /// The error for a Close Alpaca sent, kept until the reply Close is
    /// flushed, so a cancelled call does not lose it.
    ending: Option<StreamError>,
}

impl Connection {
    pub(crate) fn new(socket: Socket, client: &Client, span: Span) -> Connection {
        Connection {
            socket: Some(socket),
            client: client.clone(),
            span,
            last_frame: Instant::now(),
            ping_written: None,
            ending: None,
        }
    }

    pub(crate) fn client(&self) -> &Client {
        &self.client
    }

    pub(crate) fn span(&self) -> &Span {
        &self.span
    }

    /// The next data frame. Ping and Pong frames are handled; after
    /// `request_timeout` without any frame a Ping is sent, and if nothing
    /// arrives within another `request_timeout` the stream ends with
    /// `Timeout`. Cancel-safe: nothing is awaited after a frame is read.
    pub(crate) async fn next_frame(&mut self) -> Result<Message, StreamError> {
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
                            self.ending = Some(closed(&self.client, frame, Phase::Next));
                        }
                        Some(Ok(message)) => return Ok(message),
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

    /// Writes one frame and flushes it within `request_timeout`. A failure
    /// ends the stream; a Close already read by a cancelled call is
    /// returned instead, and nothing is written.
    pub(crate) async fn write(&mut self, message: Message) -> Result<(), StreamError> {
        let wait = self.client.request_timeout();
        let Some(socket) = self.socket.as_mut() else {
            return Err(StreamError::new(StreamErrorKind::Ended, Phase::Subscribe));
        };
        if self.ending.is_some() {
            flush(socket, wait).await;
            let error = self.ending.take().expect("checked above");
            return Err(self.end(error));
        }
        match tokio::time::timeout(wait, socket.send(message)).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(self.end(failure(error, Phase::Subscribe))),
            Err(_) => {
                let error = StreamError::new(StreamErrorKind::Timeout, Phase::Subscribe);
                Err(self.end(error))
            }
        }
    }

    /// Ends the stream: drops the socket and emits the `ended` event.
    pub(crate) fn end(&mut self, error: StreamError) -> StreamError {
        self.socket = None;
        ended(error.kind.as_str());
        error
    }

    /// Sends a Close frame and reads until Alpaca's Close arrives or
    /// `request_timeout` passes, returning what `decode` makes of every
    /// data frame read meanwhile, whatever the outcome.
    pub(crate) async fn close<T>(
        mut self,
        mut decode: impl FnMut(&Client, Message) -> Result<Vec<T>, StreamError>,
    ) -> (Vec<T>, Result<(), StreamError>) {
        let span = self.span.clone();
        async move {
            let Some(mut socket) = self.socket.take() else {
                return (
                    Vec::new(),
                    Err(StreamError::new(StreamErrorKind::Ended, Phase::Close)),
                );
            };
            // Alpaca's Close, read by a cancelled call, ends the stream as
            // `next` would have reported it.
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
                        Some(Ok(message)) => messages.extend(decode(&self.client, message)?),
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

/// The error for a server Close. The caller flushes the reply Close the
/// library queued before the socket is dropped.
fn closed(client: &Client, frame: Option<CloseFrame>, phase: Phase) -> StreamError {
    let mut error = StreamError::new(StreamErrorKind::Closed, phase);
    if let Some(frame) = frame {
        error.close_code = Some(u16::from(frame.code));
        if !frame.reason.is_empty() {
            error.message = Some(sanitise(frame.reason.as_str(), &client.credentials()));
        }
    }
    error
}

/// Writes one frame, mapping a failure to the stream's error.
pub(crate) async fn send(
    socket: &mut Socket,
    message: Message,
    phase: Phase,
) -> Result<(), StreamError> {
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
pub(crate) fn ended(kind: &'static str) {
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
    use super::{Phase, StreamError, StreamErrorKind, tls_config};

    #[test]
    fn the_tls_configuration_offers_no_alpn() {
        let config = tls_config().unwrap();
        assert!(config.alpn_protocols.is_empty());
    }

    #[test]
    fn a_subscription_error_says_later_messages_may_be_lost() {
        let timeout = StreamError::new(StreamErrorKind::Timeout, Phase::Subscribe);
        assert_eq!(
            timeout.to_string(),
            "the subscription was not sent within request_timeout; later messages may be lost"
        );
        let failed = StreamError::new(StreamErrorKind::Transport, Phase::Subscribe);
        assert_eq!(
            failed.to_string(),
            "the stream's connection failed; later messages may be lost"
        );
    }
}

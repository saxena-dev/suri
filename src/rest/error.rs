//! The error every call returns, and how far its request got.

use std::fmt;

use reqwest::Method;

use crate::ResponseMeta;
use crate::trading::ClientOrderId;

/// The kind of failure a call met.
///
/// Alpaca answered with an error or a redirect (`Api`, `Redirect`), the
/// connection failed or ran out of time (`Transport`, `Timeout`), or the
/// answer could not be read (`BodyTooLarge`, `Decode`).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Alpaca answered with a status other than success or redirect.
    Api,
    /// Alpaca answered with a redirect, which suri never follows.
    Redirect,
    /// The connection failed.
    Transport,
    /// A timeout expired.
    Timeout,
    /// The response body was larger than the configured limit.
    BodyTooLarge,
    /// The response body did not have the documented shape.
    Decode,
}

/// How far the request got before the error.
///
/// Only `NotStarted` means Alpaca never saw the request. At any later stage
/// a request that changes your account may have taken effect, which is what
/// [`Error::may_have_reached_broker`] tells you.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// No byte of the request was sent: Alpaca never saw it.
    NotStarted,
    /// The response's status line arrived.
    ResponseReceived,
    /// The request may or may not have reached Alpaca.
    Unknown,
}

/// The error of a failed call: what went wrong, how far the request got, and
/// what Alpaca said.
///
/// Match on [`kind`](Error::kind) to decide what to do. Before you send a
/// request that changes your account again, check
/// [`may_have_reached_broker`](Error::may_have_reached_broker): when it is
/// true, the request may have taken effect. For an order, look it up by its
/// [`client_order_id`](Error::client_order_id) with
/// [`Client::order_by_client_order_id`](crate::Client::order_by_client_order_id).
///
/// For an [`Api`](ErrorKind::Api) error, match on [`status`](Error::status)
/// and [`code`](Error::code): Alpaca states that its message texts can
/// change. Neither `Display` nor `Debug` shows your key pair (the
/// [contract](https://github.com/saxena-dev/suri/blob/v0.1.0/docs/contract.md)'s
/// Errors section gives the scrubbing's exact limits).
pub struct Error(Box<Inner>);

#[derive(Debug)]
struct Inner {
    kind: ErrorKind,
    stage: Stage,
    method: Method,
    endpoint: &'static str,
    attempt: u32,
    meta: Option<ResponseMeta>,
    location: Option<String>,
    code: Option<u64>,
    message: Option<String>,
    client_order_id: Option<ClientOrderId>,
}

impl Error {
    pub(crate) fn new(
        kind: ErrorKind,
        stage: Stage,
        method: Method,
        endpoint: &'static str,
        attempt: u32,
    ) -> Error {
        Error(Box::new(Inner {
            kind,
            stage,
            method,
            endpoint,
            attempt,
            meta: None,
            location: None,
            code: None,
            message: None,
            client_order_id: None,
        }))
    }

    pub(crate) fn with_meta(mut self, meta: ResponseMeta) -> Error {
        self.0.meta = Some(meta);
        self
    }

    pub(crate) fn with_location(mut self, location: Option<String>) -> Error {
        self.0.location = location;
        self
    }

    /// Adds an error body's code and message, the message already sanitised.
    pub(crate) fn with_details(mut self, code: Option<u64>, message: Option<String>) -> Error {
        self.0.code = code;
        self.0.message = message;
        self
    }

    /// Adds the client order ID of the order the call placed or replaced.
    pub(crate) fn with_client_order_id(mut self, id: ClientOrderId) -> Error {
        self.0.client_order_id = Some(id);
        self
    }

    /// What went wrong.
    pub fn kind(&self) -> ErrorKind {
        self.0.kind
    }

    /// How far the request got.
    pub fn stage(&self) -> Stage {
        self.0.stage
    }

    /// False only when the request was never sent. When true, a request that
    /// changes state may have taken effect: find out before sending it again.
    pub fn may_have_reached_broker(&self) -> bool {
        self.0.stage != Stage::NotStarted
    }

    /// The request's HTTP method.
    pub fn method(&self) -> &str {
        self.0.method.as_str()
    }

    /// The endpoint's path template, such as `/v2/clock`.
    pub fn endpoint(&self) -> &'static str {
        self.0.endpoint
    }

    /// The attempt that failed, counting from 1.
    pub fn attempt(&self) -> u32 {
        self.0.attempt
    }

    /// The HTTP status, when a response arrived.
    pub fn status(&self) -> Option<u16> {
        self.0.meta.as_ref().map(ResponseMeta::status)
    }

    /// The response's status and headers, when a response arrived.
    pub fn meta(&self) -> Option<&ResponseMeta> {
        self.0.meta.as_ref()
    }

    /// For a redirect, the scheme, host and port it pointed to, when its
    /// `Location` was an absolute http or https URL.
    pub fn location(&self) -> Option<&str> {
        self.0.location.as_deref()
    }

    /// For an `Api` error, Alpaca's numeric error code, when its body had
    /// one. The code and the status are stable; the message is not.
    pub fn code(&self) -> Option<u64> {
        self.0.code
    }

    /// For an `Api` error, Alpaca's message: the body's `message`; for a body
    /// that is a JSON string, the string itself; otherwise its whole text
    /// when it had neither a code nor a message. Credentials are
    /// replaced with `[REDACTED]`, control characters with spaces, and the
    /// text is cut to 1,024 bytes.
    pub fn message(&self) -> Option<&str> {
        self.0.message.as_deref()
    }

    /// For an order placement or replacement, the order's client order ID,
    /// whatever went wrong: the key to look the order up by when the call
    /// [may have reached Alpaca](Error::may_have_reached_broker).
    pub fn client_order_id(&self) -> Option<&ClientOrderId> {
        self.0.client_order_id.as_ref()
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = &self.0;
        let status = self.status().unwrap_or_default();
        match (inner.kind, inner.stage) {
            (ErrorKind::Api, _) => {
                write!(f, "Alpaca returned HTTP {status}")?;
                if let Some(code) = inner.code {
                    write!(f, " (code {code})")?;
                }
                if let Some(message) = &inner.message {
                    write!(f, ": {message}")?;
                }
            }
            (ErrorKind::Redirect, _) => match &inner.location {
                Some(location) => write!(
                    f,
                    "Alpaca answered HTTP {status} with a redirect to {location}; it was not followed"
                )?,
                None => write!(
                    f,
                    "Alpaca answered HTTP {status} with a redirect; it was not followed"
                )?,
            },
            (ErrorKind::Transport, Stage::NotStarted) => {
                f.write_str("the connection could not be established")?
            }
            (ErrorKind::Transport, Stage::Unknown) => {
                f.write_str("the connection failed before a response arrived")?
            }
            (ErrorKind::Transport, Stage::ResponseReceived) => {
                f.write_str("the connection failed while the response was read")?
            }
            (ErrorKind::Timeout, Stage::NotStarted) => {
                f.write_str("the connection was not established within connect_timeout")?
            }
            (ErrorKind::Timeout, Stage::Unknown) => {
                f.write_str("no response within request_timeout or read_deadline")?
            }
            (ErrorKind::Timeout, Stage::ResponseReceived) => f.write_str(
                "the response was not complete within request_timeout or read_deadline",
            )?,
            (ErrorKind::BodyTooLarge, _) => {
                f.write_str("the response body exceeded max_body_bytes")?
            }
            (ErrorKind::Decode, _) => {
                f.write_str("the response did not match the documented shape")?
            }
        }
        write!(
            f,
            " [{} {}, attempt {}]",
            inner.method, inner.endpoint, inner.attempt
        )?;
        if let Some(id) = inner.meta.as_ref().and_then(ResponseMeta::request_id) {
            write!(f, " [request id {id}]")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

/// The most of an error body's text an `Error` keeps, in bytes.
const KEPT_BYTES: usize = 1024;

/// The code and message of an error response's body (`body` is `None` when
/// it could not be read whole), each kept string passed through
/// [`sanitise`].
pub(crate) fn details(body: Option<&[u8]>, credentials: &[&str]) -> (Option<u64>, Option<String>) {
    let Some(body) = body.filter(|body| !body.is_empty()) else {
        return (None, None);
    };
    let (code, message) = match serde_json::from_slice::<serde_json::Value>(body) {
        Ok(serde_json::Value::Object(members)) => {
            let code = members.get("code").and_then(serde_json::Value::as_u64);
            let message = members
                .get("message")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            (code, message)
        }
        Ok(serde_json::Value::String(text)) => (None, Some(text)),
        _ => (None, None),
    };
    let message = match (code, message) {
        (None, None) => Some(String::from_utf8_lossy(body).into_owned()),
        (_, message) => message,
    };
    (code, message.map(|text| sanitise(&text, credentials)))
}

/// Replaces each credential, and a prefix of 8 or more characters of one
/// that ends the text, with `[REDACTED]`; then each control character with
/// a space, so the text stays on one line; then cuts it to 1,024 bytes at a
/// character boundary.
pub(crate) fn sanitise(text: &str, credentials: &[&str]) -> String {
    const MARK: &str = "[REDACTED]";
    let mut text = text.to_owned();
    for credential in credentials {
        text = text.replace(credential, MARK);
    }
    for credential in credentials {
        // Credentials are ASCII, so every byte offset is a character boundary.
        let tail = (8..credential.len())
            .rev()
            .find(|&length| text.ends_with(&credential[..length]));
        if let Some(length) = tail {
            text.truncate(text.len() - length);
            text.push_str(MARK);
        }
    }
    let mut text: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if text.len() > KEPT_BYTES {
        let cut = (0..=KEPT_BYTES)
            .rev()
            .find(|&at| text.is_char_boundary(at))
            .unwrap_or(0);
        text.truncate(cut);
    }
    text
}

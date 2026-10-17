//! The error every call returns, and how far its request got.

use std::fmt;

use reqwest::Method;

use crate::ResponseMeta;

/// What went wrong.
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

/// The error of a call: its kind, its stage and what is known of the request
/// and response.
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
            (ErrorKind::Api, _) => write!(f, "Alpaca returned HTTP {status}")?,
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
                f.write_str("no response within request_timeout")?
            }
            (ErrorKind::Timeout, Stage::ResponseReceived) => {
                f.write_str("the response was not complete within request_timeout")?
            }
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

//! What every response carries besides its data.

use reqwest::header::HeaderMap;

use crate::Timestamp;

/// The status and headers of a response that arrived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseMeta {
    status: u16,
    request_id: Option<String>,
    rate_limit: Option<RateLimit>,
}

impl ResponseMeta {
    /// The HTTP status code.
    pub fn status(&self) -> u16 {
        self.status
    }

    /// The `X-Request-ID` header, which Alpaca's support asks for, when sent.
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }

    /// The rate-limit headers, when all three were sent and read.
    pub fn rate_limit(&self) -> Option<RateLimit> {
        self.rate_limit
    }

    pub(crate) fn from_headers(status: u16, headers: &HeaderMap) -> ResponseMeta {
        let text = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
        let number = |name: &str| text(name).and_then(|t| t.parse::<u64>().ok());
        let rate_limit = match (
            number("x-ratelimit-limit"),
            number("x-ratelimit-remaining"),
            number("x-ratelimit-reset"),
        ) {
            (Some(limit), Some(remaining), Some(reset)) => i64::try_from(reset)
                .ok()
                .and_then(|seconds| Timestamp::from_unix(seconds, 0))
                .map(|reset| RateLimit {
                    limit,
                    remaining,
                    reset,
                }),
            _ => None,
        };
        ResponseMeta {
            status,
            request_id: text("x-request-id").map(str::to_owned),
            rate_limit,
        }
    }
}

/// The `X-RateLimit-Limit`, `X-RateLimit-Remaining` and `X-RateLimit-Reset`
/// headers of a response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimit {
    limit: u64,
    remaining: u64,
    reset: Timestamp,
}

impl RateLimit {
    /// Requests allowed in the current window.
    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// Requests left in the current window.
    pub fn remaining(&self) -> u64 {
        self.remaining
    }

    /// When the window resets, read as Unix seconds.
    pub fn reset(&self) -> Timestamp {
        self.reset
    }
}

/// A successful response: its data and its metadata.
#[derive(Debug, Clone)]
pub struct Response<T> {
    data: T,
    meta: ResponseMeta,
}

impl<T> Response<T> {
    pub(crate) fn new(data: T, meta: ResponseMeta) -> Response<T> {
        Response { data, meta }
    }

    /// The decoded data.
    pub fn data(&self) -> &T {
        &self.data
    }

    /// The status and headers.
    pub fn meta(&self) -> &ResponseMeta {
        &self.meta
    }

    /// The data and the metadata, by value.
    pub fn into_parts(self) -> (T, ResponseMeta) {
        (self.data, self.meta)
    }
}

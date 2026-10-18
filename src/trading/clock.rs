//! The market clock.

use reqwest::Method;
use serde::Deserialize;

use crate::rest::client::Endpoint;
use crate::{Client, Error, Response, Timestamp};

/// Whether the US equity market is open, and its next open and close.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Clock {
    /// The server's current time.
    pub timestamp: Timestamp,
    /// Whether the market is open now.
    pub is_open: bool,
    /// The next market open.
    pub next_open: Timestamp,
    /// The next market close.
    pub next_close: Timestamp,
}

const CLOCK: Endpoint = Endpoint {
    method: Method::GET,
    template: "/v2/clock",
};

impl Client {
    /// Reads the market clock (`GET /v2/clock`).
    pub async fn clock(&self) -> Result<Response<Clock>, Error> {
        self.send(&CLOCK, &["v2", "clock"]).await
    }
}

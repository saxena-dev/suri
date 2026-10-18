//! The HTTP transport.

/// The `User-Agent` every request and stream upgrade sends.
pub(crate) const SURI_USER_AGENT: &str = concat!("suri/", env!("CARGO_PKG_VERSION"));

pub(crate) mod client;
pub(crate) mod config;
pub(crate) mod error;
mod response;

pub use client::Client;
pub use config::{Config, ConfigError, Credentials, Environment, Region};
pub use error::{Error, ErrorKind, Stage};
pub use response::{RateLimit, Response, ResponseMeta};

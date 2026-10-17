//! A Rust client for the Alpaca Trading and Market Data APIs.
//!
//! Pre-release: the market clock is the only endpoint yet.
//!
//! The [contract](https://github.com/saxena-dev/suri/blob/main/docs/contract.md)
//! states every behaviour suri promises.

#[cfg(feature = "rest")]
mod client;
#[cfg(feature = "rest")]
mod clock;
#[cfg(feature = "rest")]
mod config;
mod decimal;
#[cfg(feature = "rest")]
mod error;
mod inbound;
mod path;
#[cfg(feature = "rest")]
mod response;
mod time;

#[cfg(feature = "rest")]
pub use client::Client;
#[cfg(feature = "rest")]
pub use clock::Clock;
#[cfg(feature = "rest")]
pub use config::{Config, ConfigError, Credentials, Environment, Region};
pub use decimal::{Decimal, ParseDecimalError};
#[cfg(feature = "rest")]
pub use error::{Error, ErrorKind, Stage};
pub use inbound::Inbound;
pub use path::{Id, ParsePathValueError, Symbol};
#[cfg(feature = "rest")]
pub use response::{RateLimit, Response, ResponseMeta};
pub use time::{Date, ParseTimeError, Timestamp};

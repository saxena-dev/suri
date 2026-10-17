//! A Rust client for the Alpaca Trading and Market Data APIs.
//!
//! Pre-release: value types only; no client yet.
//!
//! The [contract](https://github.com/saxena-dev/suri/blob/main/docs/contract.md)
//! states every behaviour suri promises.

mod decimal;
mod inbound;
mod time;

pub use decimal::{Decimal, ParseDecimalError};
pub use inbound::Inbound;
pub use time::{Date, ParseTimeError, Timestamp};

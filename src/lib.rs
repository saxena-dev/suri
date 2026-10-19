//! A Rust client for the Alpaca Trading and Market Data APIs.
//!
//! 0.1.0 is the first release. It covers the Trading API's REST endpoints
//! (account, orders, positions, assets, option contracts, the clock and
//! calendar, watchlists, and account activities and history) and its
//! `trade_updates` stream, and Market Data's REST endpoints and streams for
//! stocks, crypto, options and news, on Alpaca's US hosts; the streams are
//! behind the `stream` feature.
//!
//! The [contract](https://github.com/saxena-dev/suri/blob/v0.1.0/docs/contract.md)
//! states every behaviour suri promises, and the
//! [migration guide](https://github.com/saxena-dev/suri/blob/v0.1.0/docs/migration.md)
//! lists every break between versions.
//!
//! # Where things are
//!
//! The values every API shares (`Decimal`, `Timestamp`, `Date`, `Inbound`,
//! `Symbol`, `Id`) and, with the `rest` feature, the transport (`Client`,
//! `Config`, `Error`, `Response` and their companions) are at the crate
//! root; with `stream`, so are `StreamError` and `StreamErrorKind`. The
//! Trading API's types (account, assets, calendar, clock, orders,
//! positions, watchlists and the `trade_updates` stream's messages) are in
//! the `trading` module, and Market Data's in `data` and its per-asset
//! modules (`data::stocks`, `data::crypto`, `data::options`) and
//! `data::news`. The methods that call Alpaca are on `Client`.

// The README's examples compile as doc tests, so they cannot drift from the
// API. They use every feature, so they are checked when all are enabled.
#[cfg(all(doctest, feature = "rest", feature = "stream"))]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(feature = "rest")]
pub mod data;
#[cfg(feature = "rest")]
mod rest;
#[cfg(feature = "stream")]
mod stream;
#[cfg(feature = "rest")]
pub mod trading;
mod value;

#[cfg(feature = "rest")]
pub use rest::{
    Client, Config, ConfigError, Credentials, Environment, Error, ErrorKind, RateLimit, Region,
    Response, ResponseMeta, Stage,
};
#[cfg(feature = "stream")]
pub use stream::{StreamError, StreamErrorKind};
pub use value::{
    Date, Decimal, Id, Inbound, ParseDecimalError, ParsePathValueError, ParseTimeError, Symbol,
    Timestamp,
};
#[cfg(feature = "rest")]
pub use value::{ParseEnumError, Sort};

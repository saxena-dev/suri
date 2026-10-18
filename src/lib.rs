//! A Rust client for the Alpaca Trading and Market Data APIs.
//!
//! Pre-release: the Trading API's REST endpoints in the first release's
//! scope (see the README), its `trade_updates` stream behind the `stream`
//! feature, and Market Data's stock, crypto and options data and news,
//! with the stock, crypto and news streams behind `stream`, so far.
//!
//! The [contract](https://github.com/saxena-dev/suri/blob/main/docs/contract.md)
//! states every behaviour suri promises.
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

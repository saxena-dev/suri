//! An asynchronous Rust SDK for [Alpaca](https://alpaca.markets)'s Trading
//! and Market Data APIs: the REST API for your account, orders, positions,
//! assets and watchlists, the market data for stocks, crypto, options and
//! news, and the WebSocket streams for trade updates and live market data,
//! on Alpaca's US hosts.
//!
//! suri is built so that nothing goes wrong quietly. A failed call is never
//! reported as success, an order is never sent twice behind your back, a
//! stream message is never dropped without an error, and your credentials
//! never appear in suri's logs, spans or errors. When a call fails, its
//! error tells you what failed and how far the request got, so you can
//! decide what to do next.
//!
//! # Where to start
//!
//! | To… | Start with |
//! |---|---|
//! | connect to Alpaca | [`Client`] and [`Config`] |
//! | read your account | [`Client::account`] |
//! | place an order and find it again | [`Client::place_order`], [`OrderRequest`](trading::OrderRequest) and [`Client::order_by_client_order_id`] |
//! | read positions, assets, the calendar or watchlists | [`trading`] |
//! | read market data | [`data::stocks`], [`data::crypto`], [`data::options`] and [`data::news`] |
//! | follow your orders as they change | [`Client::trade_updates`] |
//! | stream live market data | [`Client::stock_stream`] and [`DataStream`](data::DataStream) |
//! | handle failures | [`Error`], [`ErrorKind`] and [`Stage`] |
//! | keep prices and quantities exact | [`Decimal`] |
//! | change timeouts, retries or limits | [`Config`] |
//!
//! # A first program
//!
//! ```no_run
//! # #[cfg(feature = "rest")]
//! # mod example {
//! use suri::{Client, Config, Credentials, Environment, Region};
//!
//! # pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! // Pass the key pair in: suri reads no `APCA_API_*` variable.
//! let credentials = Credentials::api_key("your key ID", "your secret");
//! let client = Client::new(Config::new(Region::US, Environment::Paper, credentials))?;
//!
//! let account = client.account().await?;
//! println!("account {} is {:?}", account.data().id, account.data().status);
//! # Ok(()) }
//! # }
//! ```
//!
//! The [README](https://github.com/saxena-dev/suri#readme) walks through
//! placing an order safely, reading market data page by page, streaming,
//! and handling errors, step by step.
//!
//! # Features
//!
//! | Feature | What it adds | Default |
//! |---|---|---|
//! | `rest` | [`Client`] and the REST endpoints suri covers, Trading and Market Data | yes |
//! | `stream` | the `trade_updates` stream and the Market Data streams over WebSocket; requires `rest` | no |
//!
//! With no features, suri builds its value types only ([`Decimal`],
//! [`Timestamp`], [`Date`], [`Inbound`], [`Symbol`], [`Id`]), with no
//! network stack.
//!
//! # What you can rely on
//!
//! - **Success means success.** A call succeeds only when Alpaca answers
//!   with a success status and, for a call that returns data, a body of
//!   the documented shape.
//! - **Changes are sent at most once.** A request that changes your
//!   account is never retried. When its response is lost,
//!   [`Error::may_have_reached_broker`] says so, and the client order ID of
//!   an order you placed looks it up.
//! - **Reads retry within bounds.** A read is retried after a 429, a 500,
//!   502, 503 or 504, or a failed connection, with jittered backoff, within
//!   the limits in [`Config`].
//! - **Streams end loudly.** Every end other than `close` is a
//!   [`StreamError`] saying later messages may be lost; there is no silent
//!   reconnection.
//! - **Money sent as text stays exact.** Amounts Alpaca sends as decimal
//!   strings arrive as [`Decimal`], digit for digit. Values sent as JSON
//!   numbers, such as market-data prices and portfolio history, arrive as
//!   `f64`, and documented integers as integers.
//! - **Unknown values are kept.** A value Alpaca adds later arrives as
//!   [`Inbound::Unknown`] with its exact text.
//!
//! The [contract](https://github.com/saxena-dev/suri/blob/v0.1.0/docs/contract.md)
//! states each of these, and every other behaviour, in detail, and the
//! [migration guide](https://github.com/saxena-dev/suri/blob/v0.1.0/docs/migration.md)
//! lists every break between versions.
//!
//! # Where things are
//!
//! The values every API shares ([`Decimal`], [`Timestamp`], [`Date`],
//! [`Inbound`], [`Symbol`], [`Id`]) and, with the `rest` feature, the
//! transport ([`Client`], [`Config`], [`Error`], [`Response`] and their
//! companions) are at the crate root; with `stream`, so are [`StreamError`]
//! and [`StreamErrorKind`]. The Trading API's types (account, assets,
//! calendar, clock, orders, positions, watchlists and the `trade_updates`
//! stream's messages) are in [`trading`], and Market Data's in [`data`],
//! its per-asset modules ([`data::stocks`], [`data::crypto`],
//! [`data::options`]) and [`data::news`]. The methods that call Alpaca are
//! on [`Client`].
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/saxena-dev/suri/v0.1.0/.github/assets/icon.png",
    html_favicon_url = "https://raw.githubusercontent.com/saxena-dev/suri/v0.1.0/.github/assets/icon.png"
)]

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

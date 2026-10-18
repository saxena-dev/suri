//! Market Data: prices, sizes and news from Alpaca's data host.
//!
//! Each asset class has its own module, `stocks`, with the types its
//! `Client` methods return and take; this module holds what they share.
//! Prices are `f64`, read correctly rounded; sizes and counts are integers
//! of the width their format gives. One call returns one page.
//!
//! Glob-import one family module at a time: `stocks` has its own
//! `BarsQuery` and `HistoryQuery`.

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;

use crate::Timestamp;
use crate::value::wire::wire_enum;

pub mod stocks;

wire_enum! {
    /// The kind of tick a condition code applies to.
    TickType {
        /// Trades.
        Trade => "trade",
        /// Quotes.
        Quote => "quote",
    }
}

/// One page of points per symbol: bars, trades, quotes or auctions.
///
/// Points are in the order Alpaca sent them. The page may hold fewer points
/// than the query's `limit` even when more exist: only a `None`
/// `next_page_token` means there is no next page.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Series<T> {
    /// Each symbol's points.
    #[serde(
        rename = "bars",
        alias = "trades",
        alias = "quotes",
        alias = "auctions"
    )]
    pub items: BTreeMap<String, Vec<T>>,
    /// The currency of the prices, when Alpaca names it.
    pub currency: Option<String>,
    /// The token for the next page; `None` on the last page.
    pub next_page_token: Option<String>,
}

/// The latest point per symbol: a bar, a trade or a quote.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Latest<T> {
    /// Each symbol's latest point; a symbol with no data is absent.
    #[serde(rename = "bars", alias = "trades", alias = "quotes")]
    pub items: BTreeMap<String, T>,
    /// The currency of the prices, when Alpaca names it.
    pub currency: Option<String>,
}

/// An OHLC aggregate of the trades in one interval, for stocks and options.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Bar {
    /// The start of the interval.
    #[serde(rename = "t")]
    pub timestamp: Timestamp,
    /// The opening price.
    #[serde(rename = "o")]
    pub open: f64,
    /// The highest price.
    #[serde(rename = "h")]
    pub high: f64,
    /// The lowest price.
    #[serde(rename = "l")]
    pub low: f64,
    /// The closing price.
    #[serde(rename = "c")]
    pub close: f64,
    /// The volume.
    #[serde(rename = "v")]
    pub volume: i64,
    /// The number of trades.
    #[serde(rename = "n")]
    pub trade_count: i64,
    /// The volume-weighted average price.
    #[serde(rename = "vw")]
    pub vwap: f64,
}

/// The length of a bar's interval. It is sent in Alpaca's long form
/// (`15Min`, `1Hour`, `1Day`, `1Week`, `3Month`). Alpaca accepts 1 to 59
/// minutes, 1 to 23 hours, one day, one week, and 1, 2, 3, 4, 6 or 12
/// months; another amount is sent as given, and Alpaca rejects it.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Timeframe {
    /// This many minutes.
    Minutes(u8),
    /// This many hours.
    Hours(u8),
    /// One day.
    Day,
    /// One week.
    Week,
    /// This many months.
    Months(u8),
}

impl fmt::Display for Timeframe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Timeframe::Minutes(amount) => write!(f, "{amount}Min"),
            Timeframe::Hours(amount) => write!(f, "{amount}Hour"),
            Timeframe::Day => f.write_str("1Day"),
            Timeframe::Week => f.write_str("1Week"),
            Timeframe::Months(amount) => write!(f, "{amount}Month"),
        }
    }
}

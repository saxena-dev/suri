//! Market Data: prices, sizes and news from Alpaca's data host.
//!
//! Each asset class has its own module (`stocks`, `crypto`, `options`), as
//! does `news`, with the types its `Client` methods return and take; this
//! module holds what they share.
//! Prices, and crypto sizes and volumes, are `f64`, read correctly rounded;
//! other sizes and counts are integers of the width their format gives. One
//! call returns one page. With the `stream` feature, a `DataStream` reads a
//! Market Data stream, subscribed with `Channels`.
//!
//! Glob-import one family module at a time: `stocks` has its own
//! `BarsQuery` and `HistoryQuery`, and `crypto` its own `Bar`.

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;

use crate::rest::client::Request;
use crate::value::wire::wire_enum;
use crate::{Sort, Timestamp};

pub mod crypto;
pub mod news;
pub mod options;
pub mod stocks;
#[cfg(feature = "stream")]
mod stream;

#[cfg(feature = "stream")]
pub use stream::{Channels, DataMessage, DataStream};

/// How a stream's message type is read; private, so only suri implements
/// `DataMessage`.
#[cfg(feature = "stream")]
mod sealed {
    /// How a stream's frames are encoded.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Codec {
        /// JSON, in text or binary frames.
        Json,
        /// MessagePack, in binary frames.
        MessagePack,
    }

    pub trait Sealed {
        /// The stream's name, the `stream` field of its span.
        const STREAM: &'static str;

        /// The stream's encoding: how its opening frames and its frames
        /// are read and how a subscription is written.
        const CODEC: Codec;

        /// Reads one element of a frame, its exact bytes; every element
        /// becomes a message. `credentials` scrub an `error` message.
        fn decode(element: &[u8], credentials: &[&str; 2]) -> Self;
    }
}

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

/// The latest point per symbol: a bar, a trade, a quote, an orderbook or a
/// snapshot.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Latest<T> {
    /// Each symbol's latest point; a symbol with no data is absent.
    #[serde(
        rename = "bars",
        alias = "trades",
        alias = "quotes",
        alias = "orderbooks",
        alias = "snapshots"
    )]
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

/// Which bars to read, for the asset classes whose bars take no other
/// parameter (crypto, options). One call returns one page: to read the
/// next, set `page_token` to the previous page's `next_page_token`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct BarsQuery {
    /// The symbols, sent comma-joined.
    pub symbols: Vec<String>,
    /// The bars' interval.
    pub timeframe: Timeframe,
    /// The inclusive start; Alpaca's default is the start of the current
    /// day, but at least 15 minutes ago without real-time access.
    pub start: Option<Timestamp>,
    /// The inclusive end; Alpaca's default is now, or 15 minutes ago without
    /// real-time access.
    pub end: Option<Timestamp>,
    /// The most bars in the page, across all symbols (Alpaca: 1 to 10,000,
    /// default 1,000).
    pub limit: Option<u32>,
    /// The page to read, from a previous page's `next_page_token`.
    pub page_token: Option<String>,
    /// Oldest or newest first, by time.
    pub sort: Option<Sort>,
}

impl BarsQuery {
    /// Bars of these symbols at this interval, every other parameter
    /// Alpaca's default.
    pub fn new(symbols: Vec<String>, timeframe: Timeframe) -> BarsQuery {
        BarsQuery {
            symbols,
            timeframe,
            start: None,
            end: None,
            limit: None,
            page_token: None,
            sort: None,
        }
    }

    /// Adds the pairs in the docs' order.
    pub(crate) fn pairs(&self, request: &mut Request<'_>) {
        request.pair("symbols", self.symbols.join(","));
        request.pair("timeframe", self.timeframe.to_string());
        window(request, self.start, self.end, self.limit);
        page(request, self.page_token.as_ref(), self.sort.as_ref());
    }
}

/// Which trades or quotes to read, for the asset classes whose history takes
/// no other parameter (crypto, options). One call returns one page.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryQuery {
    /// The symbols, sent comma-joined.
    pub symbols: Vec<String>,
    /// The inclusive start.
    pub start: Option<Timestamp>,
    /// The inclusive end.
    pub end: Option<Timestamp>,
    /// The most points in the page, across all symbols (Alpaca: 1 to
    /// 10,000, default 1,000).
    pub limit: Option<u32>,
    /// The page to read, from a previous page's `next_page_token`.
    pub page_token: Option<String>,
    /// Oldest or newest first, by time.
    pub sort: Option<Sort>,
}

impl HistoryQuery {
    /// Points of these symbols, every other parameter Alpaca's default.
    pub fn new(symbols: Vec<String>) -> HistoryQuery {
        HistoryQuery {
            symbols,
            start: None,
            end: None,
            limit: None,
            page_token: None,
            sort: None,
        }
    }

    /// Adds the pairs in the docs' order.
    pub(crate) fn pairs(&self, request: &mut Request<'_>) {
        request.pair("symbols", self.symbols.join(","));
        window(request, self.start, self.end, self.limit);
        page(request, self.page_token.as_ref(), self.sort.as_ref());
    }
}

/// The symbols of a request that takes nothing else.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolsQuery {
    /// The symbols, sent comma-joined.
    pub symbols: Vec<String>,
}

impl SymbolsQuery {
    /// These symbols.
    pub fn new(symbols: Vec<String>) -> SymbolsQuery {
        SymbolsQuery { symbols }
    }

    /// Adds the one pair.
    pub(crate) fn pairs(&self, request: &mut Request<'_>) {
        request.pair("symbols", self.symbols.join(","));
    }
}

fn window(
    request: &mut Request<'_>,
    start: Option<Timestamp>,
    end: Option<Timestamp>,
    limit: Option<u32>,
) {
    if let Some(start) = start {
        request.pair("start", start.to_string());
    }
    if let Some(end) = end {
        request.pair("end", end.to_string());
    }
    if let Some(limit) = limit {
        request.pair("limit", limit.to_string());
    }
}

fn page(request: &mut Request<'_>, page_token: Option<&String>, sort: Option<&Sort>) {
    if let Some(page_token) = page_token {
        request.pair("page_token", page_token.as_str());
    }
    if let Some(sort) = sort {
        request.pair("sort", sort.as_str());
    }
}

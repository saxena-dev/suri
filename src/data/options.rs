//! Options market data: history, the latest trades and quotes, snapshots
//! with greeks when Alpaca can compute them, the option chain, the code
//! tables, and the live options stream.
//!
//! | To… | Call |
//! |---|---|
//! | read history, one page per call | [`option_bars`] and [`option_trades`] |
//! | read each contract's latest trade or quote | [`option_latest_trades`] and [`option_latest_quotes`] |
//! | read contracts' snapshots, one page per call | [`option_snapshots`] |
//! | read an underlying's chain, one page per call | [`option_chain`], filtered by a [`ChainQuery`] |
//! | name the codes in trades and quotes | [`option_conditions`] and [`option_exchanges`] |
//! | stream trades and quotes live | [`option_stream`], with the `stream` feature; its messages are [`Message`] |
//!
//! Symbols are option contract symbols such as `AAPL240315C00172500`.
//! Alpaca keeps historical option data from February 2024. The options
//! stream speaks MessagePack, not JSON; suri asks for it and reads it, and a
//! message it cannot read keeps its MessagePack bytes.
//!
//! [`option_bars`]: crate::Client::option_bars
//! [`option_trades`]: crate::Client::option_trades
//! [`option_latest_trades`]: crate::Client::option_latest_trades
//! [`option_latest_quotes`]: crate::Client::option_latest_quotes
//! [`option_snapshots`]: crate::Client::option_snapshots
//! [`option_chain`]: crate::Client::option_chain
//! [`option_conditions`]: crate::Client::option_conditions
//! [`option_exchanges`]: crate::Client::option_exchanges
//! [`option_stream`]: crate::Client::option_stream

use std::collections::BTreeMap;

use reqwest::Method;
use serde::Deserialize;

#[cfg(feature = "stream")]
use super::sealed::{Codec, Sealed};
#[cfg(feature = "stream")]
use super::stream::{error, head, open, point, subscription};
pub use super::{Bar, BarsQuery, HistoryQuery, TickType, Timeframe};
#[cfg(feature = "stream")]
use super::{Channels, DataMessage, DataStream};
use super::{Latest, Series};
#[cfg(feature = "stream")]
use crate::StreamError;
use crate::rest::client::{Endpoint, Host, Request};
use crate::trading::ContractType;
use crate::value::wire::wire_enum;
use crate::{Client, Date, Decimal, Error, Response, Symbol, Timestamp};

wire_enum! {
    /// An options data feed. Without one, Alpaca picks `opra` if the
    /// caller's subscription allows it, else `indicative`.
    Feed {
        /// The official OPRA feed.
        Opra => "opra",
        /// A free indicative feed: trades are delayed and quotes are
        /// modified.
        Indicative => "indicative",
    }
}

/// Which latest option trades or quotes to read.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct LatestQuery {
    /// The contract symbols, sent comma-joined (Alpaca: at most 100).
    pub symbols: Vec<String>,
    /// The feed.
    pub feed: Option<Feed>,
}

impl LatestQuery {
    /// The latest values of these contracts, every other parameter
    /// Alpaca's default.
    pub fn new(symbols: Vec<String>) -> LatestQuery {
        LatestQuery {
            symbols,
            feed: None,
        }
    }
}

/// Which option snapshots to read. One call returns one page.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotsQuery {
    /// The contract symbols, sent comma-joined (Alpaca: at most 100).
    pub symbols: Vec<String>,
    /// The feed.
    pub feed: Option<Feed>,
    /// Only snapshots whose trade or quote is at or after this time.
    pub updated_since: Option<Timestamp>,
    /// The most snapshots in the page (Alpaca: 1 to 1,000, default 100).
    pub limit: Option<u32>,
    /// The page to read, from a previous page's `next_page_token`.
    pub page_token: Option<String>,
}

impl SnapshotsQuery {
    /// The snapshots of these contracts, every other parameter Alpaca's
    /// default.
    pub fn new(symbols: Vec<String>) -> SnapshotsQuery {
        SnapshotsQuery {
            symbols,
            feed: None,
            updated_since: None,
            limit: None,
            page_token: None,
        }
    }
}

/// Which contracts of an underlying's chain to read. One call returns one
/// page.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChainQuery {
    /// The feed.
    pub feed: Option<Feed>,
    /// The most snapshots in the page (Alpaca: 1 to 1,000, default 100).
    pub limit: Option<u32>,
    /// Only snapshots whose trade or quote is at or after this time.
    pub updated_since: Option<Timestamp>,
    /// The page to read, from a previous page's `next_page_token`.
    pub page_token: Option<String>,
    /// Only calls or only puts.
    pub r#type: Option<ContractType>,
    /// Only strikes at or above this price, sent as its exact text.
    pub strike_price_gte: Option<Decimal>,
    /// Only strikes at or below this price, sent as its exact text.
    pub strike_price_lte: Option<Decimal>,
    /// Only contracts expiring on this date.
    pub expiration_date: Option<Date>,
    /// Only contracts expiring on or after this date.
    pub expiration_date_gte: Option<Date>,
    /// Only contracts expiring on or before this date.
    pub expiration_date_lte: Option<Date>,
    /// Only contracts with this root symbol.
    pub root_symbol: Option<String>,
}

impl ChainQuery {
    /// The whole chain, every parameter Alpaca's default.
    pub fn new() -> ChainQuery {
        ChainQuery::default()
    }
}

/// An option trade.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Trade {
    /// When the trade happened.
    #[serde(rename = "t")]
    pub timestamp: Timestamp,
    /// The option exchange code.
    #[serde(rename = "x")]
    pub exchange: String,
    /// The price.
    #[serde(rename = "p")]
    pub price: f64,
    /// The size.
    #[serde(rename = "s")]
    pub size: u32,
    /// The condition code: one code, not a list.
    #[serde(rename = "c")]
    pub condition: String,
}

/// An option's best bid and ask. Exchange codes are option exchange codes,
/// and case-sensitive.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Quote {
    /// When the quote was made.
    #[serde(rename = "t")]
    pub timestamp: Timestamp,
    /// The bid's exchange code.
    #[serde(rename = "bx")]
    pub bid_exchange: String,
    /// The bid price.
    #[serde(rename = "bp")]
    pub bid_price: f64,
    /// The bid size.
    #[serde(rename = "bs")]
    pub bid_size: u32,
    /// The ask's exchange code.
    #[serde(rename = "ax")]
    pub ask_exchange: String,
    /// The ask price.
    #[serde(rename = "ap")]
    pub ask_price: f64,
    /// The ask size.
    #[serde(rename = "as")]
    pub ask_size: u32,
    /// The condition code: one code, which may be a space.
    #[serde(rename = "c")]
    pub condition: String,
}

/// An option's greeks, from the Black-Scholes model.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Greeks {
    /// Delta.
    pub delta: f64,
    /// Gamma.
    pub gamma: f64,
    /// Theta.
    pub theta: f64,
    /// Vega.
    pub vega: f64,
    /// Rho.
    pub rho: f64,
}

/// An option's latest trade and quote, its bars, greeks and implied
/// volatility.
///
/// Alpaca may leave out any of them. Greeks and implied
/// volatility need a latest quote with a non-zero bid and ask, the
/// underlying's latest trade, an expiry after today and an implied
/// volatility Alpaca's model can compute, so a same-day contract has none.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Snapshot {
    /// The latest trade.
    #[serde(rename = "latestTrade")]
    pub latest_trade: Option<Trade>,
    /// The latest quote.
    #[serde(rename = "latestQuote")]
    pub latest_quote: Option<Quote>,
    /// The latest minute bar.
    #[serde(rename = "minuteBar")]
    pub minute_bar: Option<Bar>,
    /// The current daily bar.
    #[serde(rename = "dailyBar")]
    pub daily_bar: Option<Bar>,
    /// The previous daily bar.
    #[serde(rename = "prevDailyBar")]
    pub previous_daily_bar: Option<Bar>,
    /// The greeks.
    pub greeks: Option<Greeks>,
    /// The implied volatility, from the Black-Scholes model.
    #[serde(rename = "impliedVolatility")]
    pub implied_volatility: Option<f64>,
}

/// One page of option snapshots.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Snapshots {
    /// Each contract's snapshot.
    pub snapshots: BTreeMap<String, Snapshot>,
    /// The token for the next page; `None` on the last page.
    pub next_page_token: Option<String>,
}

const BARS: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v1beta1/options/bars",
};

const TRADES: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v1beta1/options/trades",
};

const LATEST_TRADES: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v1beta1/options/trades/latest",
};

const LATEST_QUOTES: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v1beta1/options/quotes/latest",
};

const SNAPSHOTS: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v1beta1/options/snapshots",
};

const CHAIN: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v1beta1/options/snapshots/{underlying_symbol}",
};

const CONDITIONS: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v1beta1/options/meta/conditions/{ticktype}",
};

const EXCHANGES: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v1beta1/options/meta/exchanges",
};

impl Client {
    /// Reads one page of option bars (`GET /v1beta1/options/bars`), sorted
    /// by symbol, then time.
    pub async fn option_bars(&self, query: &BarsQuery) -> Result<Response<Series<Bar>>, Error> {
        let mut request = Request::new(&BARS, &["v1beta1", "options", "bars"]);
        query.pairs(&mut request);
        self.send_request(request).await
    }

    /// Reads one page of option trades (`GET /v1beta1/options/trades`),
    /// sorted by symbol, then time.
    pub async fn option_trades(
        &self,
        query: &HistoryQuery,
    ) -> Result<Response<Series<Trade>>, Error> {
        let mut request = Request::new(&TRADES, &["v1beta1", "options", "trades"]);
        query.pairs(&mut request);
        self.send_request(request).await
    }

    /// Reads each contract's latest trade (`GET
    /// /v1beta1/options/trades/latest`).
    pub async fn option_latest_trades(
        &self,
        query: &LatestQuery,
    ) -> Result<Response<Latest<Trade>>, Error> {
        let segments = &["v1beta1", "options", "trades", "latest"];
        self.send_request(latest(&LATEST_TRADES, segments, query))
            .await
    }

    /// Reads each contract's latest quote (`GET
    /// /v1beta1/options/quotes/latest`).
    pub async fn option_latest_quotes(
        &self,
        query: &LatestQuery,
    ) -> Result<Response<Latest<Quote>>, Error> {
        let segments = &["v1beta1", "options", "quotes", "latest"];
        self.send_request(latest(&LATEST_QUOTES, segments, query))
            .await
    }

    /// Reads one page of snapshots of a list of contracts (`GET
    /// /v1beta1/options/snapshots`).
    pub async fn option_snapshots(
        &self,
        query: &SnapshotsQuery,
    ) -> Result<Response<Snapshots>, Error> {
        let mut request = Request::new(&SNAPSHOTS, &["v1beta1", "options", "snapshots"]);
        request.pair("symbols", query.symbols.join(","));
        if let Some(feed) = &query.feed {
            request.pair("feed", feed.as_str());
        }
        if let Some(updated_since) = &query.updated_since {
            request.pair("updated_since", updated_since.to_string());
        }
        if let Some(limit) = query.limit {
            request.pair("limit", limit.to_string());
        }
        if let Some(page_token) = &query.page_token {
            request.pair("page_token", page_token.as_str());
        }
        self.send_request(request).await
    }

    /// Reads one page of snapshots of an underlying's contracts (`GET
    /// /v1beta1/options/snapshots/{underlying_symbol}`).
    pub async fn option_chain(
        &self,
        underlying: &Symbol,
        query: &ChainQuery,
    ) -> Result<Response<Snapshots>, Error> {
        let segments = ["v1beta1", "options", "snapshots", underlying.as_str()];
        let mut request = Request::new(&CHAIN, &segments);
        if let Some(feed) = &query.feed {
            request.pair("feed", feed.as_str());
        }
        if let Some(limit) = query.limit {
            request.pair("limit", limit.to_string());
        }
        if let Some(updated_since) = &query.updated_since {
            request.pair("updated_since", updated_since.to_string());
        }
        if let Some(page_token) = &query.page_token {
            request.pair("page_token", page_token.as_str());
        }
        if let Some(contract_type) = &query.r#type {
            request.pair("type", contract_type.as_str());
        }
        if let Some(strike) = &query.strike_price_gte {
            request.pair("strike_price_gte", strike.as_str());
        }
        if let Some(strike) = &query.strike_price_lte {
            request.pair("strike_price_lte", strike.as_str());
        }
        if let Some(date) = &query.expiration_date {
            request.pair("expiration_date", date.to_string());
        }
        if let Some(date) = &query.expiration_date_gte {
            request.pair("expiration_date_gte", date.to_string());
        }
        if let Some(date) = &query.expiration_date_lte {
            request.pair("expiration_date_lte", date.to_string());
        }
        if let Some(root_symbol) = &query.root_symbol {
            request.pair("root_symbol", root_symbol.as_str());
        }
        self.send_request(request).await
    }

    /// Reads the names of the option trade or quote condition codes, code
    /// to name (`GET /v1beta1/options/meta/conditions/{ticktype}`).
    pub async fn option_conditions(
        &self,
        tick_type: TickType,
    ) -> Result<Response<BTreeMap<String, String>>, Error> {
        let segments = [
            "v1beta1",
            "options",
            "meta",
            "conditions",
            tick_type.as_str(),
        ];
        self.send(&CONDITIONS, &segments).await
    }

    /// Reads the names of the option exchange codes, code to name (`GET
    /// /v1beta1/options/meta/exchanges`).
    pub async fn option_exchanges(&self) -> Result<Response<BTreeMap<String, String>>, Error> {
        self.send(&EXCHANGES, &["v1beta1", "options", "meta", "exchanges"])
            .await
    }
}

fn latest<'a>(endpoint: &'a Endpoint, segments: &'a [&'a str], query: &LatestQuery) -> Request<'a> {
    let mut request = Request::new(endpoint, segments);
    request.pair("symbols", query.symbols.join(","));
    if let Some(feed) = &query.feed {
        request.pair("feed", feed.as_str());
    }
    request
}

/// A message of the options stream that
/// [`Client::option_stream`](crate::Client::option_stream) opens.
///
/// A data message carries its contract symbol. The stream is MessagePack, so a
/// message suri does not type keeps its MessagePack bytes.
#[cfg(feature = "stream")]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// A trade (`t`).
    Trade {
        /// The contract symbol.
        symbol: String,
        /// The trade.
        trade: Trade,
    },
    /// A quote (`q`).
    Quote {
        /// The contract symbol.
        symbol: String,
        /// The quote.
        quote: Quote,
    },
    /// The whole current subscription, Alpaca's reply to a subscribe or
    /// unsubscribe.
    Subscription(Channels),
    /// An error Alpaca reported; the stream goes on.
    Error {
        /// Alpaca's code.
        code: u16,
        /// Alpaca's message, scrubbed and bounded.
        message: String,
    },
    /// A message of a known type that suri could not read, its MessagePack
    /// bytes as sent.
    Undecoded(Vec<u8>),
    /// A message of another type, or not a map, its MessagePack bytes as
    /// sent.
    Other(Vec<u8>),
}

#[cfg(feature = "stream")]
impl Sealed for Message {
    const STREAM: &'static str = "options";
    const CODEC: Codec = Codec::MessagePack;

    fn decode(element: &[u8], credentials: &[&str; 2]) -> Message {
        let Some(head) = head(Self::CODEC, element) else {
            return Message::Other(element.to_vec());
        };
        let symbol = head.symbol;
        let message = match head.kind.as_str() {
            "t" => point(Self::CODEC, element, symbol)
                .map(|(symbol, trade)| Message::Trade { symbol, trade }),
            "q" => point(Self::CODEC, element, symbol)
                .map(|(symbol, quote)| Message::Quote { symbol, quote }),
            "subscription" => subscription(Self::CODEC, element).map(Message::Subscription),
            "error" => error(Self::CODEC, element, credentials)
                .map(|(code, message)| Message::Error { code, message }),
            _ => return Message::Other(element.to_vec()),
        };
        message.unwrap_or_else(|| Message::Undecoded(element.to_vec()))
    }
}

#[cfg(feature = "stream")]
impl DataMessage for Message {}

#[cfg(feature = "stream")]
impl Client {
    /// Opens the options stream of a feed: connects to `/v1beta1/{feed}` on
    /// the Market Data stream host, authenticated by the key-pair headers
    /// and asking for MessagePack, the stream's only format, and returns
    /// once Alpaca confirms the connection, subscribed to nothing. The
    /// returned value owns the connection and reads it only while
    /// [`DataStream::next`] or [`DataStream::close`] runs.
    ///
    /// Needs the `stream` feature.
    pub async fn option_stream(&self, feed: Feed) -> Result<DataStream<Message>, StreamError> {
        let path = format!("/v1beta1/{}", feed.as_str());
        open(self, &path, Some(feed.as_str())).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_option_endpoint_is_on_the_data_host() {
        for endpoint in [
            &BARS,
            &TRADES,
            &LATEST_TRADES,
            &LATEST_QUOTES,
            &SNAPSHOTS,
            &CHAIN,
            &CONDITIONS,
            &EXCHANGES,
        ] {
            assert!(matches!(endpoint.host, Host::Data), "{}", endpoint.template);
        }
    }
}

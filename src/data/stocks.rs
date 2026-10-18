//! Stock market data: historical bars, trades, quotes and auctions.
//!
//! The `Client` methods that return these types: `stock_bars`,
//! `stock_trades`, `stock_quotes` and `stock_auctions`.

use reqwest::Method;
use serde::Deserialize;

use super::Series;
pub use super::{Bar, Timeframe};
use crate::rest::client::{Endpoint, Host, Request};
use crate::value::wire::wire_enum;
use crate::{Client, Date, Error, Inbound, Response, Sort, Timestamp};

wire_enum! {
    /// A stock data feed. Which feeds an endpoint accepts, and which the
    /// caller's subscription allows, is Alpaca's to decide.
    Feed {
        /// Every US exchange.
        Sip => "sip",
        /// The Investors Exchange.
        Iex => "iex",
        /// SIP, 15 minutes delayed.
        DelayedSip => "delayed_sip",
        /// Blue Ocean ATS, overnight US trading.
        Boats => "boats",
        /// Alpaca's overnight feed derived from Blue Ocean.
        Overnight => "overnight",
        /// Over-the-counter.
        Otc => "otc",
    }
}

wire_enum! {
    /// A corporate-action adjustment applied to bars.
    Adjustment {
        /// No adjustment.
        Raw => "raw",
        /// Price and volume for forward and reverse splits.
        Split => "split",
        /// Price for cash dividends.
        Dividend => "dividend",
        /// Price for spin-offs.
        SpinOff => "spin-off",
        /// Every adjustment above.
        All => "all",
    }
}

wire_enum! {
    /// The tape a trade or quote was reported on.
    Tape {
        /// New York Stock Exchange.
        A => "A",
        /// NYSE Arca, Bats, IEX and other regional exchanges.
        B => "B",
        /// NASDAQ.
        C => "C",
        /// Overnight.
        N => "N",
        /// OTC.
        O => "O",
    }
}

wire_enum! {
    /// A later update to a trade; a trade without one is valid.
    TradeUpdateKind {
        /// The trade was canceled.
        Canceled => "canceled",
        /// The trade was incorrect.
        Incorrect => "incorrect",
        /// The trade was corrected.
        Corrected => "corrected",
    }
}

/// How a symbol is mapped across renames.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AsOf {
    /// The entity the symbol named on this date, sent as `YYYY-MM-DD`.
    Date(Date),
    /// No mapping, sent as `-`.
    Unmapped,
}

impl AsOf {
    fn text(&self) -> String {
        match self {
            AsOf::Date(date) => date.to_string(),
            AsOf::Unmapped => "-".to_owned(),
        }
    }
}

/// Which stock bars to read. One call returns one page: to read the next,
/// set `page_token` to the previous page's `next_page_token`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct BarsQuery {
    /// The symbols, sent comma-joined.
    pub symbols: Vec<String>,
    /// The bars' interval.
    pub timeframe: Timeframe,
    /// The inclusive start; Alpaca's default is the start of the current
    /// day.
    pub start: Option<Timestamp>,
    /// The inclusive end; Alpaca's default is now, or 15 minutes ago without
    /// real-time access.
    pub end: Option<Timestamp>,
    /// The most bars in the page, across all symbols (Alpaca: 1 to 10,000,
    /// default 1,000).
    pub limit: Option<u32>,
    /// The adjustments to apply, sent comma-joined (Alpaca's default: raw).
    pub adjustment: Option<Vec<Adjustment>>,
    /// How symbols are mapped across renames.
    pub asof: Option<AsOf>,
    /// The feed.
    pub feed: Option<Feed>,
    /// The currency of the prices, ISO 4217 (Alpaca's default: USD).
    pub currency: Option<String>,
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
            adjustment: None,
            asof: None,
            feed: None,
            currency: None,
            page_token: None,
            sort: None,
        }
    }
}

/// Which stock trades, quotes or auctions to read. One call returns one page.
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
    /// How symbols are mapped across renames.
    pub asof: Option<AsOf>,
    /// The feed (auctions accept only `sip`).
    pub feed: Option<Feed>,
    /// The currency of the prices, ISO 4217.
    pub currency: Option<String>,
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
            asof: None,
            feed: None,
            currency: None,
            page_token: None,
            sort: None,
        }
    }
}

/// A stock trade.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Trade {
    /// When the trade happened.
    #[serde(rename = "t")]
    pub timestamp: Timestamp,
    /// The exchange code.
    #[serde(rename = "x")]
    pub exchange: String,
    /// The price.
    #[serde(rename = "p")]
    pub price: f64,
    /// The size.
    #[serde(rename = "s")]
    pub size: u32,
    /// The condition flags.
    #[serde(rename = "c")]
    pub conditions: Vec<String>,
    /// The trade ID the exchange sent.
    #[serde(rename = "i")]
    pub id: u64,
    /// The tape.
    #[serde(rename = "z")]
    pub tape: Inbound<Tape>,
    /// A later update to the trade; `None` means the trade is valid.
    #[serde(rename = "u")]
    pub update: Option<Inbound<TradeUpdateKind>>,
}

/// A stock's best bid and ask.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Quote {
    /// When the quote was made.
    #[serde(rename = "t")]
    pub timestamp: Timestamp,
    /// The bid's exchange code.
    #[serde(rename = "bx")]
    pub bid_exchange: String,
    /// The bid price; 0 means no active bid.
    #[serde(rename = "bp")]
    pub bid_price: f64,
    /// The bid size in shares (round lots before November 3, 2025).
    #[serde(rename = "bs")]
    pub bid_size: u32,
    /// The ask's exchange code.
    #[serde(rename = "ax")]
    pub ask_exchange: String,
    /// The ask price; 0 means no active ask.
    #[serde(rename = "ap")]
    pub ask_price: f64,
    /// The ask size in shares (round lots before November 3, 2025).
    #[serde(rename = "as")]
    pub ask_size: u32,
    /// The condition flags: one flag applies to both bid and ask; two are
    /// the bid's, then the ask's.
    #[serde(rename = "c")]
    pub conditions: Vec<String>,
    /// The tape.
    #[serde(rename = "z")]
    pub tape: Inbound<Tape>,
}

/// One day's opening and closing auctions of a stock.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DailyAuctions {
    /// The day.
    #[serde(rename = "d")]
    pub date: Date,
    /// The opening auctions.
    #[serde(rename = "o")]
    pub opening: Vec<Auction>,
    /// The closing auctions; each price, exchange and condition is shown
    /// once, with its earliest time.
    #[serde(rename = "c")]
    pub closing: Vec<Auction>,
}

/// One auction price.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Auction {
    /// When the auction happened.
    #[serde(rename = "t")]
    pub timestamp: Timestamp,
    /// The exchange code.
    #[serde(rename = "x")]
    pub exchange: String,
    /// The price.
    #[serde(rename = "p")]
    pub price: f64,
    /// The size, when Alpaca sends it.
    #[serde(rename = "s")]
    pub size: Option<i64>,
    /// The condition flag that marks the auction.
    #[serde(rename = "c")]
    pub condition: String,
}

const BARS: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v2/stocks/bars",
};

const TRADES: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v2/stocks/trades",
};

const QUOTES: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v2/stocks/quotes",
};

const AUCTIONS: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v2/stocks/auctions",
};

/// Adds the pairs a bars or history query share after their first ones,
/// in the docs' order.
fn tail(
    request: &mut Request<'_>,
    asof: Option<&AsOf>,
    feed: Option<&Feed>,
    currency: Option<&String>,
    page_token: Option<&String>,
    sort: Option<&Sort>,
) {
    if let Some(asof) = asof {
        request.pair("asof", asof.text());
    }
    if let Some(feed) = feed {
        request.pair("feed", feed.as_str());
    }
    if let Some(currency) = currency {
        request.pair("currency", currency.as_str());
    }
    if let Some(page_token) = page_token {
        request.pair("page_token", page_token.as_str());
    }
    if let Some(sort) = sort {
        request.pair("sort", sort.as_str());
    }
}

impl Client {
    /// Reads one page of stock bars (`GET /v2/stocks/bars`), sorted by
    /// symbol, then time.
    pub async fn stock_bars(&self, query: &BarsQuery) -> Result<Response<Series<Bar>>, Error> {
        let mut request = Request::new(&BARS, &["v2", "stocks", "bars"]);
        request.pair("symbols", query.symbols.join(","));
        request.pair("timeframe", query.timeframe.to_string());
        if let Some(start) = &query.start {
            request.pair("start", start.to_string());
        }
        if let Some(end) = &query.end {
            request.pair("end", end.to_string());
        }
        if let Some(limit) = query.limit {
            request.pair("limit", limit.to_string());
        }
        if let Some(adjustment) = &query.adjustment {
            let texts: Vec<&str> = adjustment.iter().map(Adjustment::as_str).collect();
            request.pair("adjustment", texts.join(","));
        }
        tail(
            &mut request,
            query.asof.as_ref(),
            query.feed.as_ref(),
            query.currency.as_ref(),
            query.page_token.as_ref(),
            query.sort.as_ref(),
        );
        self.send_request(request).await
    }

    /// Reads one page of stock trades (`GET /v2/stocks/trades`), sorted by
    /// symbol, then time.
    pub async fn stock_trades(
        &self,
        query: &HistoryQuery,
    ) -> Result<Response<Series<Trade>>, Error> {
        self.send_request(history(&TRADES, &["v2", "stocks", "trades"], query))
            .await
    }

    /// Reads one page of stock quotes (`GET /v2/stocks/quotes`), sorted by
    /// symbol, then time.
    pub async fn stock_quotes(
        &self,
        query: &HistoryQuery,
    ) -> Result<Response<Series<Quote>>, Error> {
        self.send_request(history(&QUOTES, &["v2", "stocks", "quotes"], query))
            .await
    }

    /// Reads one page of stock auctions (`GET /v2/stocks/auctions`), one
    /// entry per symbol and day.
    pub async fn stock_auctions(
        &self,
        query: &HistoryQuery,
    ) -> Result<Response<Series<DailyAuctions>>, Error> {
        self.send_request(history(&AUCTIONS, &["v2", "stocks", "auctions"], query))
            .await
    }
}

/// A trades, quotes or auctions request, its pairs in the docs' order.
fn history<'a>(
    endpoint: &'a Endpoint,
    segments: &'a [&'a str],
    query: &HistoryQuery,
) -> Request<'a> {
    let mut request = Request::new(endpoint, segments);
    request.pair("symbols", query.symbols.join(","));
    if let Some(start) = &query.start {
        request.pair("start", start.to_string());
    }
    if let Some(end) = &query.end {
        request.pair("end", end.to_string());
    }
    if let Some(limit) = query.limit {
        request.pair("limit", limit.to_string());
    }
    tail(
        &mut request,
        query.asof.as_ref(),
        query.feed.as_ref(),
        query.currency.as_ref(),
        query.page_token.as_ref(),
        query.sort.as_ref(),
    );
    request
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_stock_endpoint_is_on_the_data_host() {
        for endpoint in [&BARS, &TRADES, &QUOTES, &AUCTIONS] {
            assert!(matches!(endpoint.host, Host::Data), "{}", endpoint.template);
        }
    }
}

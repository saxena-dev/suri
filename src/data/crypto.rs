//! Crypto market data: history, the latest values, orderbooks and
//! snapshots, and the live crypto stream, each at a [`Location`].
//!
//! | To… | Call |
//! |---|---|
//! | read history, one page per call | [`crypto_bars`], [`crypto_trades`] and [`crypto_quotes`] |
//! | read each pair's latest value | [`crypto_latest_bars`], [`crypto_latest_trades`] and [`crypto_latest_quotes`] |
//! | read each pair's orderbook | [`crypto_latest_orderbooks`] |
//! | read a pair's whole picture at once | [`crypto_snapshots`] |
//! | stream trades, quotes, bars and orderbooks live | [`crypto_stream`], with the `stream` feature; its messages are [`Message`] |
//!
//! Every call takes a [`Location`] first: `Us` for Alpaca US, `Us1` for
//! Kraken US, `Eu1` for Kraken EU. Symbols are pairs such as `BTC/USD`;
//! they are sent percent-encoded and come back as written. Unlike stock
//! bars, a crypto [`Bar`]'s volume can be fractional, so it is an `f64`.
//!
//! [`crypto_bars`]: crate::Client::crypto_bars
//! [`crypto_trades`]: crate::Client::crypto_trades
//! [`crypto_quotes`]: crate::Client::crypto_quotes
//! [`crypto_latest_bars`]: crate::Client::crypto_latest_bars
//! [`crypto_latest_trades`]: crate::Client::crypto_latest_trades
//! [`crypto_latest_quotes`]: crate::Client::crypto_latest_quotes
//! [`crypto_latest_orderbooks`]: crate::Client::crypto_latest_orderbooks
//! [`crypto_snapshots`]: crate::Client::crypto_snapshots
//! [`crypto_stream`]: crate::Client::crypto_stream

use reqwest::Method;
use serde::Deserialize;

#[cfg(feature = "stream")]
use super::sealed::{Codec, Sealed};
#[cfg(feature = "stream")]
use super::stream::{error, head, open, point, subscription, text};
pub use super::{BarsQuery, HistoryQuery, SymbolsQuery, Timeframe};
#[cfg(feature = "stream")]
use super::{Channels, DataMessage, DataStream};
use super::{Latest, Series};
#[cfg(feature = "stream")]
use crate::StreamError;
use crate::rest::client::{Endpoint, Host, Request};
use crate::value::wire::wire_enum;
use crate::{Client, Error, Inbound, Response, Timestamp};

wire_enum! {
    /// Where crypto market data comes from. Alpaca describes `us`, `us-1`
    /// and `eu-1`; the other two are in its schema only.
    Location {
        /// Alpaca US.
        Us => "us",
        /// Kraken US.
        Us1 => "us-1",
        /// Listed in Alpaca's schema, not described.
        Us2 => "us-2",
        /// Kraken EU.
        Eu1 => "eu-1",
        /// Listed in Alpaca's schema, not described.
        Bs1 => "bs-1",
    }
}

wire_enum! {
    /// Which side took liquidity in a trade.
    TakerSide {
        /// The buyer.
        Buyer => "B",
        /// The seller.
        Seller => "S",
    }
}

/// An OHLC aggregate of a crypto pair's trades in one interval. A bar with
/// no trade has volume 0 and prices from quotes.
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
    /// The volume, which may be fractional.
    #[serde(rename = "v")]
    pub volume: f64,
    /// The number of trades.
    #[serde(rename = "n")]
    pub trade_count: i64,
    /// The volume-weighted average price.
    #[serde(rename = "vw")]
    pub vwap: f64,
}

/// A crypto trade.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Trade {
    /// When the trade happened.
    #[serde(rename = "t")]
    pub timestamp: Timestamp,
    /// The price.
    #[serde(rename = "p")]
    pub price: f64,
    /// The size, which may be fractional.
    #[serde(rename = "s")]
    pub size: f64,
    /// The trade ID.
    #[serde(rename = "i")]
    pub id: i64,
    /// Which side took liquidity.
    #[serde(rename = "tks")]
    pub taker_side: Inbound<TakerSide>,
}

/// A crypto pair's best bid and ask.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Quote {
    /// When the quote was made.
    #[serde(rename = "t")]
    pub timestamp: Timestamp,
    /// The bid price.
    #[serde(rename = "bp")]
    pub bid_price: f64,
    /// The bid size.
    #[serde(rename = "bs")]
    pub bid_size: f64,
    /// The ask price.
    #[serde(rename = "ap")]
    pub ask_price: f64,
    /// The ask size.
    #[serde(rename = "as")]
    pub ask_size: f64,
}

/// A crypto pair's latest trade, quote, minute bar, daily bar and previous
/// daily bar; Alpaca may leave out any of them.
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
}

/// An orderbook's bids and asks. Alpaca states no depth and no order of
/// levels. A level of size 0 appears in Alpaca's REST example unexplained;
/// on the stream it means the level was removed.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Orderbook {
    /// When the book was taken.
    #[serde(rename = "t")]
    pub timestamp: Timestamp,
    /// The bids.
    #[serde(rename = "b")]
    pub bids: Vec<OrderbookLevel>,
    /// The asks.
    #[serde(rename = "a")]
    pub asks: Vec<OrderbookLevel>,
    /// On the stream, whether this is the whole book rather than an update;
    /// `false` when absent, as it always is over REST.
    #[serde(rename = "r", default)]
    pub reset: bool,
}

/// One price level of an orderbook.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OrderbookLevel {
    /// The price.
    #[serde(rename = "p")]
    pub price: f64,
    /// The size.
    #[serde(rename = "s")]
    pub size: f64,
}

const BARS: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v1beta3/crypto/{loc}/bars",
};

const TRADES: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v1beta3/crypto/{loc}/trades",
};

const QUOTES: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v1beta3/crypto/{loc}/quotes",
};

const LATEST_BARS: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v1beta3/crypto/{loc}/latest/bars",
};

const LATEST_TRADES: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v1beta3/crypto/{loc}/latest/trades",
};

const LATEST_QUOTES: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v1beta3/crypto/{loc}/latest/quotes",
};

const LATEST_ORDERBOOKS: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v1beta3/crypto/{loc}/latest/orderbooks",
};

const SNAPSHOTS: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v1beta3/crypto/{loc}/snapshots",
};

impl Client {
    /// Reads one page of crypto bars (`GET /v1beta3/crypto/{loc}/bars`).
    pub async fn crypto_bars(
        &self,
        location: Location,
        query: &BarsQuery,
    ) -> Result<Response<Series<Bar>>, Error> {
        let segments = ["v1beta3", "crypto", location.as_str(), "bars"];
        let mut request = Request::new(&BARS, &segments);
        query.pairs(&mut request);
        self.send_request(request).await
    }

    /// Reads one page of crypto trades (`GET
    /// /v1beta3/crypto/{loc}/trades`).
    pub async fn crypto_trades(
        &self,
        location: Location,
        query: &HistoryQuery,
    ) -> Result<Response<Series<Trade>>, Error> {
        let segments = ["v1beta3", "crypto", location.as_str(), "trades"];
        let mut request = Request::new(&TRADES, &segments);
        query.pairs(&mut request);
        self.send_request(request).await
    }

    /// Reads one page of crypto quotes (`GET
    /// /v1beta3/crypto/{loc}/quotes`). The oldest `us-1` quotes Alpaca keeps
    /// are from 14 October 2025, 00:00 UTC.
    pub async fn crypto_quotes(
        &self,
        location: Location,
        query: &HistoryQuery,
    ) -> Result<Response<Series<Quote>>, Error> {
        let segments = ["v1beta3", "crypto", location.as_str(), "quotes"];
        let mut request = Request::new(&QUOTES, &segments);
        query.pairs(&mut request);
        self.send_request(request).await
    }

    /// Reads each pair's latest minute bar (`GET
    /// /v1beta3/crypto/{loc}/latest/bars`).
    pub async fn crypto_latest_bars(
        &self,
        location: Location,
        query: &SymbolsQuery,
    ) -> Result<Response<Latest<Bar>>, Error> {
        let segments = ["v1beta3", "crypto", location.as_str(), "latest", "bars"];
        self.send_request(latest(&LATEST_BARS, &segments, query))
            .await
    }

    /// Reads each pair's latest trade (`GET
    /// /v1beta3/crypto/{loc}/latest/trades`).
    pub async fn crypto_latest_trades(
        &self,
        location: Location,
        query: &SymbolsQuery,
    ) -> Result<Response<Latest<Trade>>, Error> {
        let segments = ["v1beta3", "crypto", location.as_str(), "latest", "trades"];
        self.send_request(latest(&LATEST_TRADES, &segments, query))
            .await
    }

    /// Reads each pair's latest quote (`GET
    /// /v1beta3/crypto/{loc}/latest/quotes`).
    pub async fn crypto_latest_quotes(
        &self,
        location: Location,
        query: &SymbolsQuery,
    ) -> Result<Response<Latest<Quote>>, Error> {
        let segments = ["v1beta3", "crypto", location.as_str(), "latest", "quotes"];
        self.send_request(latest(&LATEST_QUOTES, &segments, query))
            .await
    }

    /// Reads each pair's latest orderbook (`GET
    /// /v1beta3/crypto/{loc}/latest/orderbooks`).
    pub async fn crypto_latest_orderbooks(
        &self,
        location: Location,
        query: &SymbolsQuery,
    ) -> Result<Response<Latest<Orderbook>>, Error> {
        let segments = [
            "v1beta3",
            "crypto",
            location.as_str(),
            "latest",
            "orderbooks",
        ];
        self.send_request(latest(&LATEST_ORDERBOOKS, &segments, query))
            .await
    }

    /// Reads each pair's snapshot (`GET /v1beta3/crypto/{loc}/snapshots`).
    pub async fn crypto_snapshots(
        &self,
        location: Location,
        query: &SymbolsQuery,
    ) -> Result<Response<Latest<Snapshot>>, Error> {
        let segments = ["v1beta3", "crypto", location.as_str(), "snapshots"];
        self.send_request(latest(&SNAPSHOTS, &segments, query))
            .await
    }
}

fn latest<'a>(
    endpoint: &'a Endpoint,
    segments: &'a [&'a str],
    query: &SymbolsQuery,
) -> Request<'a> {
    let mut request = Request::new(endpoint, segments);
    query.pairs(&mut request);
    request
}

/// A message of the crypto stream that
/// [`Client::crypto_stream`](crate::Client::crypto_stream) opens. A data
/// message carries its pair.
#[cfg(feature = "stream")]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// A trade (`t`).
    Trade {
        /// The pair.
        symbol: String,
        /// The trade.
        trade: Trade,
    },
    /// A quote (`q`).
    Quote {
        /// The pair.
        symbol: String,
        /// The quote.
        quote: Quote,
    },
    /// A minute bar (`b`).
    Bar {
        /// The pair.
        symbol: String,
        /// The bar.
        bar: Bar,
    },
    /// A daily bar (`d`).
    DailyBar {
        /// The pair.
        symbol: String,
        /// The bar.
        bar: Bar,
    },
    /// An updated minute bar (`u`), revisiting an earlier minute.
    UpdatedBar {
        /// The pair.
        symbol: String,
        /// The bar.
        bar: Bar,
    },
    /// An orderbook (`o`): the whole book when `reset` is set, else an
    /// update in which a size of 0 removes the level. Delivered as sent;
    /// suri applies nothing.
    Orderbook {
        /// The pair.
        symbol: String,
        /// The book or its update.
        orderbook: Orderbook,
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
    /// A message of a known type that suri could not read, as sent.
    Undecoded(String),
    /// A message of another type, or not an object, as sent.
    Other(String),
}

#[cfg(feature = "stream")]
impl Sealed for Message {
    const STREAM: &'static str = "crypto";
    const CODEC: Codec = Codec::Json;

    fn decode(element: &[u8], credentials: &[&str; 2]) -> Message {
        let Some(head) = head(Self::CODEC, element) else {
            return Message::Other(text(element));
        };
        let symbol = head.symbol;
        let message = match head.kind.as_str() {
            "t" => point(Self::CODEC, element, symbol)
                .map(|(symbol, trade)| Message::Trade { symbol, trade }),
            "q" => point(Self::CODEC, element, symbol)
                .map(|(symbol, quote)| Message::Quote { symbol, quote }),
            "b" => point(Self::CODEC, element, symbol)
                .map(|(symbol, bar)| Message::Bar { symbol, bar }),
            "d" => point(Self::CODEC, element, symbol)
                .map(|(symbol, bar)| Message::DailyBar { symbol, bar }),
            "u" => point(Self::CODEC, element, symbol)
                .map(|(symbol, bar)| Message::UpdatedBar { symbol, bar }),
            "o" => point(Self::CODEC, element, symbol)
                .map(|(symbol, orderbook)| Message::Orderbook { symbol, orderbook }),
            "subscription" => subscription(Self::CODEC, element).map(Message::Subscription),
            "error" => error(Self::CODEC, element, credentials)
                .map(|(code, message)| Message::Error { code, message }),
            _ => return Message::Other(text(element)),
        };
        message.unwrap_or_else(|| Message::Undecoded(text(element)))
    }
}

#[cfg(feature = "stream")]
impl DataMessage for Message {}

#[cfg(feature = "stream")]
impl Client {
    /// Opens the crypto stream of a location: connects to
    /// `/v1beta3/crypto/{loc}` on the Market Data stream host, authenticated
    /// by the key-pair headers, and returns once Alpaca confirms the
    /// connection, subscribed to nothing. Alpaca's stream describes `us`,
    /// `us-1` and `eu-1`. The returned value owns the connection and reads
    /// it only while [`DataStream::next`] or [`DataStream::close`] runs.
    ///
    /// Needs the `stream` feature.
    pub async fn crypto_stream(
        &self,
        location: Location,
    ) -> Result<DataStream<Message>, StreamError> {
        let path = format!("/v1beta3/crypto/{}", location.as_str());
        open(self, &path, Some(location.as_str())).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_crypto_endpoint_is_on_the_data_host() {
        for endpoint in [
            &BARS,
            &TRADES,
            &QUOTES,
            &LATEST_BARS,
            &LATEST_TRADES,
            &LATEST_QUOTES,
            &LATEST_ORDERBOOKS,
            &SNAPSHOTS,
        ] {
            assert!(matches!(endpoint.host, Host::Data), "{}", endpoint.template);
        }
    }
}

//! The stock stream: its feeds, its messages and the models only the stream
//! sends.

use serde::Deserialize;

use super::{Bar, Quote, Tape, Trade};
use crate::data::sealed::Sealed;
use crate::data::stream::{error, head, open, point, subscription};
use crate::data::{Channels, DataMessage, DataStream};
use crate::value::wire::wire_enum;
use crate::{Client, Inbound, StreamError, Timestamp};

/// Which stock stream to open; each is its own path on the stream host.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamFeed {
    /// The SIP feed, every US exchange (`v2/sip`).
    Sip,
    /// The IEX feed (`v2/iex`).
    Iex,
    /// The SIP feed, 15 minutes delayed (`v2/delayed_sip`).
    DelayedSip,
    /// The BOATS feed, Blue Ocean ATS (`v1beta1/boats`).
    Boats,
    /// Alpaca's derived overnight feed (`v1beta1/overnight`).
    Overnight,
    /// The test stream, open at all hours, with the symbol `FAKEPACA`
    /// (`v2/test`).
    Test,
}

impl StreamFeed {
    /// The stream's path and the feed's name.
    fn path(self) -> (&'static str, &'static str) {
        match self {
            StreamFeed::Sip => ("/v2/sip", "sip"),
            StreamFeed::Iex => ("/v2/iex", "iex"),
            StreamFeed::DelayedSip => ("/v2/delayed_sip", "delayed_sip"),
            StreamFeed::Boats => ("/v1beta1/boats", "boats"),
            StreamFeed::Overnight => ("/v1beta1/overnight", "overnight"),
            StreamFeed::Test => ("/v2/test", "test"),
        }
    }
}

wire_enum! {
    /// What a trade cancel did.
    CancelAction {
        /// The trade was cancelled.
        Cancel => "C",
        /// The trade was an error.
        Error => "E",
    }
}

/// A correction of an earlier trade, from the stock stream.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Correction {
    /// The exchange code.
    #[serde(rename = "x")]
    pub exchange: String,
    /// The original trade's ID.
    #[serde(rename = "oi")]
    pub original_id: u64,
    /// The original trade's price.
    #[serde(rename = "op")]
    pub original_price: f64,
    /// The original trade's size.
    #[serde(rename = "os")]
    pub original_size: u32,
    /// The original trade's condition codes.
    #[serde(rename = "oc")]
    pub original_conditions: Vec<String>,
    /// The corrected trade's ID.
    #[serde(rename = "ci")]
    pub corrected_id: u64,
    /// The corrected trade's price.
    #[serde(rename = "cp")]
    pub corrected_price: f64,
    /// The corrected trade's size.
    #[serde(rename = "cs")]
    pub corrected_size: u32,
    /// The corrected trade's condition codes.
    #[serde(rename = "cc")]
    pub corrected_conditions: Vec<String>,
    /// When the correction was made.
    #[serde(rename = "t")]
    pub timestamp: Timestamp,
    /// The tape.
    #[serde(rename = "z")]
    pub tape: Inbound<Tape>,
}

/// A trade cancel or error, from the stock stream.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Cancel {
    /// The trade's ID.
    #[serde(rename = "i")]
    pub id: u64,
    /// The exchange code.
    #[serde(rename = "x")]
    pub exchange: String,
    /// The trade's price.
    #[serde(rename = "p")]
    pub price: f64,
    /// The trade's size.
    #[serde(rename = "s")]
    pub size: u32,
    /// Whether it was a cancel or an error.
    #[serde(rename = "a")]
    pub action: Inbound<CancelAction>,
    /// When it was made.
    #[serde(rename = "t")]
    pub timestamp: Timestamp,
    /// The tape.
    #[serde(rename = "z")]
    pub tape: Inbound<Tape>,
}

/// A limit up-limit down price band, from the stock stream.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Luld {
    /// The upper limit price band.
    #[serde(rename = "u")]
    pub limit_up: f64,
    /// The lower limit price band.
    #[serde(rename = "d")]
    pub limit_down: f64,
    /// The indicator.
    #[serde(rename = "i")]
    pub indicator: String,
    /// When the band was set.
    #[serde(rename = "t")]
    pub timestamp: Timestamp,
    /// The tape.
    #[serde(rename = "z")]
    pub tape: Inbound<Tape>,
}

/// A trading status, such as a halt or a resumption, from the stock stream.
/// The codes are the tape's, kept as text.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Status {
    /// The status code.
    #[serde(rename = "sc")]
    pub status_code: String,
    /// The status message.
    #[serde(rename = "sm")]
    pub status_message: String,
    /// The reason code.
    #[serde(rename = "rc")]
    pub reason_code: String,
    /// The reason message.
    #[serde(rename = "rm")]
    pub reason_message: String,
    /// When the status was set.
    #[serde(rename = "t")]
    pub timestamp: Timestamp,
    /// The tape.
    #[serde(rename = "z")]
    pub tape: Inbound<Tape>,
}

/// An order imbalance, from the stock stream.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Imbalance {
    /// The price.
    #[serde(rename = "p")]
    pub price: f64,
    /// When it was reported.
    #[serde(rename = "t")]
    pub timestamp: Timestamp,
    /// The tape.
    #[serde(rename = "z")]
    pub tape: Inbound<Tape>,
}

/// A message of the stock stream that
/// [`Client::stock_stream`](crate::Client::stock_stream) opens. A data
/// message carries its symbol.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// A trade (`t`).
    Trade {
        /// The symbol.
        symbol: String,
        /// The trade.
        trade: Trade,
    },
    /// A quote (`q`).
    Quote {
        /// The symbol.
        symbol: String,
        /// The quote.
        quote: Quote,
    },
    /// A minute bar (`b`).
    Bar {
        /// The symbol.
        symbol: String,
        /// The bar.
        bar: Bar,
    },
    /// A daily bar (`d`).
    DailyBar {
        /// The symbol.
        symbol: String,
        /// The bar.
        bar: Bar,
    },
    /// An updated minute bar (`u`), revisiting an earlier minute.
    UpdatedBar {
        /// The symbol.
        symbol: String,
        /// The bar.
        bar: Bar,
    },
    /// A trade correction (`c`).
    Correction {
        /// The symbol.
        symbol: String,
        /// The correction.
        correction: Correction,
    },
    /// A trade cancel or error (`x`).
    Cancel {
        /// The symbol.
        symbol: String,
        /// The cancel.
        cancel: Cancel,
    },
    /// A limit up-limit down band (`l`).
    Luld {
        /// The symbol.
        symbol: String,
        /// The band.
        luld: Luld,
    },
    /// A trading status (`s`).
    Status {
        /// The symbol.
        symbol: String,
        /// The status.
        status: Status,
    },
    /// An order imbalance (`i`).
    Imbalance {
        /// The symbol.
        symbol: String,
        /// The imbalance.
        imbalance: Imbalance,
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

impl Sealed for Message {
    const STREAM: &'static str = "stocks";

    fn decode(element: &str, credentials: &[&str; 2]) -> Message {
        let Some(head) = head(element) else {
            return Message::Other(element.to_owned());
        };
        let symbol = head.symbol;
        let message = match head.kind.as_str() {
            "t" => point(element, symbol).map(|(symbol, trade)| Message::Trade { symbol, trade }),
            "q" => point(element, symbol).map(|(symbol, quote)| Message::Quote { symbol, quote }),
            "b" => point(element, symbol).map(|(symbol, bar)| Message::Bar { symbol, bar }),
            "d" => point(element, symbol).map(|(symbol, bar)| Message::DailyBar { symbol, bar }),
            "u" => point(element, symbol).map(|(symbol, bar)| Message::UpdatedBar { symbol, bar }),
            "c" => point(element, symbol)
                .map(|(symbol, correction)| Message::Correction { symbol, correction }),
            "x" => {
                point(element, symbol).map(|(symbol, cancel)| Message::Cancel { symbol, cancel })
            }
            "l" => point(element, symbol).map(|(symbol, luld)| Message::Luld { symbol, luld }),
            "s" => {
                point(element, symbol).map(|(symbol, status)| Message::Status { symbol, status })
            }
            "i" => point(element, symbol)
                .map(|(symbol, imbalance)| Message::Imbalance { symbol, imbalance }),
            "subscription" => subscription(element).map(Message::Subscription),
            "error" => {
                error(element, credentials).map(|(code, message)| Message::Error { code, message })
            }
            _ => return Message::Other(element.to_owned()),
        };
        message.unwrap_or_else(|| Message::Undecoded(element.to_owned()))
    }
}

impl DataMessage for Message {}

impl Client {
    /// Opens a stock stream: connects to the feed's path on the Market Data
    /// stream host, authenticated by the key-pair headers, and returns once
    /// Alpaca confirms the connection, subscribed to nothing. The returned
    /// value owns the connection and reads it only while
    /// [`DataStream::next`] or [`DataStream::close`] runs.
    pub async fn stock_stream(&self, feed: StreamFeed) -> Result<DataStream<Message>, StreamError> {
        let (path, name) = feed.path();
        open(self, path, Some(name)).await
    }
}

//! Helpers shared by the stock data tests: fixtures by name, symbol lists,
//! times, the bar check and the sample doubles.

#![allow(dead_code, reason = "each test binary uses part of the helper")]

use suri::Timestamp;
use suri::data::stocks::Bar;

use crate::fixture;
use crate::requests::serve;
use crate::server::Server;

const ROUTES: &str = "data/test_historical_stock_data";

/// Sample doubles: the text and the double it must read as.
pub const SAMPLES: [&str; 4] = [
    "8425.21",
    "0.30000000000000004",
    "1.1368683772161603e-13",
    "9007199254740993",
];

pub fn at(text: &str) -> Timestamp {
    text.parse().unwrap()
}

/// A server answering with the named upstream stock fixture.
pub fn serve_fixture(name: &str, route: &str) -> Server {
    let fixture = fixture::upstream(&format!("{ROUTES}::{name}::GET {route}#0"));
    assert_eq!(fixture.status, 200);
    serve("200 OK", &fixture.body)
}

pub fn symbols(list: &[&str]) -> Vec<String> {
    list.iter().map(|symbol| (*symbol).to_owned()).collect()
}

#[allow(clippy::too_many_arguments)]
pub fn assert_bar(
    bar: &Bar,
    t: &str,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: i64,
    trade_count: i64,
    vwap: f64,
) {
    assert_eq!(bar.timestamp, at(t));
    assert_eq!(bar.open, open);
    assert_eq!(bar.high, high);
    assert_eq!(bar.low, low);
    assert_eq!(bar.close, close);
    assert_eq!(bar.volume, volume);
    assert_eq!(bar.trade_count, trade_count);
    assert_eq!(bar.vwap, vwap);
}

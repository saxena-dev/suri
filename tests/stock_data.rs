//! Historical stock data: the upstream fixtures field by field, the
//! documented auctions, numbers and times as sent, every query parameter on
//! the wire, and reads retried.

#![cfg(feature = "rest")]

#[path = "support/fixture.rs"]
mod fixture;
#[path = "support/requests.rs"]
mod requests;
#[path = "support/server.rs"]
mod server;
#[path = "support/stock.rs"]
mod stock;

use requests::{client, failing_once, only_request, owned, pairs, path, retrying_client, serve};
use stock::{SAMPLES, assert_bar, at, serve_fixture, symbols};
use suri::data::stocks::{
    Adjustment, AsOf, Auction, BarsQuery, Feed, HistoryQuery, Quote, Tape, Timeframe, Trade,
    TradeUpdateKind,
};
use suri::{Date, ErrorKind, Inbound, Sort};

fn bars_query() -> BarsQuery {
    BarsQuery::new(symbols(&["AAPL"]), Timeframe::Day)
}

fn history_query() -> HistoryQuery {
    HistoryQuery::new(symbols(&["AAPL"]))
}

fn assert_trade(trade: &Trade, t: &str, price: f64, size: u32, id: u64) {
    assert_eq!(trade.timestamp, at(t));
    assert_eq!(trade.exchange, "D");
    assert_eq!(trade.price, price);
    assert_eq!(trade.size, size);
    assert_eq!(trade.conditions, ["@", "T", "I"]);
    assert_eq!(trade.id, id);
    assert_eq!(trade.tape, Inbound::Known(Tape::C));
    assert_eq!(trade.update, None);
}

#[allow(clippy::too_many_arguments)]
fn assert_quote(
    quote: &Quote,
    t: &str,
    ask_price: f64,
    bid_exchange: &str,
    bid_price: f64,
    bid_size: u32,
) {
    assert_eq!(quote.timestamp, at(t));
    assert_eq!(quote.ask_exchange, "K");
    assert_eq!(quote.ask_price, ask_price);
    assert_eq!(quote.ask_size, 1);
    assert_eq!(quote.bid_exchange, bid_exchange);
    assert_eq!(quote.bid_price, bid_price);
    assert_eq!(quote.bid_size, bid_size);
    assert_eq!(quote.conditions, ["R"]);
    assert_eq!(quote.tape, Inbound::Known(Tape::C));
}

#[tokio::test]
async fn the_bars_fixtures_are_read_field_by_field() {
    let server = serve_fixture("test_get_bars", "/v2/stocks/bars");
    let page = client(&server)
        .stock_bars(&bars_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(page.items.len(), 1);
    let bars = &page.items["AAPL"];
    assert_eq!(bars.len(), 2);
    // An integral price arrives without a fraction.
    assert_bar(
        &bars[0],
        "2022-02-01T05:00:00Z",
        174.0,
        174.84,
        172.31,
        174.61,
        85998033,
        732412,
        173.703516,
    );
    assert_bar(
        &bars[1],
        "2022-02-02T05:00:00Z",
        174.64,
        175.88,
        173.33,
        175.84,
        84817432,
        675034,
        174.941288,
    );
    assert_eq!(page.currency, None);
    assert_eq!(
        page.next_page_token.as_deref(),
        Some("QUFQTHxEfDIwMjItMDItMDJUMDU6MDA6MDAuMDAwMDAwMDAwWg==")
    );
    assert_eq!(path(&only_request(&server)), "/v2/stocks/bars");

    // Descending time is kept as sent.
    let server = serve_fixture("test_get_bars_desc", "/v2/stocks/bars");
    let page = client(&server)
        .stock_bars(&bars_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    let bars = &page.items["TSLA"];
    assert_eq!(bars.len(), 3);
    assert_bar(
        &bars[0],
        "2023-09-27T04:00:00Z",
        244.262,
        245.33,
        234.58,
        240.5,
        136616287,
        1393690,
        240.479286,
    );
    assert_bar(
        &bars[1],
        "2023-09-26T04:00:00Z",
        242.98,
        249.55,
        241.6601,
        244.12,
        102033779,
        1118004,
        245.559325,
    );
    assert_bar(
        &bars[2],
        "2023-09-25T04:00:00Z",
        243.38,
        247.1,
        238.31,
        246.99,
        104668716,
        1196613,
        244.197325,
    );
    assert_eq!(
        page.next_page_token.as_deref(),
        Some("VFNMQXxEfDc1Mjc3NTc2MzYwMDAwMDAwMDA=")
    );

    let server = serve_fixture("test_multisymbol_get_bars", "/v2/stocks/bars");
    let page = client(&server)
        .stock_bars(&bars_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(page.items.len(), 2);
    assert_bar(
        &page.items["AAPL"][0],
        "2022-03-09T05:00:00Z",
        161.51,
        163.41,
        159.41,
        162.95,
        88496480,
        700291,
        161.942117,
    );
    assert_bar(
        &page.items["TSLA"][0],
        "2022-03-09T05:00:00Z",
        839.0,
        860.56,
        832.01,
        858.97,
        19227323,
        528531,
        850.616587,
    );
    assert_eq!(page.next_page_token, None);

    let server = serve_fixture("test_barset_contains", "/v2/stocks/bars");
    let page = client(&server)
        .stock_bars(&bars_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(page.items["AAPL"].len(), 1);
    assert_bar(
        &page.items["AAPL"][0],
        "2022-02-01T05:00:00Z",
        174.0,
        174.84,
        172.31,
        174.61,
        85998033,
        732412,
        173.703516,
    );
    assert_eq!(page.next_page_token, None);

    let server = serve_fixture("test_get_bars_multi_empty_response", "/v2/stocks/bars");
    let page = client(&server)
        .stock_bars(&bars_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert!(page.items.is_empty());
    assert_eq!(page.next_page_token, None);
}

#[tokio::test]
async fn the_trades_fixtures_are_read_field_by_field() {
    let server = serve_fixture("test_get_trades", "/v2/stocks/trades");
    let page = client(&server)
        .stock_trades(&history_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    let trades = &page.items["AAPL"];
    assert_eq!(trades.len(), 2);
    // Three and two fractional digits.
    assert_trade(&trades[0], "2022-03-09T05:00:02.183Z", 159.07, 1, 151);
    assert_trade(&trades[1], "2022-03-09T05:00:16.91Z", 159.07, 2, 168);
    assert_eq!(
        page.next_page_token.as_deref(),
        Some("QUFQTHwyMDIyLTAzLTA5VDA1OjAwOjE2LjkxMDAwMDAwMFp8RHwwOTIyMzM3MjAzNjg1NDc3NTk3Ng==")
    );
    assert_eq!(path(&only_request(&server)), "/v2/stocks/trades");

    let server = serve_fixture("test_multisymbol_get_trades", "/v2/stocks/trades");
    let page = client(&server)
        .stock_trades(&history_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(page.items.len(), 2);
    assert_trade(
        &page.items["AAPL"][0],
        "2022-03-09T05:00:02.183Z",
        159.07,
        1,
        151,
    );
    assert_trade(
        &page.items["TSLA"][0],
        "2022-03-09T05:08:03.035Z",
        833.0,
        1,
        145,
    );
    assert_eq!(page.next_page_token, None);

    let server = serve_fixture("test_get_trades_multi_empty_response", "/v2/stocks/trades");
    let page = client(&server)
        .stock_trades(&history_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert!(page.items.is_empty());
    assert_eq!(page.next_page_token, None);
}

#[tokio::test]
async fn the_quotes_fixtures_are_read_field_by_field() {
    let server = serve_fixture("test_get_quotes", "/v2/stocks/quotes");
    let page = client(&server)
        .stock_quotes(&history_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    let quotes = &page.items["AAPL"];
    assert_eq!(quotes.len(), 2);
    // Six fractional digits; an ask below the bid is kept as sent.
    assert_quote(
        &quotes[0],
        "2022-03-09T09:00:00.000059Z",
        158.65,
        "Q",
        159.52,
        4,
    );
    assert_quote(
        &quotes[1],
        "2022-03-09T09:00:00.000059Z",
        158.8,
        "Q",
        159.52,
        4,
    );
    assert_eq!(
        page.next_page_token.as_deref(),
        Some("QUFQTHwyMDIyLTAzLTA5VDA5OjAwOjAwLjAwMDA1OTAwMFp8Q0ZEQUU5QTg=")
    );
    assert_eq!(path(&only_request(&server)), "/v2/stocks/quotes");

    let server = serve_fixture("test_multisymbol_quotes", "/v2/stocks/quotes");
    let page = client(&server)
        .stock_quotes(&history_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(page.items.len(), 2);
    assert_quote(
        &page.items["AAPL"][0],
        "2022-03-09T09:00:00.000059Z",
        158.65,
        "Q",
        159.52,
        4,
    );
    assert_quote(
        &page.items["TSLA"][0],
        "2022-03-09T09:00:00.000805Z",
        830.0,
        "P",
        840.75,
        1,
    );
    assert_eq!(page.next_page_token, None);

    // The token key comes first in this body.
    let server = serve_fixture("test_get_quotes_multi_empty_response", "/v2/stocks/quotes");
    let page = client(&server)
        .stock_quotes(&history_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert!(page.items.is_empty());
    assert_eq!(page.next_page_token, None);
}

// The documented example's AAPL days, as sent.
const AUCTIONS: &str = r#"{"auctions":{"AAPL":[
{"c":[{"c":"6","p":138.36,"t":"2022-10-12T20:00:00.120649216Z","x":"P"},
      {"c":"M","p":138.36,"t":"2022-10-12T20:00:00.125925888Z","x":"P"},
      {"c":"6","p":138.34,"t":"2022-10-12T20:00:00.875570864Z","x":"Q"},
      {"c":"M","p":138.34,"t":"2022-10-12T20:00:00.875603021Z","x":"Q"}],
 "d":"2022-10-12",
 "o":[{"c":"Q","p":139.12,"t":"2022-10-12T13:30:00.188390144Z","x":"P"},
      {"c":"O","p":138.99,"t":"2022-10-12T13:30:01.474665705Z","x":"Q"},
      {"c":"Q","p":138.99,"t":"2022-10-12T13:30:01.475216565Z","x":"Q"}]},
{"c":[{"c":"M","p":142.94,"t":"2022-10-13T20:00:00.166980864Z","x":"P"}],
 "d":"2022-10-13",
 "o":[{"c":"Q","p":134.8,"t":"2022-10-13T13:30:00.20304384Z","x":"P"},
      {"c":"O","p":135,"t":"2022-10-13T13:30:01.688322951Z","x":"Q"},
      {"c":"Q","p":135,"t":"2022-10-13T13:30:01.699259366Z","x":"Q"}]}]},
"next_page_token":"MjAyMi0xMC0xM1QyMDowMDowMC4xNTI5MDI5MTJafFA="}"#;

fn assert_auctions(auctions: &[Auction], expected: &[(&str, f64, &str, &str)]) {
    assert_eq!(auctions.len(), expected.len());
    for (auction, (condition, price, t, exchange)) in auctions.iter().zip(expected) {
        assert_eq!(auction.condition, *condition);
        assert_eq!(auction.price, *price);
        assert_eq!(auction.timestamp, at(t));
        assert_eq!(auction.exchange, *exchange);
        assert_eq!(auction.size, None);
    }
}

#[tokio::test]
async fn the_documented_auctions_are_read_field_by_field() {
    let server = serve("200 OK", AUCTIONS.as_bytes());
    let page = client(&server)
        .stock_auctions(&history_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(path(&only_request(&server)), "/v2/stocks/auctions");
    let days = &page.items["AAPL"];
    assert_eq!(days.len(), 2);
    assert_eq!(days[0].date, "2022-10-12".parse::<Date>().unwrap());
    assert_auctions(
        &days[0].closing,
        &[
            ("6", 138.36, "2022-10-12T20:00:00.120649216Z", "P"),
            ("M", 138.36, "2022-10-12T20:00:00.125925888Z", "P"),
            ("6", 138.34, "2022-10-12T20:00:00.875570864Z", "Q"),
            ("M", 138.34, "2022-10-12T20:00:00.875603021Z", "Q"),
        ],
    );
    assert_auctions(
        &days[0].opening,
        &[
            ("Q", 139.12, "2022-10-12T13:30:00.188390144Z", "P"),
            ("O", 138.99, "2022-10-12T13:30:01.474665705Z", "Q"),
            ("Q", 138.99, "2022-10-12T13:30:01.475216565Z", "Q"),
        ],
    );
    assert_eq!(days[1].date, "2022-10-13".parse::<Date>().unwrap());
    assert_auctions(
        &days[1].closing,
        &[("M", 142.94, "2022-10-13T20:00:00.166980864Z", "P")],
    );
    assert_auctions(
        &days[1].opening,
        &[
            ("Q", 134.8, "2022-10-13T13:30:00.20304384Z", "P"),
            ("O", 135.0, "2022-10-13T13:30:01.688322951Z", "Q"),
            ("Q", 135.0, "2022-10-13T13:30:01.699259366Z", "Q"),
        ],
    );
    assert_eq!(page.currency, None);
    assert_eq!(
        page.next_page_token.as_deref(),
        Some("MjAyMi0xMC0xM1QyMDowMDowMC4xNTI5MDI5MTJafFA=")
    );

    // An auction with a size, and a currency beside the page.
    let body = r#"{"auctions":{"X":[{"d":"2022-10-12","o":[{"c":"Q","p":1.5,"s":300,"t":"2022-10-12T13:30:00Z","x":"P"}],"c":[]}]},"currency":"USD","next_page_token":null}"#;
    let server = serve("200 OK", body.as_bytes());
    let page = client(&server)
        .stock_auctions(&history_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(page.items["X"][0].opening[0].size, Some(300));
    assert!(page.items["X"][0].closing.is_empty());
    assert_eq!(page.currency.as_deref(), Some("USD"));
}

const TRADE: &str =
    r#"{"t":"2022-03-09T05:00:02.183Z","x":"D","p":159.07,"s":1,"c":["@"],"i":151,"z":"C""#;

#[tokio::test]
async fn optional_and_unknown_values_are_kept() {
    let body = format!(
        r#"{{"trades":{{"AAPL":[{TRADE},"u":"canceled"}},{TRADE},"u":"incorrect"}},{TRADE},"u":"corrected"}},{TRADE},"u":"retracted"}},{TRADE}}}]}},"currency":"USD"}}"#
    );
    let server = serve("200 OK", body.as_bytes());
    let page = client(&server)
        .stock_trades(&history_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    let updates: Vec<_> = page.items["AAPL"]
        .iter()
        .map(|trade| trade.update.clone())
        .collect();
    assert_eq!(
        updates,
        [
            Some(Inbound::Known(TradeUpdateKind::Canceled)),
            Some(Inbound::Known(TradeUpdateKind::Incorrect)),
            Some(Inbound::Known(TradeUpdateKind::Corrected)),
            Some(Inbound::Unknown("retracted".to_owned())),
            None,
        ]
    );
    // No next_page_token key is a last page.
    assert_eq!(page.next_page_token, None);
    assert_eq!(page.currency.as_deref(), Some("USD"));

    // A quote with no active ask: a zero price and a single-space exchange,
    // on a tape suri does not know.
    let body = r#"{"quotes":{"AAPL":[{"t":"2022-01-03T09:00:00Z","ax":" ","ap":0,"as":0,"bx":"Q","bp":1.5,"bs":2,"c":["R","T"],"z":"Q"}]},"next_page_token":null}"#;
    let server = serve("200 OK", body.as_bytes());
    let page = client(&server)
        .stock_quotes(&history_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    let quote = &page.items["AAPL"][0];
    assert_eq!(quote.ask_exchange, " ");
    assert_eq!(quote.ask_price, 0.0);
    assert_eq!(quote.ask_size, 0);
    assert_eq!(quote.conditions, ["R", "T"]);
    assert_eq!(quote.tape, Inbound::Unknown("Q".to_owned()));
}

#[tokio::test]
async fn unknown_keys_are_ignored_at_every_level() {
    let body = r#"{"bars":{"AAPL":[{"t":"2022-01-03T09:00:00Z","o":1,"h":2,"l":0.5,"c":1.5,"v":10,"n":2,"vw":1.25,"extra":[1]}]},"next_page_token":null,"surprise":{"a":1}}"#;
    let server = serve("200 OK", body.as_bytes());
    let page = client(&server)
        .stock_bars(&bars_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_bar(
        &page.items["AAPL"][0],
        "2022-01-03T09:00:00Z",
        1.0,
        2.0,
        0.5,
        1.5,
        10,
        2,
        1.25,
    );

    let body = r#"{"auctions":{"X":[{"d":"2022-10-12","o":[{"c":"Q","p":1,"t":"2022-10-12T13:30:00Z","x":"P","new":true}],"c":[],"extra":0}]},"next_page_token":null}"#;
    let server = serve("200 OK", body.as_bytes());
    let page = client(&server)
        .stock_auctions(&history_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(page.items["X"][0].opening[0].price, 1.0);
}

#[tokio::test]
async fn a_shape_the_docs_do_not_allow_fails_the_page() {
    for body in [
        // A symbol whose value is null.
        r#"{"trades":{"AAPL":null},"next_page_token":null}"#,
        // No points key at all.
        r#"{"next_page_token":null}"#,
        // A fractional size, and a negative one.
        r#"{"trades":{"AAPL":[{"t":"2022-03-09T05:00:02Z","x":"D","p":1,"s":1.5,"c":[],"i":1,"z":"C"}]}}"#,
        r#"{"trades":{"AAPL":[{"t":"2022-03-09T05:00:02Z","x":"D","p":1,"s":-1,"c":[],"i":1,"z":"C"}]}}"#,
    ] {
        let server = serve("200 OK", body.as_bytes());
        let error = client(&server)
            .stock_trades(&history_query())
            .await
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Decode, "{body}");
    }
}

#[tokio::test]
async fn every_double_reads_as_the_nearest_f64() {
    for sample in SAMPLES {
        let expected: f64 = sample.parse().unwrap();
        let same = |value: f64| assert_eq!(value.to_bits(), expected.to_bits(), "{sample}");

        let body = format!(
            r#"{{"bars":{{"A":[{{"t":"2022-01-03T09:00:00Z","o":{sample},"h":{sample},"l":{sample},"c":{sample},"v":1,"n":1,"vw":{sample}}}]}}}}"#
        );
        let server = serve("200 OK", body.as_bytes());
        let page = client(&server)
            .stock_bars(&bars_query())
            .await
            .unwrap()
            .into_parts()
            .0;
        let bar = &page.items["A"][0];
        for value in [bar.open, bar.high, bar.low, bar.close, bar.vwap] {
            same(value);
        }

        let body = format!(
            r#"{{"trades":{{"A":[{{"t":"2022-01-03T09:00:00Z","x":"D","p":{sample},"s":1,"c":[],"i":1,"z":"C"}}]}}}}"#
        );
        let server = serve("200 OK", body.as_bytes());
        let page = client(&server)
            .stock_trades(&history_query())
            .await
            .unwrap()
            .into_parts()
            .0;
        same(page.items["A"][0].price);

        let body = format!(
            r#"{{"quotes":{{"A":[{{"t":"2022-01-03T09:00:00Z","ax":"K","ap":{sample},"as":1,"bx":"Q","bp":{sample},"bs":1,"c":[],"z":"C"}}]}}}}"#
        );
        let server = serve("200 OK", body.as_bytes());
        let page = client(&server)
            .stock_quotes(&history_query())
            .await
            .unwrap()
            .into_parts()
            .0;
        same(page.items["A"][0].ask_price);
        same(page.items["A"][0].bid_price);

        let body = format!(
            r#"{{"auctions":{{"A":[{{"d":"2022-10-12","o":[{{"c":"Q","p":{sample},"t":"2022-10-12T13:30:00Z","x":"P"}}],"c":[]}}]}}}}"#
        );
        let server = serve("200 OK", body.as_bytes());
        let page = client(&server)
            .stock_auctions(&history_query())
            .await
            .unwrap()
            .into_parts()
            .0;
        same(page.items["A"][0].opening[0].price);
    }
    // The documented rounding of a 16-digit integer.
    assert_eq!(
        "9007199254740993".parse::<f64>().unwrap(),
        9007199254740992.0
    );
}

#[tokio::test]
async fn integers_keep_their_full_width() {
    let body = r#"{"trades":{"A":[{"t":"2022-01-03T09:00:00Z","x":"D","p":1,"s":4294967295,"c":[],"i":18446744073709551615,"z":"C"}]}}"#;
    let server = serve("200 OK", body.as_bytes());
    let page = client(&server)
        .stock_trades(&history_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(page.items["A"][0].id, u64::MAX);
    assert_eq!(page.items["A"][0].size, u32::MAX);

    let body = r#"{"bars":{"A":[{"t":"2022-01-03T09:00:00Z","o":1,"h":1,"l":1,"c":1,"v":9223372036854775807,"n":9223372036854775807,"vw":1}]}}"#;
    let server = serve("200 OK", body.as_bytes());
    let page = client(&server)
        .stock_bars(&bars_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(page.items["A"][0].volume, i64::MAX);
    assert_eq!(page.items["A"][0].trade_count, i64::MAX);
}

const EMPTY_BARS: &[u8] = br#"{"bars":{},"next_page_token":null}"#;

#[tokio::test]
async fn every_bars_parameter_goes_out_in_the_documented_order() {
    let server = serve("200 OK", EMPTY_BARS);
    let mut query = BarsQuery::new(symbols(&["AAPL", "TSLA"]), Timeframe::Minutes(5));
    query.start = Some(at("2024-01-03T01:02:03.123456789Z"));
    query.end = Some(at("2024-01-04T00:00:00Z"));
    query.limit = Some(10000);
    query.adjustment = Some(vec![Adjustment::Split, Adjustment::SpinOff]);
    query.asof = Some(AsOf::Date("2022-03-09".parse().unwrap()));
    query.feed = Some(Feed::Iex);
    query.currency = Some("JPY".to_owned());
    query.page_token = Some("QUFQTHxE+/=".to_owned());
    query.sort = Some(Sort::Desc);
    client(&server).stock_bars(&query).await.unwrap();
    let request = only_request(&server);
    assert_eq!(request.method, "GET");
    assert_eq!(path(&request), "/v2/stocks/bars");
    // Market Data requests carry the key pair, as every request does.
    assert_eq!(request.header("APCA-API-KEY-ID"), Some(requests::KEY));
    assert_eq!(
        request.header("APCA-API-SECRET-KEY"),
        Some(requests::SECRET)
    );
    assert_eq!(
        pairs(&request),
        owned(&[
            ("symbols", "AAPL,TSLA"),
            ("timeframe", "5Min"),
            ("start", "2024-01-03T01:02:03.123456789Z"),
            ("end", "2024-01-04T00:00:00Z"),
            ("limit", "10000"),
            ("adjustment", "split,spin-off"),
            ("asof", "2022-03-09"),
            ("feed", "iex"),
            ("currency", "JPY"),
            ("page_token", "QUFQTHxE+/="),
            ("sort", "desc"),
        ])
    );
    assert!(request.body.is_empty());
}

#[tokio::test]
async fn each_timeframe_is_sent_in_its_long_form() {
    for (timeframe, text) in [
        (Timeframe::Minutes(5), "5Min"),
        (Timeframe::Hours(2), "2Hour"),
        (Timeframe::Day, "1Day"),
        (Timeframe::Week, "1Week"),
        (Timeframe::Months(4), "4Month"),
        // A range Alpaca rejects is sent as given.
        (Timeframe::Minutes(0), "0Min"),
    ] {
        assert_eq!(timeframe.to_string(), text);
        let server = serve("200 OK", EMPTY_BARS);
        client(&server)
            .stock_bars(&BarsQuery::new(symbols(&["AAPL"]), timeframe))
            .await
            .unwrap();
        assert_eq!(
            pairs(&only_request(&server)),
            owned(&[("symbols", "AAPL"), ("timeframe", text)])
        );
    }
}

#[tokio::test]
async fn every_history_parameter_goes_out_in_the_documented_order() {
    let mut query = HistoryQuery::new(symbols(&["AAPL"]));
    query.start = Some(at("2024-01-03T00:00:00Z"));
    query.end = Some(at("2024-01-04T00:00:00.5Z"));
    query.limit = Some(1);
    query.asof = Some(AsOf::Unmapped);
    query.feed = Some(Feed::Sip);
    query.currency = Some("USD".to_owned());
    query.page_token = Some("abc".to_owned());
    query.sort = Some(Sort::Asc);
    let expected = owned(&[
        ("symbols", "AAPL"),
        ("start", "2024-01-03T00:00:00Z"),
        ("end", "2024-01-04T00:00:00.5Z"),
        ("limit", "1"),
        ("asof", "-"),
        ("feed", "sip"),
        ("currency", "USD"),
        ("page_token", "abc"),
        ("sort", "asc"),
    ]);
    for (route, body) in [
        ("/v2/stocks/trades", &br#"{"trades":{}}"#[..]),
        ("/v2/stocks/quotes", br#"{"quotes":{}}"#),
        ("/v2/stocks/auctions", br#"{"auctions":{}}"#),
    ] {
        let server = serve("200 OK", body);
        let client = client(&server);
        match route {
            "/v2/stocks/trades" => drop(client.stock_trades(&query).await.unwrap()),
            "/v2/stocks/quotes" => drop(client.stock_quotes(&query).await.unwrap()),
            _ => drop(client.stock_auctions(&query).await.unwrap()),
        }
        let request = only_request(&server);
        assert_eq!(path(&request), route);
        assert_eq!(pairs(&request), expected, "{route}");

        // Only the required symbols.
        let server = serve("200 OK", body);
        let client = requests::client(&server);
        let minimal = HistoryQuery::new(symbols(&["AAPL", "BRK.B"]));
        match route {
            "/v2/stocks/trades" => drop(client.stock_trades(&minimal).await.unwrap()),
            "/v2/stocks/quotes" => drop(client.stock_quotes(&minimal).await.unwrap()),
            _ => drop(client.stock_auctions(&minimal).await.unwrap()),
        }
        assert_eq!(
            pairs(&only_request(&server)),
            owned(&[("symbols", "AAPL,BRK.B")])
        );
    }
}

#[tokio::test]
async fn a_read_is_retried() {
    let server = failing_once(EMPTY_BARS);
    let page = retrying_client(&server)
        .stock_bars(&bars_query())
        .await
        .unwrap();
    assert!(page.data().items.is_empty());
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].path, requests[1].path);
}

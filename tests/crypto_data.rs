//! Crypto market data: the upstream fixtures field by field, the documented
//! orderbooks, numbers and identifiers as sent, unknown values kept, and
//! every location and query parameter on the wire.

#![cfg(feature = "rest")]

#[path = "support/fixture.rs"]
mod fixture;
#[path = "support/requests.rs"]
mod requests;
#[path = "support/server.rs"]
mod server;
#[path = "support/stock.rs"]
mod stock;

use requests::{client, only_request, owned, pairs, path, serve};
use server::Server;
use stock::{SAMPLES, at, symbols};
use suri::data::crypto::{
    Bar, BarsQuery, HistoryQuery, Location, Orderbook, Quote, SymbolsQuery, TakerSide, Timeframe,
    Trade,
};
use suri::{Inbound, Sort};

const ROUTES: &str = "data/test_historical_crypto_data";

fn serve_fixture(name: &str, route: &str) -> Server {
    let fixture = fixture::upstream(&format!("{ROUTES}::{name}::GET {route}#0"));
    assert_eq!(fixture.status, 200);
    serve("200 OK", &fixture.body)
}

fn pair(list: &[&str]) -> SymbolsQuery {
    SymbolsQuery::new(symbols(list))
}

#[allow(clippy::too_many_arguments)]
fn assert_bar(
    bar: &Bar,
    t: &str,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
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

fn assert_trade(trade: &Trade, t: &str, price: f64, size: f64, id: i64, side: TakerSide) {
    assert_eq!(trade.timestamp, at(t));
    assert_eq!(trade.price, price);
    assert_eq!(trade.size, size);
    assert_eq!(trade.id, id);
    assert_eq!(trade.taker_side, Inbound::Known(side));
}

fn assert_quote(quote: &Quote, t: &str, bid: (f64, f64), ask: (f64, f64)) {
    assert_eq!(quote.timestamp, at(t));
    assert_eq!((quote.bid_price, quote.bid_size), bid);
    assert_eq!((quote.ask_price, quote.ask_size), ask);
}

#[tokio::test]
async fn the_history_fixtures_are_read_field_by_field() {
    let route = "/v1beta3/crypto/us/bars";
    let server = serve_fixture("test_get_crypto_bars", route);
    let query = BarsQuery::new(symbols(&["BTC/USD"]), Timeframe::Day);
    let page = client(&server)
        .crypto_bars(Location::Us, &query)
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(path(&only_request(&server)), route);
    assert_eq!(page.items.len(), 2);
    // An integral volume reads into a double.
    assert_bar(
        &page.items["BTC/USD"][0],
        "2022-03-09T05:00:00Z",
        161.51,
        163.41,
        159.41,
        162.95,
        88496480.0,
        700291,
        161.942117,
    );
    assert_bar(
        &page.items["ETH/USD"][0],
        "2022-03-09T05:00:00Z",
        839.0,
        860.56,
        832.01,
        858.97,
        19227323.0,
        528531,
        850.616587,
    );
    assert_eq!(page.next_page_token, None);
    assert_eq!(page.currency, None);

    // An empty page with no token key at all.
    let server = serve_fixture("test_crypto_client_without_credentials", route);
    let page = client(&server)
        .crypto_bars(Location::Us, &query)
        .await
        .unwrap()
        .into_parts()
        .0;
    assert!(page.items.is_empty());
    assert_eq!(page.next_page_token, None);

    let route = "/v1beta3/crypto/us/trades";
    let server = serve_fixture("test_get_crypto_trades", route);
    let history = HistoryQuery::new(symbols(&["BTC/USD"]));
    let page = client(&server)
        .crypto_trades(Location::Us, &history)
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(path(&only_request(&server)), route);
    assert_trade(
        &page.items["BTC/USD"][0],
        "2022-03-09T06:00:00.080264Z",
        41516.08,
        0.00315427,
        293648598,
        TakerSide::Buyer,
    );
    assert_trade(
        &page.items["ETH/USD"][0],
        "2022-03-09T06:00:00.228546Z",
        2715.06,
        0.001,
        236866246,
        TakerSide::Seller,
    );
    assert_eq!(page.next_page_token, None);

    let route = "/v1beta3/crypto/us/quotes";
    let server = serve_fixture("test_get_crypto_quotes", route);
    let page = client(&server)
        .crypto_quotes(Location::Us, &history)
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(path(&only_request(&server)), route);
    // Eight and nine fractional digits.
    assert_quote(
        &page.items["BTC/USD"][0],
        "2022-05-26T11:47:18.44347136Z",
        (29058.0, 0.3544),
        (29059.0, 3.252),
    );
    assert_quote(
        &page.items["ETH/USD"][0],
        "2022-05-26T11:47:18.499478272Z",
        (1817.0, 4.76),
        (1817.7, 6.137),
    );
    assert_eq!(page.next_page_token, None);
}

#[tokio::test]
async fn the_latest_fixtures_are_read_field_by_field() {
    // A token key the docs do not list beside the latest bars is ignored.
    let route = "/v1beta3/crypto/us/latest/bars";
    let server = serve_fixture("test_crypto_latest_bar", route);
    let latest = client(&server)
        .crypto_latest_bars(Location::Us, &pair(&["BTC/USD"]))
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(path(&only_request(&server)), route);
    assert_eq!(latest.items.len(), 1);
    assert_bar(
        &latest.items["BTC/USD"],
        "2022-05-27T10:18:00Z",
        28999.0,
        29003.0,
        28999.0,
        29003.0,
        0.01,
        4,
        29001.0,
    );

    let route = "/v1beta3/crypto/us/latest/trades";
    let server = serve_fixture("test_get_crypto_latest_trade", route);
    let latest = client(&server)
        .crypto_latest_trades(Location::Us, &pair(&["BTC/USD"]))
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(path(&only_request(&server)), route);
    assert_trade(
        &latest.items["BTC/USD"],
        "2022-03-18T14:03:31.960672Z",
        40650.0,
        0.1517,
        26932440,
        TakerSide::Buyer,
    );

    let route = "/v1beta3/crypto/us/latest/quotes";
    let server = serve_fixture("test_get_crypto_latest_quote", route);
    let latest = client(&server)
        .crypto_latest_quotes(Location::Us, &pair(&["BTC/USD"]))
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(path(&only_request(&server)), route);
    assert_quote(
        &latest.items["BTC/USD"],
        "2022-03-18T14:03:13.661518592Z",
        (40517.08, 4.0178),
        (40765.93, 1.5516),
    );
}

#[tokio::test]
async fn the_snapshot_fixture_is_read_field_by_field() {
    let route = "/v1beta3/crypto/us/snapshots";
    let server = serve_fixture("test_crypto_get_snapshot", route);
    let latest = client(&server)
        .crypto_snapshots(Location::Us, &pair(&["ETH/USD", "BTC/USD"]))
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(path(&only_request(&server)), route);
    assert_eq!(latest.items.len(), 2);

    let eth = &latest.items["ETH/USD"];
    assert_trade(
        eth.latest_trade.as_ref().unwrap(),
        "2022-03-28T17:33:20.180926Z",
        3373.04,
        0.2436732,
        247644006,
        TakerSide::Seller,
    );
    assert_quote(
        eth.latest_quote.as_ref().unwrap(),
        "2022-03-28T17:32:30.318Z",
        (3374.33, 0.001),
        (3374.34, 0.001),
    );
    assert_bar(
        eth.minute_bar.as_ref().unwrap(),
        "2022-03-28T17:32:00Z",
        3368.5,
        3376.31,
        3366.76,
        3374.07,
        907.81905184,
        1149,
        3372.4441210299,
    );
    // An integral low in the daily bar.
    assert_bar(
        eth.daily_bar.as_ref().unwrap(),
        "2022-03-28T05:00:00Z",
        3319.63,
        3402.17,
        3305.0,
        3374.07,
        112120.6572392,
        382244,
        3353.1926442533,
    );
    assert_bar(
        eth.previous_daily_bar.as_ref().unwrap(),
        "2022-03-27T05:00:00Z",
        3140.35,
        3328.83,
        3127.74,
        3319.46,
        143163.68434276,
        493112,
        3240.5911005308,
    );

    let btc = &latest.items["BTC/USD"];
    // Five fractional digits.
    assert_trade(
        btc.latest_trade.as_ref().unwrap(),
        "2022-03-28T17:33:20.19842Z",
        47537.64,
        0.00023096,
        304775291,
        TakerSide::Buyer,
    );
    assert_quote(
        btc.latest_quote.as_ref().unwrap(),
        "2022-03-28T17:32:30.824Z",
        (47530.99, 1.65666998),
        (47531.66, 0.00209349),
    );
    assert_bar(
        btc.minute_bar.as_ref().unwrap(),
        "2022-03-28T17:32:00Z",
        47494.71,
        47542.23,
        47471.56,
        47532.2,
        41.34429609,
        873,
        47515.679114746,
    );
    assert_bar(
        btc.daily_bar.as_ref().unwrap(),
        "2022-03-28T05:00:00Z",
        47149.3,
        47900.0,
        46770.7,
        47532.2,
        8888.08292165,
        348798,
        47385.1549250663,
    );
    assert_bar(
        btc.previous_daily_bar.as_ref().unwrap(),
        "2022-03-27T05:00:00Z",
        44598.44,
        47694.0,
        44437.22,
        47148.98,
        12278.6017929,
        559695,
        46041.2023793348,
    );
}

// The documented example, as sent.
const ORDERBOOKS: &str = r#"{"orderbooks":{"BTC/USD":{"a":[{"p":20902,"s":0.0097},{"p":21444,"s":0}],"b":[{"p":20846,"s":0.1902},{"p":20350,"s":0}],"t":"2022-06-24T08:00:14.137774336Z"}}}"#;

fn levels(book: &[suri::data::crypto::OrderbookLevel]) -> Vec<(f64, f64)> {
    book.iter().map(|level| (level.price, level.size)).collect()
}

#[tokio::test]
async fn the_documented_orderbook_is_read_as_sent() {
    let route = "/v1beta3/crypto/us/latest/orderbooks";
    let server = serve("200 OK", ORDERBOOKS.as_bytes());
    let latest = client(&server)
        .crypto_latest_orderbooks(Location::Us, &pair(&["BTC/USD"]))
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(path(&only_request(&server)), route);
    let book: &Orderbook = &latest.items["BTC/USD"];
    assert_eq!(book.timestamp, at("2022-06-24T08:00:14.137774336Z"));
    // Levels in the order sent, zero sizes included.
    assert_eq!(levels(&book.asks), [(20902.0, 0.0097), (21444.0, 0.0)]);
    assert_eq!(levels(&book.bids), [(20846.0, 0.1902), (20350.0, 0.0)]);
    assert!(!book.reset);

    // A reset flag, as the stream sends it, is read.
    let body = r#"{"orderbooks":{"X":{"a":[],"b":[],"t":"2022-06-24T08:00:14Z","r":true}}}"#;
    let server = serve("200 OK", body.as_bytes());
    let latest = client(&server)
        .crypto_latest_orderbooks(Location::Us, &pair(&["X"]))
        .await
        .unwrap()
        .into_parts()
        .0;
    assert!(latest.items["X"].reset);
}

#[tokio::test]
async fn unknown_values_and_missing_members_are_kept() {
    let trade =
        r#"{"t":"2022-03-18T14:03:31Z","p":1,"s":1,"tks":"X","i":3447222699101865076,"new":[1]}"#;
    let body =
        format!(r#"{{"snapshots":{{"A":{{"latestTrade":{trade},"other":{{}}}}}},"extra":true}}"#);
    let server = serve("200 OK", body.as_bytes());
    let latest = client(&server)
        .crypto_snapshots(Location::Us, &pair(&["A"]))
        .await
        .unwrap()
        .into_parts()
        .0;
    let snapshot = &latest.items["A"];
    let trade = snapshot.latest_trade.as_ref().unwrap();
    assert_eq!(trade.taker_side, Inbound::Unknown("X".to_owned()));
    // An ID above 2^53 is kept exactly.
    assert_eq!(trade.id, 3447222699101865076);
    assert_eq!(snapshot.latest_quote, None);
    assert_eq!(snapshot.minute_bar, None);
    assert_eq!(snapshot.daily_bar, None);
    assert_eq!(snapshot.previous_daily_bar, None);
}

#[tokio::test]
async fn every_double_reads_as_the_nearest_f64() {
    for sample in SAMPLES {
        let expected: f64 = sample.parse().unwrap();
        let same = |value: f64| assert_eq!(value.to_bits(), expected.to_bits(), "{sample}");

        let body = format!(
            r#"{{"bars":{{"A":{{"t":"2022-01-03T09:00:00Z","o":{sample},"h":{sample},"l":{sample},"c":{sample},"v":{sample},"n":1,"vw":{sample}}}}}}}"#
        );
        let server = serve("200 OK", body.as_bytes());
        let latest = client(&server)
            .crypto_latest_bars(Location::Us, &pair(&["A"]))
            .await
            .unwrap()
            .into_parts()
            .0;
        let bar = &latest.items["A"];
        for value in [bar.open, bar.high, bar.low, bar.close, bar.volume, bar.vwap] {
            same(value);
        }

        let body = format!(
            r#"{{"trades":{{"A":{{"t":"2022-01-03T09:00:00Z","p":{sample},"s":{sample},"tks":"B","i":1}}}}}}"#
        );
        let server = serve("200 OK", body.as_bytes());
        let latest = client(&server)
            .crypto_latest_trades(Location::Us, &pair(&["A"]))
            .await
            .unwrap()
            .into_parts()
            .0;
        same(latest.items["A"].price);
        same(latest.items["A"].size);

        let body = format!(
            r#"{{"quotes":{{"A":{{"t":"2022-01-03T09:00:00Z","bp":{sample},"bs":{sample},"ap":{sample},"as":{sample}}}}}}}"#
        );
        let server = serve("200 OK", body.as_bytes());
        let latest = client(&server)
            .crypto_latest_quotes(Location::Us, &pair(&["A"]))
            .await
            .unwrap()
            .into_parts()
            .0;
        let quote = &latest.items["A"];
        for value in [
            quote.bid_price,
            quote.bid_size,
            quote.ask_price,
            quote.ask_size,
        ] {
            same(value);
        }

        let body = format!(
            r#"{{"orderbooks":{{"A":{{"t":"2022-01-03T09:00:00Z","a":[{{"p":{sample},"s":{sample}}}],"b":[{{"p":{sample},"s":{sample}}}]}}}}}}"#
        );
        let server = serve("200 OK", body.as_bytes());
        let latest = client(&server)
            .crypto_latest_orderbooks(Location::Us, &pair(&["A"]))
            .await
            .unwrap()
            .into_parts()
            .0;
        let book = &latest.items["A"];
        for level in book.asks.iter().chain(&book.bids) {
            same(level.price);
            same(level.size);
        }
    }
}

#[tokio::test]
async fn every_location_and_parameter_goes_out_as_documented() {
    let locations = [
        (Location::Us, "us"),
        (Location::Us1, "us-1"),
        (Location::Us2, "us-2"),
        (Location::Eu1, "eu-1"),
        (Location::Bs1, "bs-1"),
    ];
    for (location, text) in locations {
        assert_eq!(location.as_str(), text);
        let server = serve("200 OK", br#"{"bars":{}}"#);
        let mut query = BarsQuery::new(symbols(&["BTC/USD", "ETH/USD"]), Timeframe::Hours(4));
        query.start = Some(at("2024-01-03T01:02:03.123456789Z"));
        query.end = Some(at("2024-01-04T00:00:00Z"));
        query.limit = Some(10000);
        query.page_token = Some("MTY0MDk0ODkyMzAwMDAwMDAwMHwyNDg0MzE3MQ==".to_owned());
        query.sort = Some(Sort::Desc);
        client(&server).crypto_bars(location, &query).await.unwrap();
        let request = only_request(&server);
        assert_eq!(request.method, "GET");
        assert_eq!(path(&request), format!("/v1beta3/crypto/{text}/bars"));
        // The key pair goes out on historical crypto data too.
        assert_eq!(request.header("APCA-API-KEY-ID"), Some(requests::KEY));
        assert_eq!(
            request.header("APCA-API-SECRET-KEY"),
            Some(requests::SECRET)
        );
        // The slash in a pair is percent-encoded in the raw query.
        assert!(
            request.path.contains("symbols=BTC%2FUSD%2CETH%2FUSD"),
            "{}",
            request.path
        );
        assert_eq!(
            pairs(&request),
            owned(&[
                ("symbols", "BTC/USD,ETH/USD"),
                ("timeframe", "4Hour"),
                ("start", "2024-01-03T01:02:03.123456789Z"),
                ("end", "2024-01-04T00:00:00Z"),
                ("limit", "10000"),
                ("page_token", "MTY0MDk0ODkyMzAwMDAwMDAwMHwyNDg0MzE3MQ=="),
                ("sort", "desc"),
            ])
        );
    }

    let mut history = HistoryQuery::new(symbols(&["BTC/USD"]));
    history.start = Some(at("2024-01-03T00:00:00Z"));
    history.end = Some(at("2024-01-04T00:00:00Z"));
    history.limit = Some(5);
    history.page_token = Some("abc".to_owned());
    history.sort = Some(Sort::Asc);
    let expected = owned(&[
        ("symbols", "BTC/USD"),
        ("start", "2024-01-03T00:00:00Z"),
        ("end", "2024-01-04T00:00:00Z"),
        ("limit", "5"),
        ("page_token", "abc"),
        ("sort", "asc"),
    ]);
    let server = serve("200 OK", br#"{"trades":{}}"#);
    client(&server)
        .crypto_trades(Location::Us1, &history)
        .await
        .unwrap();
    let request = only_request(&server);
    assert_eq!(path(&request), "/v1beta3/crypto/us-1/trades");
    assert_eq!(pairs(&request), expected);
    let server = serve("200 OK", br#"{"quotes":{}}"#);
    client(&server)
        .crypto_quotes(Location::Eu1, &history)
        .await
        .unwrap();
    let request = only_request(&server);
    assert_eq!(path(&request), "/v1beta3/crypto/eu-1/quotes");
    assert_eq!(pairs(&request), expected);

    // The required parameters alone.
    let server = serve("200 OK", br#"{"trades":{}}"#);
    client(&server)
        .crypto_trades(Location::Us, &HistoryQuery::new(symbols(&["BTC/USD"])))
        .await
        .unwrap();
    assert_eq!(
        pairs(&only_request(&server)),
        owned(&[("symbols", "BTC/USD")])
    );
    let server = serve("200 OK", br#"{"bars":{}}"#);
    client(&server)
        .crypto_bars(
            Location::Us,
            &BarsQuery::new(symbols(&["BTC/USD"]), Timeframe::Minutes(1)),
        )
        .await
        .unwrap();
    assert_eq!(
        pairs(&only_request(&server)),
        owned(&[("symbols", "BTC/USD"), ("timeframe", "1Min")])
    );

    // The latest and snapshot endpoints take the symbols only.
    for (route, body) in [
        ("latest/bars", &br#"{"bars":{}}"#[..]),
        ("latest/trades", br#"{"trades":{}}"#),
        ("latest/quotes", br#"{"quotes":{}}"#),
        ("latest/orderbooks", br#"{"orderbooks":{}}"#),
        ("snapshots", br#"{"snapshots":{}}"#),
    ] {
        let server = serve("200 OK", body);
        let client = client(&server);
        let query = pair(&["BTC/USD", "ETH/BTC"]);
        match route {
            "latest/bars" => drop(
                client
                    .crypto_latest_bars(Location::Us2, &query)
                    .await
                    .unwrap(),
            ),
            "latest/trades" => drop(
                client
                    .crypto_latest_trades(Location::Us2, &query)
                    .await
                    .unwrap(),
            ),
            "latest/quotes" => drop(
                client
                    .crypto_latest_quotes(Location::Us2, &query)
                    .await
                    .unwrap(),
            ),
            "latest/orderbooks" => drop(
                client
                    .crypto_latest_orderbooks(Location::Us2, &query)
                    .await
                    .unwrap(),
            ),
            _ => drop(
                client
                    .crypto_snapshots(Location::Us2, &query)
                    .await
                    .unwrap(),
            ),
        }
        let request = only_request(&server);
        assert_eq!(path(&request), format!("/v1beta3/crypto/us-2/{route}"));
        assert_eq!(pairs(&request), owned(&[("symbols", "BTC/USD,ETH/BTC")]));
        // The key pair goes out although historical crypto data is
        // documented as needing none.
        assert_eq!(request.header("APCA-API-KEY-ID"), Some(requests::KEY));
    }
}

#[test]
fn each_taker_side_has_its_documented_text() {
    for (side, text) in [(TakerSide::Buyer, "B"), (TakerSide::Seller, "S")] {
        assert_eq!(side.as_str(), text);
        assert_eq!(text.parse::<TakerSide>(), Ok(side));
    }
}

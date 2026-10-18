//! Options market data: the upstream fixtures field by field, a fixture
//! that breaks the schema refused, the documented code tables, doubles read
//! correctly rounded, and every query parameter on the wire.

#![cfg(feature = "rest")]

#[path = "support/fixture.rs"]
mod fixture;
#[path = "support/requests.rs"]
mod requests;
#[path = "support/server.rs"]
mod server;
#[path = "support/stock.rs"]
mod stock;

use std::collections::BTreeMap;

use requests::{client, only_request, owned, pairs, path, serve};
use server::Server;
use stock::{SAMPLES, assert_bar, at, symbols};
use suri::data::options::{
    BarsQuery, ChainQuery, Feed, Greeks, HistoryQuery, LatestQuery, Quote, Snapshot,
    SnapshotsQuery, TickType, Timeframe, Trade,
};
use suri::trading::ContractType;
use suri::{ErrorKind, Sort, Symbol};

const ROUTES: &str = "data/test_historical_option_data";

fn serve_fixture(name: &str, route: &str) -> Server {
    let fixture = fixture::upstream(&format!("{ROUTES}::{name}::GET {route}#0"));
    assert_eq!(fixture.status, 200);
    serve("200 OK", &fixture.body)
}

fn assert_trade(trade: &Trade, t: &str, exchange: &str, price: f64, size: u32, condition: &str) {
    assert_eq!(trade.timestamp, at(t));
    assert_eq!(trade.exchange, exchange);
    assert_eq!(trade.price, price);
    assert_eq!(trade.size, size);
    assert_eq!(trade.condition, condition);
}

fn assert_quote(
    quote: &Quote,
    t: &str,
    bid: (&str, f64, u32),
    ask: (&str, f64, u32),
    condition: &str,
) {
    assert_eq!(quote.timestamp, at(t));
    assert_eq!(
        (quote.bid_exchange.as_str(), quote.bid_price, quote.bid_size),
        bid
    );
    assert_eq!(
        (quote.ask_exchange.as_str(), quote.ask_price, quote.ask_size),
        ask
    );
    assert_eq!(quote.condition, condition);
}

fn assert_greeks(greeks: &Greeks, [delta, gamma, rho, theta, vega]: [f64; 5]) {
    assert_eq!(greeks.delta, delta);
    assert_eq!(greeks.gamma, gamma);
    assert_eq!(greeks.rho, rho);
    assert_eq!(greeks.theta, theta);
    assert_eq!(greeks.vega, vega);
}

fn assert_no_bars(snapshot: &Snapshot) {
    assert_eq!(snapshot.minute_bar, None);
    assert_eq!(snapshot.daily_bar, None);
    assert_eq!(snapshot.previous_daily_bar, None);
}

#[tokio::test]
async fn the_bar_fixtures_are_read_field_by_field() {
    let route = "/v1beta1/options/bars";
    let query = BarsQuery::new(symbols(&["SPY240426C00555000"]), Timeframe::Hours(1));
    let server = serve_fixture("test_get_bars", route);
    let page = client(&server)
        .option_bars(&query)
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(path(&only_request(&server)), route);
    assert_eq!(page.items.len(), 1);
    let bars = &page.items["SPY240426C00555000"];
    assert_eq!(bars.len(), 2);
    assert_bar(
        &bars[0],
        "2024-03-12T13:00:00Z",
        0.24,
        0.24,
        0.21,
        0.21,
        21,
        2,
        0.211429,
    );
    assert_bar(
        &bars[1],
        "2024-03-12T14:00:00Z",
        0.25,
        0.3,
        0.25,
        0.3,
        3,
        3,
        0.273333,
    );
    assert_eq!(page.currency, None);
    assert_eq!(
        page.next_page_token.as_deref(),
        Some("U1BZMjQwNDI2QzAwNTU1MDAwfE18MTcxMDI1NDM0MDAwMDAwMDAwMA==")
    );

    let server = serve_fixture("test_multisymbol_get_bars", route);
    let page = client(&server)
        .option_bars(&query)
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(page.items.len(), 2);
    let bars = &page.items["SPY240426C00550000"];
    assert_eq!(bars.len(), 2);
    assert_bar(
        &bars[0],
        "2024-03-12T13:00:00Z",
        0.37,
        0.37,
        0.37,
        0.37,
        1,
        1,
        0.37,
    );
    assert_bar(
        &bars[1],
        "2024-03-12T14:00:00Z",
        0.41,
        0.52,
        0.41,
        0.51,
        16,
        6,
        0.496875,
    );
    let bars = &page.items["SPY240426C00555000"];
    assert_eq!(bars.len(), 1);
    assert_bar(
        &bars[0],
        "2024-03-12T13:00:00Z",
        0.24,
        0.24,
        0.21,
        0.21,
        21,
        2,
        0.211429,
    );
    assert_eq!(page.next_page_token, None);

    let server = serve_fixture("test_get_bars_multi_empty_response", route);
    let page = client(&server)
        .option_bars(&query)
        .await
        .unwrap()
        .into_parts()
        .0;
    assert!(page.items.is_empty());
    assert_eq!(page.next_page_token, None);
}

#[tokio::test]
async fn the_trade_fixtures_are_read_field_by_field() {
    let route = "/v1beta1/options/trades";
    let query = HistoryQuery::new(symbols(&["SPY240426C00555000"]));
    let server = serve_fixture("test_get_trades", route);
    let page = client(&server)
        .option_trades(&query)
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(path(&only_request(&server)), route);
    assert_eq!(page.items.len(), 1);
    let trades = &page.items["SPY240426C00555000"];
    assert_eq!(trades.len(), 2);
    assert_trade(
        &trades[0],
        "2024-03-12T13:34:54.178322688Z",
        "M",
        0.24,
        1,
        "I",
    );
    assert_trade(
        &trades[1],
        "2024-03-12T13:52:22.558447104Z",
        "B",
        0.21,
        20,
        "j",
    );
    assert_eq!(
        page.next_page_token.as_deref(),
        Some("U1BZMjQwNDI2QzAwNTU1MDAwfDE3MTAyNTE1NDI1NTg0NDcxMDR8Qg==")
    );

    let server = serve_fixture("test_multisymbol_get_trades", route);
    let page = client(&server)
        .option_trades(&query)
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(page.items.len(), 2);
    let trades = &page.items["SPY240426C00550000"];
    assert_eq!(trades.len(), 2);
    // Eight fractional digits.
    assert_trade(
        &trades[0],
        "2024-03-12T13:55:39.17011584Z",
        "C",
        0.37,
        1,
        "g",
    );
    assert_trade(
        &trades[1],
        "2024-03-12T14:10:14.088364544Z",
        "C",
        0.41,
        1,
        "a",
    );
    let trades = &page.items["SPY240426C00555000"];
    assert_eq!(trades.len(), 1);
    assert_trade(
        &trades[0],
        "2024-03-12T13:34:54.178322688Z",
        "M",
        0.24,
        1,
        "I",
    );
    assert_eq!(page.next_page_token, None);

    let server = serve_fixture("test_get_trades_multi_empty_response", route);
    let page = client(&server)
        .option_trades(&query)
        .await
        .unwrap()
        .into_parts()
        .0;
    assert!(page.items.is_empty());
    assert_eq!(page.next_page_token, None);
}

#[tokio::test]
async fn the_latest_fixtures_are_read_field_by_field() {
    let route = "/v1beta1/options/trades/latest";
    let query = LatestQuery::new(symbols(&["AAPL240126P00050000"]));
    let server = serve_fixture("test_get_latest_trade", route);
    let latest = client(&server)
        .option_latest_trades(&query)
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(path(&only_request(&server)), route);
    assert_eq!(latest.items.len(), 1);
    assert_trade(
        &latest.items["AAPL240126P00050000"],
        "2024-01-24T14:02:09.722539521Z",
        "A",
        0.06,
        1,
        "I",
    );
    assert_eq!(latest.currency, None);

    let server = serve_fixture("test_get_multisymbol_latest_trade", route);
    let latest = client(&server)
        .option_latest_trades(&query)
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(latest.items.len(), 2);
    assert_trade(
        &latest.items["AAPL240126P00050000"],
        "2024-01-24T14:02:09.722539521Z",
        "A",
        0.06,
        1,
        "I",
    );
    assert_trade(
        &latest.items["AAPL240126P00100000"],
        "2024-01-24T19:59:59.405545378Z",
        "M",
        0.59,
        50,
        "I",
    );

    let server = serve_fixture("test_get_latest_trade_multi_not_found", route);
    let latest = client(&server)
        .option_latest_trades(&query)
        .await
        .unwrap()
        .into_parts()
        .0;
    assert!(latest.items.is_empty());

    // This fixture names its map `trade`, not the schema's `trades`.
    let server = serve_fixture("test_get_latest_trade_with_feed", route);
    let error = client(&server)
        .option_latest_trades(&query)
        .await
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Decode);

    let route = "/v1beta1/options/quotes/latest";
    let server = serve_fixture("test_get_latest_quote", route);
    let latest = client(&server)
        .option_latest_quotes(&query)
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(path(&only_request(&server)), route);
    assert_eq!(latest.items.len(), 1);
    assert_quote(
        &latest.items["AAPL240126P00050000"],
        "2024-01-24T14:02:43.651613184Z",
        ("N", 0.05, 1344),
        ("N", 0.06, 1593),
        "B",
    );

    let server = serve_fixture("test_get_latest_quote_single_empty_response", route);
    let latest = client(&server)
        .option_latest_quotes(&query)
        .await
        .unwrap()
        .into_parts()
        .0;
    assert!(latest.items.is_empty());
}

#[tokio::test]
async fn the_snapshot_fixtures_are_read_field_by_field() {
    let route = "/v1beta1/options/snapshots";
    let query = SnapshotsQuery::new(symbols(&["AAPL240126P00050000"]));
    let server = serve_fixture("test_get_snapshot", route);
    let page = client(&server)
        .option_snapshots(&query)
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(path(&only_request(&server)), route);
    assert_eq!(page.snapshots.len(), 1);
    let snapshot = &page.snapshots["AAPL240126P00050000"];
    assert_greeks(
        snapshot.greeks.as_ref().unwrap(),
        [
            0.6742666141545315,
            0.027997890141030567,
            0.06118767991284862,
            -0.12579419566886865,
            0.1466720335488638,
        ],
    );
    assert_eq!(snapshot.implied_volatility, Some(0.3160291785379539));
    // A space is a condition code.
    assert_quote(
        snapshot.latest_quote.as_ref().unwrap(),
        "2024-02-02T20:59:59.731674112Z",
        ("X", 0.48, 396),
        ("X", 0.59, 458),
        " ",
    );
    assert_trade(
        snapshot.latest_trade.as_ref().unwrap(),
        "2024-02-02T17:29:53.55240448Z",
        "W",
        0.52,
        1,
        "I",
    );
    assert_no_bars(snapshot);
    // No token key reads as the last page.
    assert_eq!(page.next_page_token, None);

    let server = serve_fixture("test_get_snapshot_latest_quote_only", route);
    let page = client(&server)
        .option_snapshots(&query)
        .await
        .unwrap()
        .into_parts()
        .0;
    let snapshot = &page.snapshots["AAPL240126P00050000"];
    assert_quote(
        snapshot.latest_quote.as_ref().unwrap(),
        "2024-02-02T20:59:59.731674112Z",
        ("X", 0.48, 396),
        ("X", 0.59, 458),
        " ",
    );
    assert_eq!(snapshot.latest_trade, None);
    assert_eq!(snapshot.greeks, None);
    assert_eq!(snapshot.implied_volatility, None);
    assert_no_bars(snapshot);

    let server = serve_fixture("test_get_snapshot_multi_empty_response", route);
    let page = client(&server)
        .option_snapshots(&query)
        .await
        .unwrap()
        .into_parts()
        .0;
    assert!(page.snapshots.is_empty());
    assert_eq!(page.next_page_token, None);

    // A page without its snapshots map fails rather than reading as empty.
    let server = serve("200 OK", br#"{"next_page_token":null}"#);
    let error = client(&server).option_snapshots(&query).await.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Decode);

    let server = serve_fixture(
        "test_get_option_chain",
        "/v1beta1/options/snapshots/{symbol}",
    );
    let underlying: Symbol = "AAPL".parse().unwrap();
    let page = client(&server)
        .option_chain(&underlying, &ChainQuery::new())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(
        path(&only_request(&server)),
        "/v1beta1/options/snapshots/AAPL"
    );
    assert_eq!(page.snapshots.len(), 1);
    let snapshot = &page.snapshots["AAPL240503P00155000"];
    assert_greeks(
        snapshot.greeks.as_ref().unwrap(),
        [
            -0.06962352623019707,
            0.012407748296130911,
            -0.00233165154510125,
            -0.10116067021921063,
            0.03143087569879542,
        ],
    );
    assert_eq!(snapshot.implied_volatility, Some(0.4584478444465036));
    // Exchange codes kept in their case.
    assert_quote(
        snapshot.latest_quote.as_ref().unwrap(),
        "2024-04-25T20:00:00.259610112Z",
        ("N", 0.32, 73),
        ("W", 0.35, 119),
        "A",
    );
    assert_trade(
        snapshot.latest_trade.as_ref().unwrap(),
        "2024-04-25T19:58:16.034908416Z",
        "N",
        0.34,
        1,
        "I",
    );
    assert_no_bars(snapshot);
    assert_eq!(page.next_page_token, None);
}

#[tokio::test]
async fn a_snapshot_with_every_member_and_unknown_keys_is_read() {
    let bar = |t: &str, extra: &str| {
        format!(r#"{{"t":"{t}","o":1.5,"h":2,"l":1,"c":1.75,"v":7,"n":3,"vw":1.6{extra}}}"#)
    };
    let body = format!(
        r#"{{"snapshots":{{"A":{{"latestTrade":{{"t":"2024-02-02T17:29:53Z","x":"w","p":0.5,"s":2,"c":"I","new":1}},"latestQuote":{{"t":"2024-02-02T17:29:54Z","bx":"w","bp":0.4,"bs":3,"ax":"W","ap":0.6,"as":4,"c":"A","new":[]}},"minuteBar":{},"dailyBar":{},"prevDailyBar":{},"greeks":{{"delta":0.1,"gamma":0.2,"theta":-0.3,"vega":0.4,"rho":0.5,"new":null}},"impliedVolatility":0.25,"new":{{}}}}}},"next_page_token":"next","new":"x"}}"#,
        bar("2024-02-02T17:29:00Z", r#","new":true"#),
        bar("2024-02-02T05:00:00Z", ""),
        bar("2024-02-01T05:00:00Z", ""),
    );
    let server = serve("200 OK", body.as_bytes());
    let page = client(&server)
        .option_snapshots(&SnapshotsQuery::new(symbols(&["A"])))
        .await
        .unwrap()
        .into_parts()
        .0;
    let snapshot = &page.snapshots["A"];
    assert_trade(
        snapshot.latest_trade.as_ref().unwrap(),
        "2024-02-02T17:29:53Z",
        "w",
        0.5,
        2,
        "I",
    );
    assert_quote(
        snapshot.latest_quote.as_ref().unwrap(),
        "2024-02-02T17:29:54Z",
        ("w", 0.4, 3),
        ("W", 0.6, 4),
        "A",
    );
    for (bar, t) in [
        (&snapshot.minute_bar, "2024-02-02T17:29:00Z"),
        (&snapshot.daily_bar, "2024-02-02T05:00:00Z"),
        (&snapshot.previous_daily_bar, "2024-02-01T05:00:00Z"),
    ] {
        assert_bar(bar.as_ref().unwrap(), t, 1.5, 2.0, 1.0, 1.75, 7, 3, 1.6);
    }
    assert_greeks(
        snapshot.greeks.as_ref().unwrap(),
        [0.1, 0.2, 0.5, -0.3, 0.4],
    );
    assert_eq!(snapshot.implied_volatility, Some(0.25));
    assert_eq!(page.next_page_token.as_deref(), Some("next"));
}

#[tokio::test]
async fn the_documented_code_tables_are_read_code_to_name() {
    let body = br#"{"a": "SLAN - Single Leg Auction Non ISO", "e": "SLFT - Single Leg Floor Trade", "g": "MLAT - Multi Leg Auction"}"#;
    for (tick_type, segment) in [(TickType::Trade, "trade"), (TickType::Quote, "quote")] {
        let server = serve("200 OK", body);
        let codes = client(&server)
            .option_conditions(tick_type)
            .await
            .unwrap()
            .into_parts()
            .0;
        assert_eq!(codes.len(), 3);
        assert_eq!(codes["a"], "SLAN - Single Leg Auction Non ISO");
        assert_eq!(codes["e"], "SLFT - Single Leg Floor Trade");
        assert_eq!(codes["g"], "MLAT - Multi Leg Auction");
        let request = only_request(&server);
        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            format!("/v1beta1/options/meta/conditions/{segment}")
        );
    }

    let server = serve(
        "200 OK",
        br#"{"A": "NYSE American Options", "Q": "Nasdaq Options"}"#,
    );
    let codes = client(&server)
        .option_exchanges()
        .await
        .unwrap()
        .into_parts()
        .0;
    let expected: BTreeMap<String, String> =
        [("A", "NYSE American Options"), ("Q", "Nasdaq Options")]
            .into_iter()
            .map(|(code, name)| (code.to_owned(), name.to_owned()))
            .collect();
    assert_eq!(codes, expected);
    let request = only_request(&server);
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, "/v1beta1/options/meta/exchanges");
}

#[tokio::test]
async fn every_double_reads_as_the_nearest_f64() {
    for sample in SAMPLES {
        let expected: f64 = sample.parse().unwrap();
        let same = |value: f64| assert_eq!(value.to_bits(), expected.to_bits(), "{sample}");

        let trade = format!(r#"{{"t":"2024-01-03T09:00:00Z","x":"A","p":{sample},"s":1,"c":"I"}}"#);
        let quote = format!(
            r#"{{"t":"2024-01-03T09:00:00Z","bx":"A","bp":{sample},"bs":1,"ax":"A","ap":{sample},"as":1,"c":"A"}}"#
        );
        let greeks = format!(
            r#"{{"delta":{sample},"gamma":{sample},"theta":{sample},"vega":{sample},"rho":{sample}}}"#
        );
        let body = format!(
            r#"{{"snapshots":{{"A":{{"latestTrade":{trade},"latestQuote":{quote},"greeks":{greeks},"impliedVolatility":{sample}}}}}}}"#
        );
        let server = serve("200 OK", body.as_bytes());
        let page = client(&server)
            .option_snapshots(&SnapshotsQuery::new(symbols(&["A"])))
            .await
            .unwrap()
            .into_parts()
            .0;
        let snapshot = &page.snapshots["A"];
        same(snapshot.latest_trade.as_ref().unwrap().price);
        let quote = snapshot.latest_quote.as_ref().unwrap();
        same(quote.bid_price);
        same(quote.ask_price);
        let greeks = snapshot.greeks.as_ref().unwrap();
        for value in [
            greeks.delta,
            greeks.gamma,
            greeks.theta,
            greeks.vega,
            greeks.rho,
        ] {
            same(value);
        }
        same(snapshot.implied_volatility.unwrap());
    }
}

#[tokio::test]
async fn every_parameter_goes_out_as_documented() {
    let mut bars = BarsQuery::new(
        symbols(&["SPY240426C00550000", "SPY240426C00555000"]),
        Timeframe::Hours(1),
    );
    bars.start = Some(at("2024-03-12T00:00:00Z"));
    bars.end = Some(at("2024-03-13T00:00:00.5Z"));
    bars.limit = Some(500);
    bars.page_token = Some("abc".to_owned());
    bars.sort = Some(Sort::Desc);
    let server = serve("200 OK", br#"{"bars":{}}"#);
    client(&server).option_bars(&bars).await.unwrap();
    let request = only_request(&server);
    assert_eq!(request.method, "GET");
    assert_eq!(path(&request), "/v1beta1/options/bars");
    assert_eq!(
        pairs(&request),
        owned(&[
            ("symbols", "SPY240426C00550000,SPY240426C00555000"),
            ("timeframe", "1Hour"),
            ("start", "2024-03-12T00:00:00Z"),
            ("end", "2024-03-13T00:00:00.5Z"),
            ("limit", "500"),
            ("page_token", "abc"),
            ("sort", "desc"),
        ])
    );

    let mut trades = HistoryQuery::new(symbols(&["SPY240426C00550000"]));
    trades.start = Some(at("2024-03-12T00:00:00Z"));
    trades.end = Some(at("2024-03-13T00:00:00Z"));
    trades.limit = Some(5);
    trades.page_token = Some("abc".to_owned());
    trades.sort = Some(Sort::Asc);
    let server = serve("200 OK", br#"{"trades":{}}"#);
    client(&server).option_trades(&trades).await.unwrap();
    let request = only_request(&server);
    assert_eq!(path(&request), "/v1beta1/options/trades");
    assert_eq!(
        pairs(&request),
        owned(&[
            ("symbols", "SPY240426C00550000"),
            ("start", "2024-03-12T00:00:00Z"),
            ("end", "2024-03-13T00:00:00Z"),
            ("limit", "5"),
            ("page_token", "abc"),
            ("sort", "asc"),
        ])
    );

    for (feed, text) in [(Feed::Opra, "opra"), (Feed::Indicative, "indicative")] {
        assert_eq!(feed.as_str(), text);
        let mut latest = LatestQuery::new(symbols(&["A", "B"]));
        latest.feed = Some(feed);
        let server = serve("200 OK", br#"{"trades":{}}"#);
        client(&server).option_latest_trades(&latest).await.unwrap();
        let request = only_request(&server);
        assert_eq!(path(&request), "/v1beta1/options/trades/latest");
        assert_eq!(
            pairs(&request),
            owned(&[("symbols", "A,B"), ("feed", text)])
        );
        let server = serve("200 OK", br#"{"quotes":{}}"#);
        client(&server).option_latest_quotes(&latest).await.unwrap();
        let request = only_request(&server);
        assert_eq!(path(&request), "/v1beta1/options/quotes/latest");
        assert_eq!(
            pairs(&request),
            owned(&[("symbols", "A,B"), ("feed", text)])
        );
    }

    // The snapshots page puts `updated_since` before `limit`.
    let mut snapshots = SnapshotsQuery::new(symbols(&["A", "B"]));
    snapshots.feed = Some(Feed::Indicative);
    snapshots.updated_since = Some(at("2024-04-25T19:00:00.123Z"));
    snapshots.limit = Some(1000);
    snapshots.page_token = Some("abc".to_owned());
    let server = serve("200 OK", br#"{"snapshots":{}}"#);
    client(&server).option_snapshots(&snapshots).await.unwrap();
    let request = only_request(&server);
    assert_eq!(path(&request), "/v1beta1/options/snapshots");
    assert_eq!(
        pairs(&request),
        owned(&[
            ("symbols", "A,B"),
            ("feed", "indicative"),
            ("updated_since", "2024-04-25T19:00:00.123Z"),
            ("limit", "1000"),
            ("page_token", "abc"),
        ])
    );

    // The chain page puts `limit` before `updated_since`; strikes go out
    // as their exact text, beyond what a double holds.
    let mut chain = ChainQuery::new();
    chain.feed = Some(Feed::Opra);
    chain.limit = Some(1);
    chain.updated_since = Some(at("2024-04-25T19:00:00Z"));
    chain.page_token = Some("abc".to_owned());
    chain.r#type = Some(ContractType::Put);
    chain.strike_price_gte = Some("150.000000000000000001".parse().unwrap());
    chain.strike_price_lte = Some("160.5".parse().unwrap());
    chain.expiration_date = Some("2024-05-03".parse().unwrap());
    chain.expiration_date_gte = Some("2024-05-01".parse().unwrap());
    chain.expiration_date_lte = Some("2024-05-31".parse().unwrap());
    chain.root_symbol = Some("AAPL".to_owned());
    let server = serve("200 OK", br#"{"snapshots":{}}"#);
    client(&server)
        .option_chain(&"AAPL".parse().unwrap(), &chain)
        .await
        .unwrap();
    let request = only_request(&server);
    assert_eq!(path(&request), "/v1beta1/options/snapshots/AAPL");
    assert_eq!(
        pairs(&request),
        owned(&[
            ("feed", "opra"),
            ("limit", "1"),
            ("updated_since", "2024-04-25T19:00:00Z"),
            ("page_token", "abc"),
            ("type", "put"),
            ("strike_price_gte", "150.000000000000000001"),
            ("strike_price_lte", "160.5"),
            ("expiration_date", "2024-05-03"),
            ("expiration_date_gte", "2024-05-01"),
            ("expiration_date_lte", "2024-05-31"),
            ("root_symbol", "AAPL"),
        ])
    );

    // The required parameters alone: none at all on the chain.
    let server = serve("200 OK", br#"{"snapshots":{}}"#);
    client(&server)
        .option_chain(&"AAPL".parse().unwrap(), &ChainQuery::new())
        .await
        .unwrap();
    assert!(pairs(&only_request(&server)).is_empty());
    let server = serve("200 OK", br#"{"snapshots":{}}"#);
    client(&server)
        .option_snapshots(&SnapshotsQuery::new(symbols(&["A"])))
        .await
        .unwrap();
    assert_eq!(pairs(&only_request(&server)), owned(&[("symbols", "A")]));
    let server = serve("200 OK", br#"{"quotes":{}}"#);
    client(&server)
        .option_latest_quotes(&LatestQuery::new(symbols(&["A"])))
        .await
        .unwrap();
    assert_eq!(pairs(&only_request(&server)), owned(&[("symbols", "A")]));
}

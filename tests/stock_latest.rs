//! Latest stock data and snapshots: the upstream fixtures field by field, a
//! currency beside the latest values and the snapshot symbols, missing
//! members, numbers as sent, and every query parameter on the wire.

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
use stock::{SAMPLES, assert_bar, at, serve_fixture, symbols};
use suri::data::stocks::{Feed, LatestQuery, Tape, Trade};
use suri::{ErrorKind, Inbound};

fn latest_query() -> LatestQuery {
    LatestQuery::new(symbols(&["AAPL"]))
}

fn assert_latest_trade(trade: &Trade, t: &str, x: &str, p: f64, s: u32, i: u64) {
    assert_eq!(trade.timestamp, at(t));
    assert_eq!(trade.exchange, x);
    assert_eq!(trade.price, p);
    assert_eq!(trade.size, s);
    assert_eq!(trade.conditions, ["@"]);
    assert_eq!(trade.id, i);
    assert_eq!(trade.tape, Inbound::Known(Tape::C));
    assert_eq!(trade.update, None);
}

#[tokio::test]
async fn the_latest_fixtures_are_read_field_by_field() {
    const SPY: (&str, f64) = ("2022-07-26T20:50:00Z", 392.18);
    let server = serve_fixture("test_stock_latest_bar", "/v2/stocks/bars/latest");
    let latest = client(&server)
        .stock_latest_bars(&latest_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(latest.items.len(), 1);
    assert_bar(
        &latest.items["SPY"],
        SPY.0,
        SPY.1,
        SPY.1,
        SPY.1,
        SPY.1,
        2100,
        2,
        SPY.1,
    );
    assert_eq!(latest.currency, None);
    assert_eq!(path(&only_request(&server)), "/v2/stocks/bars/latest");

    let server = serve_fixture("test_multi_stock_latest_bar", "/v2/stocks/bars/latest");
    let latest = client(&server)
        .stock_latest_bars(&latest_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(latest.items.len(), 2);
    assert_bar(
        &latest.items["SPY"],
        SPY.0,
        SPY.1,
        SPY.1,
        SPY.1,
        SPY.1,
        2100,
        2,
        SPY.1,
    );
    assert_bar(
        &latest.items["TSLA"],
        "2022-07-26T19:59:00Z",
        776.605,
        776.95,
        776.41,
        776.63,
        5993,
        124,
        776.654473,
    );

    let server = serve_fixture(
        "test_stock_latest_bar_multi_empty_response",
        "/v2/stocks/bars/latest",
    );
    let latest = client(&server)
        .stock_latest_bars(&latest_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert!(latest.items.is_empty());

    let server = serve_fixture("test_get_latest_trade", "/v2/stocks/trades/latest");
    let latest = client(&server)
        .stock_latest_trades(&latest_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(latest.items.len(), 1);
    assert_latest_trade(
        &latest.items["AAPL"],
        "2022-03-18T14:02:09.722539521Z",
        "D",
        161.2958,
        100,
        22730,
    );
    assert_eq!(path(&only_request(&server)), "/v2/stocks/trades/latest");

    let server = serve_fixture(
        "test_get_multisymbol_latest_trade",
        "/v2/stocks/trades/latest",
    );
    let latest = client(&server)
        .stock_latest_trades(&latest_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(latest.items.len(), 2);
    assert_latest_trade(
        &latest.items["AAPL"],
        "2022-03-18T14:02:09.722539521Z",
        "D",
        161.2958,
        100,
        22730,
    );
    assert_latest_trade(
        &latest.items["TSLA"],
        "2022-07-15T19:59:59.405545378Z",
        "V",
        720.19,
        100,
        11017,
    );

    // A symbol with no data is absent: an empty map, status 200.
    let server = serve_fixture(
        "test_get_latest_trade_multi_not_found",
        "/v2/stocks/trades/latest",
    );
    let latest = client(&server)
        .stock_latest_trades(&latest_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert!(latest.items.is_empty());

    let server = serve_fixture("test_get_latest_quote", "/v2/stocks/quotes/latest");
    let latest = client(&server)
        .stock_latest_quotes(&latest_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    let quote = &latest.items["AAPL"];
    assert_eq!(quote.timestamp, at("2022-03-18T14:02:43.651613184Z"));
    assert_eq!(quote.ask_exchange, "P");
    assert_eq!(quote.ask_price, 161.11);
    assert_eq!(quote.ask_size, 13);
    assert_eq!(quote.bid_exchange, "K");
    assert_eq!(quote.bid_price, 161.1);
    assert_eq!(quote.bid_size, 2);
    assert_eq!(quote.conditions, ["R"]);
    assert_eq!(quote.tape, Inbound::Known(Tape::C));
    assert_eq!(path(&only_request(&server)), "/v2/stocks/quotes/latest");

    let server = serve_fixture(
        "test_get_latest_quote_multi_empty_response",
        "/v2/stocks/quotes/latest",
    );
    let latest = client(&server)
        .stock_latest_quotes(&latest_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert!(latest.items.is_empty());

    // A currency beside the latest values.
    let body = r#"{"bars":{"A":{"t":"2022-07-26T20:50:00Z","o":1,"h":1,"l":1,"c":1,"v":1,"n":1,"vw":1}},"currency":"JPY"}"#;
    let server = serve("200 OK", body.as_bytes());
    let latest = client(&server)
        .stock_latest_bars(&latest_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(latest.currency.as_deref(), Some("JPY"));
}

#[tokio::test]
async fn the_snapshot_fixtures_are_read_field_by_field() {
    let server = serve_fixture("test_get_snapshot", "/v2/stocks/snapshots");
    let snapshots = client(&server)
        .stock_snapshots(&latest_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(path(&only_request(&server)), "/v2/stocks/snapshots");
    assert_eq!(snapshots.snapshots.len(), 1);
    assert_eq!(snapshots.currency, None);
    let snapshot = &snapshots.snapshots["AAPL"];
    let trade = snapshot.latest_trade.as_ref().unwrap();
    assert_eq!(trade.timestamp, at("2022-03-18T14:33:58.448432206Z"));
    assert_eq!(trade.exchange, "D");
    assert_eq!(trade.price, 161.1998);
    assert_eq!(trade.size, 200);
    assert_eq!(trade.conditions, ["@"]);
    assert_eq!(trade.id, 39884);
    assert_eq!(trade.tape, Inbound::Known(Tape::C));
    assert_eq!(trade.update, None);
    let quote = snapshot.latest_quote.as_ref().unwrap();
    assert_eq!(quote.timestamp, at("2022-03-18T14:33:58.547942Z"));
    assert_eq!(quote.ask_exchange, "K");
    assert_eq!(quote.ask_price, 161.2);
    assert_eq!(quote.ask_size, 2);
    assert_eq!(quote.bid_exchange, "K");
    assert_eq!(quote.bid_price, 161.19);
    assert_eq!(quote.bid_size, 5);
    assert_eq!(quote.conditions, ["R"]);
    assert_eq!(quote.tape, Inbound::Known(Tape::C));
    assert_bar(
        snapshot.minute_bar.as_ref().unwrap(),
        "2022-03-18T14:32:00Z",
        161.595,
        161.63,
        161.31,
        161.365,
        195503,
        1880,
        161.448073,
    );
    assert_bar(
        snapshot.daily_bar.as_ref().unwrap(),
        "2022-03-18T04:00:00Z",
        160.59,
        161.92,
        159.76,
        161.365,
        31749988,
        186143,
        160.683364,
    );
    // An integral high in the previous daily bar.
    assert_bar(
        snapshot.previous_daily_bar.as_ref().unwrap(),
        "2022-03-17T04:00:00Z",
        158.6,
        161.0,
        157.63,
        160.62,
        73839892,
        609067,
        159.425082,
    );

    let server = serve_fixture(
        "test_get_snapshot_multi_empty_response",
        "/v2/stocks/snapshots",
    );
    let snapshots = client(&server)
        .stock_snapshots(&latest_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert!(snapshots.snapshots.is_empty());
    assert_eq!(snapshots.currency, None);
}

#[tokio::test]
async fn a_snapshot_may_lack_members_and_a_currency_sits_beside_the_symbols() {
    let quote = r#"{"t":"2022-03-18T14:33:58Z","ax":"K","ap":2,"as":1,"bx":"K","bp":1,"bs":1,"c":[],"z":"C"}"#;
    let body = format!(
        r#"{{"AAPL":{{"latestQuote":{quote}}},"currency":"USD","TSLA":{{"latestQuote":{quote},"extra":1}}}}"#
    );
    let server = serve("200 OK", body.as_bytes());
    let snapshots = client(&server)
        .stock_snapshots(&latest_query())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(snapshots.currency.as_deref(), Some("USD"));
    assert_eq!(snapshots.snapshots.len(), 2);
    for symbol in ["AAPL", "TSLA"] {
        let snapshot = &snapshots.snapshots[symbol];
        assert_eq!(snapshot.latest_quote.as_ref().unwrap().ask_price, 2.0);
        assert_eq!(snapshot.latest_trade, None);
        assert_eq!(snapshot.minute_bar, None);
        assert_eq!(snapshot.daily_bar, None);
        assert_eq!(snapshot.previous_daily_bar, None);
    }

    // A currency that is not text, and a symbol whose value is null, fail.
    for body in [r#"{"currency":1}"#, r#"{"AAPL":null}"#, r#"[]"#] {
        let server = serve("200 OK", body.as_bytes());
        let error = client(&server)
            .stock_snapshots(&latest_query())
            .await
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Decode, "{body}");
    }
}

#[tokio::test]
async fn every_double_in_a_snapshot_reads_as_the_nearest_f64() {
    for sample in SAMPLES {
        let expected: f64 = sample.parse().unwrap();
        let bar = format!(
            r#"{{"t":"2022-01-03T09:00:00Z","o":{sample},"h":{sample},"l":{sample},"c":{sample},"v":1,"n":1,"vw":{sample}}}"#
        );
        let body = format!(
            r#"{{"A":{{"latestTrade":{{"t":"2022-01-03T09:00:00Z","x":"D","p":{sample},"s":1,"c":[],"i":1,"z":"C"}},"latestQuote":{{"t":"2022-01-03T09:00:00Z","ax":"K","ap":{sample},"as":1,"bx":"Q","bp":{sample},"bs":1,"c":[],"z":"C"}},"minuteBar":{bar},"dailyBar":{bar},"prevDailyBar":{bar}}}}}"#
        );
        let server = serve("200 OK", body.as_bytes());
        let snapshots = client(&server)
            .stock_snapshots(&latest_query())
            .await
            .unwrap()
            .into_parts()
            .0;
        let snapshot = &snapshots.snapshots["A"];
        let mut values = vec![
            snapshot.latest_trade.as_ref().unwrap().price,
            snapshot.latest_quote.as_ref().unwrap().ask_price,
            snapshot.latest_quote.as_ref().unwrap().bid_price,
        ];
        for bar in [
            &snapshot.minute_bar,
            &snapshot.daily_bar,
            &snapshot.previous_daily_bar,
        ] {
            let bar = bar.as_ref().unwrap();
            values.extend([bar.open, bar.high, bar.low, bar.close, bar.vwap]);
        }
        for value in values {
            assert_eq!(value.to_bits(), expected.to_bits(), "{sample}");
        }
    }
}

#[tokio::test]
async fn every_latest_parameter_goes_out_in_the_documented_order() {
    let mut query = LatestQuery::new(symbols(&["AAPL", "TSLA"]));
    query.feed = Some(Feed::DelayedSip);
    query.currency = Some("EUR".to_owned());
    for (route, body) in [
        ("/v2/stocks/bars/latest", &br#"{"bars":{}}"#[..]),
        ("/v2/stocks/trades/latest", br#"{"trades":{}}"#),
        ("/v2/stocks/quotes/latest", br#"{"quotes":{}}"#),
        ("/v2/stocks/snapshots", br#"{}"#),
    ] {
        for (query, expected) in [
            (
                query.clone(),
                owned(&[
                    ("symbols", "AAPL,TSLA"),
                    ("feed", "delayed_sip"),
                    ("currency", "EUR"),
                ]),
            ),
            (
                LatestQuery::new(symbols(&["AAPL"])),
                owned(&[("symbols", "AAPL")]),
            ),
        ] {
            let server = serve("200 OK", body);
            let client = client(&server);
            match route {
                "/v2/stocks/bars/latest" => drop(client.stock_latest_bars(&query).await.unwrap()),
                "/v2/stocks/trades/latest" => {
                    drop(client.stock_latest_trades(&query).await.unwrap())
                }
                "/v2/stocks/quotes/latest" => {
                    drop(client.stock_latest_quotes(&query).await.unwrap())
                }
                _ => drop(client.stock_snapshots(&query).await.unwrap()),
            }
            let request = only_request(&server);
            assert_eq!(request.method, "GET");
            assert_eq!(path(&request), route);
            assert_eq!(pairs(&request), expected, "{route}");
        }
    }
}

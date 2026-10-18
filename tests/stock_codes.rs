//! Stock codes: the condition and exchange tables (documented examples,
//! path and query on the wire, a table that is not text), and every feed
//! and adjustment's wire text.

#![cfg(feature = "rest")]

#[path = "support/requests.rs"]
mod requests;
#[path = "support/server.rs"]
mod server;

use std::collections::BTreeMap;

use requests::{client, only_request, owned, pairs, path, serve};
use suri::ErrorKind;
use suri::data::stocks::{Adjustment, BarsQuery, ConditionsQuery, Feed, Tape};
use suri::data::{TickType, Timeframe};

fn table(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(code, name)| ((*code).to_owned(), (*name).to_owned()))
        .collect()
}

#[test]
fn each_tick_type_has_its_documented_text() {
    for (tick_type, text) in [(TickType::Trade, "trade"), (TickType::Quote, "quote")] {
        assert_eq!(tick_type.as_str(), text);
        assert_eq!(tick_type.to_string(), text);
        assert_eq!(text.parse::<TickType>(), Ok(tick_type));
    }
}

#[tokio::test]
async fn the_documented_condition_table_is_read_code_to_name() {
    let body = br#"{"@": "Regular Sale", "A": "Acquisition", "B": "Bunched Trade"}"#;
    for (tick_type, segment) in [(TickType::Trade, "trade"), (TickType::Quote, "quote")] {
        let server = serve("200 OK", body);
        let codes = client(&server)
            .stock_conditions(tick_type, &ConditionsQuery::new(Tape::C))
            .await
            .unwrap()
            .into_parts()
            .0;
        assert_eq!(
            codes,
            table(&[
                ("@", "Regular Sale"),
                ("A", "Acquisition"),
                ("B", "Bunched Trade"),
            ])
        );
        let request = only_request(&server);
        assert_eq!(request.method, "GET");
        assert_eq!(
            path(&request),
            format!("/v2/stocks/meta/conditions/{segment}")
        );
        assert_eq!(pairs(&request), owned(&[("tape", "C")]));
    }

    // A tape the conditions page does not list is sent as given.
    let server = serve("200 OK", b"{}");
    client(&server)
        .stock_conditions(TickType::Trade, &ConditionsQuery::new(Tape::N))
        .await
        .unwrap();
    assert_eq!(pairs(&only_request(&server)), owned(&[("tape", "N")]));
}

#[tokio::test]
async fn the_documented_exchange_table_is_read_code_to_name() {
    let server = serve("200 OK", br#"{"N": "New York Stock Exchange", "V": "IEX"}"#);
    let codes = client(&server)
        .stock_exchanges()
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(
        codes,
        table(&[("N", "New York Stock Exchange"), ("V", "IEX")])
    );
    let request = only_request(&server);
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, "/v2/stocks/meta/exchanges");
}

#[tokio::test]
async fn a_table_that_is_not_text_fails_the_call() {
    for body in [&br#"{"N": 1}"#[..], br#"{"N": null}"#, br#"[]"#] {
        let server = serve("200 OK", body);
        let error = client(&server).stock_exchanges().await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Decode);
    }
}

const FEEDS: [(Feed, &str); 6] = [
    (Feed::Sip, "sip"),
    (Feed::Iex, "iex"),
    (Feed::DelayedSip, "delayed_sip"),
    (Feed::Boats, "boats"),
    (Feed::Overnight, "overnight"),
    (Feed::Otc, "otc"),
];

const ADJUSTMENTS: [(Adjustment, &str); 5] = [
    (Adjustment::Raw, "raw"),
    (Adjustment::Split, "split"),
    (Adjustment::Dividend, "dividend"),
    (Adjustment::SpinOff, "spin-off"),
    (Adjustment::All, "all"),
];

#[test]
fn each_feed_and_adjustment_has_its_documented_text() {
    for (feed, text) in FEEDS {
        assert_eq!(feed.as_str(), text);
        assert_eq!(feed.to_string(), text);
        assert_eq!(text.parse::<Feed>(), Ok(feed));
    }
    for (adjustment, text) in ADJUSTMENTS {
        assert_eq!(adjustment.as_str(), text);
        assert_eq!(adjustment.to_string(), text);
        assert_eq!(text.parse::<Adjustment>(), Ok(adjustment));
    }
}

#[tokio::test]
async fn each_feed_and_adjustment_goes_out_as_its_text() {
    for (feed, text) in FEEDS {
        let mut query = BarsQuery::new(vec!["AAPL".to_owned()], Timeframe::Day);
        query.feed = Some(feed);
        let server = serve("200 OK", br#"{"bars":{}}"#);
        client(&server).stock_bars(&query).await.unwrap();
        assert_eq!(
            pairs(&only_request(&server)),
            owned(&[("symbols", "AAPL"), ("timeframe", "1Day"), ("feed", text)])
        );
    }
    // Every adjustment, comma-joined in the order given.
    let mut query = BarsQuery::new(vec!["AAPL".to_owned()], Timeframe::Day);
    query.adjustment = Some(
        ADJUSTMENTS
            .iter()
            .map(|(adjustment, _)| *adjustment)
            .collect(),
    );
    let server = serve("200 OK", br#"{"bars":{}}"#);
    client(&server).stock_bars(&query).await.unwrap();
    assert_eq!(
        pairs(&only_request(&server)),
        owned(&[
            ("symbols", "AAPL"),
            ("timeframe", "1Day"),
            ("adjustment", "raw,split,dividend,spin-off,all"),
        ])
    );
}

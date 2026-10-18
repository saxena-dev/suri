//! Stock condition and exchange codes: the documented tables, the path and
//! query on the wire, and a table that is not text.

#![cfg(feature = "rest")]

#[path = "support/requests.rs"]
mod requests;
#[path = "support/server.rs"]
mod server;

use std::collections::BTreeMap;

use requests::{client, only_request, owned, pairs, path, serve};
use suri::ErrorKind;
use suri::data::TickType;
use suri::data::stocks::{ConditionsQuery, Tape};

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

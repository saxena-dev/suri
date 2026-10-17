//! Watchlists: the documented shapes field by field, each request on the
//! wire, and mutations made exactly once.

#![cfg(feature = "rest")]

use std::time::Duration;

#[path = "support/fixture.rs"]
mod fixture;
#[path = "support/server.rs"]
mod server;

use server::{Request, Server, Step, response};
use suri::{
    Asset, AssetClass, AssetStatus, Client, Config, CreateWatchlist, Credentials, Environment,
    ErrorKind, Exchange, Id, Inbound, Region, Symbol, Timestamp, Watchlist, WatchlistUpdate,
};

const BROKER_LIST: &str = "broker/broker_client/test_watchlist_routes::test_get_watchlists_for_account::GET /v1/trading/accounts/{account_id}/watchlists#0";
const BROKER_BY_ID: &str = "broker/broker_client/test_watchlist_routes::test_get_watchlist_for_account_by_id::GET /v1/trading/accounts/{account_id}/watchlists/{watchlist_id}#0";

const WATCHLIST_ID: &str = "3174d6df-7726-44b4-a5bd-7fda5ae6e009";

// The watchlist schema's example, with an unknown field.
const WITH_ASSETS: &str = r#"{
  "account_id":"abe25343-a7ba-4255-bdeb-f7e013e9ee5d",
  "assets":[{"class":"us_equity","easy_to_borrow":true,"exchange":"NASDAQ","fractionable":true,
    "id":"8ccae427-5dd0-45b3-b5fe-7ba5e422c766","marginable":true,"name":"Tesla, Inc. Common Stock",
    "shortable":true,"status":"active","symbol":"TSLA","tradable":true}],
  "created_at":"2022-01-31T21:49:05.14628Z",
  "id":"3174d6df-7726-44b4-a5bd-7fda5ae6e009",
  "name":"Primary Watchlist",
  "updated_at":"2022-01-31T21:49:06.5Z",
  "unknown_member":{"a":1}
}"#;

// The list endpoint's example.
const LIST: &str = r#"[{"account_id":"abe25343-a7ba-4255-bdeb-f7e013e9ee5d","created_at":"2022-01-31T21:49:05.14628Z","id":"3174d6df-7726-44b4-a5bd-7fda5ae6e009","name":"Primary Watchlist","updated_at":"2022-01-31T21:49:05.14628Z"}]"#;

fn config(server: &Server) -> Config {
    Config::new(
        Region::US,
        Environment::Paper,
        Credentials::api_key("PKSENTINELKEY0001", "SECRETSENTINELVALUE0001"),
    )
    .with_base_url(&server.url)
}

fn client(server: &Server) -> Client {
    Client::new(config(server).with_max_read_attempts(1)).unwrap()
}

/// A client with the default four attempts, so a read in the same
/// situation would reach a server's second step.
fn retrying_client(server: &Server) -> Client {
    Client::new(config(server).with_retry_base(Duration::from_millis(1))).unwrap()
}

fn at(text: &str) -> Timestamp {
    text.parse().unwrap()
}

fn watchlist_id() -> Id {
    WATCHLIST_ID.parse().unwrap()
}

fn serve(status: &str, body: &[u8]) -> Server {
    Server::start(vec![Step::Respond(response(status, &[], body))])
}

fn only_request(server: &Server) -> Request {
    let mut requests = server.requests();
    assert_eq!(requests.len(), 1);
    requests.remove(0)
}

fn assert_no_body(request: &Request) {
    assert_eq!(request.header("Content-Type"), None);
    assert!(request.body.is_empty());
}

/// Asserts a request's method, path and exact JSON body.
fn assert_sent(request: &Request, method: &str, path: &str, body: &str) {
    assert_eq!(request.method, method);
    assert_eq!(request.path, path);
    assert_eq!(request.header("Content-Type"), Some("application/json"));
    assert_eq!(String::from_utf8(request.body.clone()).unwrap(), body);
}

/// Asserts every field of a listed US equity as the examples and fixtures
/// give it: active on NASDAQ, every flag true, and none of the optional
/// fields sent.
#[allow(deprecated)]
fn assert_asset(asset: &Asset, id: &str, symbol: &str, name: &str) {
    assert_eq!(asset.id, id);
    assert_eq!(asset.symbol, symbol);
    assert_eq!(asset.name, name);
    assert_eq!(asset.class, Inbound::Known(AssetClass::UsEquity));
    assert_eq!(asset.exchange, Inbound::Known(Exchange::Nasdaq));
    assert_eq!(asset.status, Inbound::Known(AssetStatus::Active));
    assert!(asset.tradable);
    assert!(asset.marginable);
    assert!(asset.shortable);
    assert!(asset.fractionable);
    assert_eq!(asset.attributes, None);
    assert_eq!(asset.borrow_status, None);
    assert_eq!(asset.cusip, None);
    assert_eq!(asset.margin_requirement_long, None);
    assert_eq!(asset.margin_requirement_short, None);
    assert_eq!(asset.min_order_size, None);
    assert_eq!(asset.min_trade_increment, None);
    assert_eq!(asset.price_increment, None);
    assert_eq!(asset.maintenance_margin_requirement, None);
}

fn assert_example_fields(watchlist: &Watchlist) {
    assert_eq!(watchlist.id, WATCHLIST_ID);
    assert_eq!(watchlist.account_id, "abe25343-a7ba-4255-bdeb-f7e013e9ee5d");
    assert_eq!(watchlist.name, "Primary Watchlist");
    assert_eq!(watchlist.created_at, at("2022-01-31T21:49:05.14628Z"));
}

#[tokio::test]
async fn a_watchlist_decodes_with_its_assets() {
    let server = serve("200 OK", WITH_ASSETS.as_bytes());
    let watchlist = client(&server)
        .watchlist(&watchlist_id())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_example_fields(&watchlist);
    assert_eq!(watchlist.updated_at, at("2022-01-31T21:49:06.5Z"));
    let assets = watchlist.assets.unwrap();
    assert_eq!(assets.len(), 1);
    assert_asset(
        &assets[0],
        "8ccae427-5dd0-45b3-b5fe-7ba5e422c766",
        "TSLA",
        "Tesla, Inc. Common Stock",
    );
    let request = only_request(&server);
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, format!("/v2/watchlists/{WATCHLIST_ID}"));
    assert_no_body(&request);
}

#[tokio::test]
async fn the_list_decodes_without_assets() {
    let server = serve("200 OK", LIST.as_bytes());
    let list = client(&server).watchlists().await.unwrap().into_parts().0;
    assert_eq!(list.len(), 1);
    assert_example_fields(&list[0]);
    assert_eq!(list[0].updated_at, at("2022-01-31T21:49:05.14628Z"));
    assert_eq!(list[0].assets, None);
    let request = only_request(&server);
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, "/v2/watchlists");
    assert_no_body(&request);
}

#[tokio::test]
async fn an_empty_watchlist_has_an_empty_asset_list() {
    let body = WITH_ASSETS.replace(
        &WITH_ASSETS
            [WITH_ASSETS.find("\"assets\"").unwrap()..WITH_ASSETS.find("\"created_at\"").unwrap()],
        "\"assets\":[],\n  ",
    );
    let server = serve("200 OK", body.as_bytes());
    let watchlist = client(&server)
        .watchlist(&watchlist_id())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(watchlist.assets, Some(vec![]));
}

#[tokio::test]
async fn a_watchlist_without_a_required_field_fails() {
    for field in [
        r#""account_id":"abe25343-a7ba-4255-bdeb-f7e013e9ee5d","#,
        r#""name":"Primary Watchlist","#,
    ] {
        assert_eq!(WITH_ASSETS.matches(field).count(), 1, "{field}");
        let server = serve("200 OK", WITH_ASSETS.replace(field, "").as_bytes());
        let error = client(&server)
            .watchlist(&watchlist_id())
            .await
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Decode, "{field}");
    }
}

#[tokio::test]
async fn the_broker_watchlists_have_the_same_shape() {
    let server = serve("200 OK", &fixture::upstream(BROKER_LIST).body);
    let list = client(&server).watchlists().await.unwrap().into_parts().0;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "7c3ac77f-894c-4c08-987f-18a2e43e8e2a");
    assert_eq!(list[0].account_id, "0d969814-40d6-4b2b-99ac-2e37427f1ad2");
    assert_eq!(list[0].name, "test watchlist");
    assert_eq!(list[0].created_at, at("2022-05-20T16:17:30.159561Z"));
    assert_eq!(list[0].updated_at, at("2022-05-20T16:17:30.159561Z"));
    assert_eq!(list[0].assets, None);

    let server = serve("200 OK", &fixture::upstream(BROKER_BY_ID).body);
    let watchlist = client(&server)
        .watchlist(&"7c3ac77f-894c-4c08-987f-18a2e43e8e2a".parse().unwrap())
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_eq!(watchlist.id, "7c3ac77f-894c-4c08-987f-18a2e43e8e2a");
    assert_eq!(watchlist.account_id, "0d969814-40d6-4b2b-99ac-2e37427f1ad2");
    assert_eq!(watchlist.name, "test watchlist");
    assert_eq!(watchlist.created_at, at("2022-05-20T16:17:30.159561Z"));
    assert_eq!(watchlist.updated_at, at("2022-05-20T16:17:30.159561Z"));
    let assets = watchlist.assets.unwrap();
    assert_eq!(assets.len(), 1);
    assert_asset(
        &assets[0],
        "93f58d0b-6c53-432d-b8ce-2bad264dbd94",
        "AAPL",
        "Apple Inc. Common Stock",
    );
}

#[tokio::test]
async fn a_watchlist_is_created_with_its_name_and_symbols() {
    let mut with_symbols = CreateWatchlist::new("Tech Stocks");
    with_symbols.symbols = Some(vec!["AAPL".to_owned(), "TSLA".to_owned()]);
    for (create, body) in [
        (
            CreateWatchlist::new("Tech Stocks"),
            r#"{"name":"Tech Stocks"}"#,
        ),
        (
            with_symbols,
            r#"{"name":"Tech Stocks","symbols":["AAPL","TSLA"]}"#,
        ),
    ] {
        let server = serve("200 OK", WITH_ASSETS.as_bytes());
        let watchlist = client(&server)
            .create_watchlist(&create)
            .await
            .unwrap()
            .into_parts()
            .0;
        assert_example_fields(&watchlist);
        assert_sent(&only_request(&server), "POST", "/v2/watchlists", body);
    }
}

#[tokio::test]
async fn an_update_sends_what_it_changes() {
    for (update, body) in [
        (
            WatchlistUpdate::Name("Renamed".to_owned()),
            r#"{"name":"Renamed"}"#,
        ),
        // An empty list is sent: it empties the watchlist.
        (WatchlistUpdate::Symbols(vec![]), r#"{"symbols":[]}"#),
        (
            WatchlistUpdate::NameAndSymbols {
                name: "Renamed".to_owned(),
                symbols: vec!["AAPL".to_owned()],
            },
            r#"{"name":"Renamed","symbols":["AAPL"]}"#,
        ),
    ] {
        let server = serve("200 OK", WITH_ASSETS.as_bytes());
        client(&server)
            .update_watchlist(&watchlist_id(), &update)
            .await
            .unwrap();
        let path = format!("/v2/watchlists/{WATCHLIST_ID}");
        assert_sent(&only_request(&server), "PUT", &path, body);
        assert_eq!(serde_json::to_string(&update).unwrap(), body);
    }
}

#[tokio::test]
async fn an_asset_is_added_and_removed() {
    let server = serve("200 OK", WITH_ASSETS.as_bytes());
    client(&server)
        .add_to_watchlist(&watchlist_id(), "BTC/USD")
        .await
        .unwrap();
    let path = format!("/v2/watchlists/{WATCHLIST_ID}");
    assert_sent(
        &only_request(&server),
        "POST",
        &path,
        r#"{"symbol":"BTC/USD"}"#,
    );

    for (symbol, segment) in [("AAPL", "AAPL"), ("BTC/USD", "BTC%2FUSD")] {
        let server = serve("200 OK", WITH_ASSETS.as_bytes());
        client(&server)
            .remove_from_watchlist(&watchlist_id(), &symbol.parse::<Symbol>().unwrap())
            .await
            .unwrap();
        let request = only_request(&server);
        assert_eq!(request.method, "DELETE");
        assert_eq!(
            request.path,
            format!("/v2/watchlists/{WATCHLIST_ID}/{segment}")
        );
        assert_no_body(&request);
    }
}

#[tokio::test]
async fn a_watchlist_is_deleted() {
    let server = serve("204 No Content", b"");
    let response = client(&server)
        .delete_watchlist(&watchlist_id())
        .await
        .unwrap();
    assert_eq!(response.meta().status(), 204);
    let request = only_request(&server);
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.path, format!("/v2/watchlists/{WATCHLIST_ID}"));
    assert_no_body(&request);
}

/// A server that fails the first request with a 503 and keeps a success for
/// a second one, which a retry would reach.
fn failing_once(spare: &[u8]) -> Server {
    Server::start(vec![
        Step::Respond(response("503 Service Unavailable", &[], b"")),
        Step::Respond(response("200 OK", &[], spare)),
    ])
}

#[tokio::test]
async fn each_mutation_is_made_once() {
    let id = watchlist_id();
    let spare = WITH_ASSETS.as_bytes();

    let server = failing_once(spare);
    let error = retrying_client(&server)
        .create_watchlist(&CreateWatchlist::new("Tech Stocks"))
        .await
        .unwrap_err();
    assert_eq!(error.status(), Some(503));
    assert_eq!(server.requests().len(), 1);

    let server = failing_once(spare);
    let error = retrying_client(&server)
        .update_watchlist(&id, &WatchlistUpdate::Symbols(vec![]))
        .await
        .unwrap_err();
    assert_eq!(error.status(), Some(503));
    assert_eq!(server.requests().len(), 1);

    let server = failing_once(spare);
    let error = retrying_client(&server)
        .add_to_watchlist(&id, "AAPL")
        .await
        .unwrap_err();
    assert_eq!(error.status(), Some(503));
    assert_eq!(server.requests().len(), 1);

    let server = failing_once(spare);
    let error = retrying_client(&server)
        .remove_from_watchlist(&id, &"AAPL".parse().unwrap())
        .await
        .unwrap_err();
    assert_eq!(error.status(), Some(503));
    assert_eq!(server.requests().len(), 1);

    let server = failing_once(b"");
    let error = retrying_client(&server)
        .delete_watchlist(&id)
        .await
        .unwrap_err();
    assert_eq!(error.status(), Some(503));
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn a_read_in_the_same_situation_is_retried() {
    let server = failing_once(LIST.as_bytes());
    retrying_client(&server).watchlists().await.unwrap();
    assert_eq!(server.requests().len(), 2);
}

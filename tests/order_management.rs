//! Finding, replacing and cancelling orders: the queries and bodies on the
//! wire, empty and per-order responses, and mutations made exactly once.

#![cfg(feature = "rest")]

#[path = "support/fixture.rs"]
mod fixture;
#[path = "support/order.rs"]
mod order;
#[path = "support/server.rs"]
mod server;

use order::{AAPL_MARKET_BUY, assert_fixture_order};
use server::{Request, Server, Step, response};
use suri::{
    AssetClass, CancelResult, Client, Config, Credentials, Decimal, Environment, ErrorKind, Id,
    OrderQueryStatus, OrderSide, OrderWindow, OrdersQuery, Region, ReplaceOrderRequest, Sort,
    Stage, TimeInForce,
};

const ORDERS: &str = "trading/trading_client/test_order_routes::test_get_orders::GET /v2/orders#0";
const BY_ID: &str =
    "trading/trading_client/test_order_routes::test_get_order_by_id::GET /v2/orders/{order_id}#0";
const REPLACE: &str =
    "trading/trading_client/test_order_routes::test_replace_order::PATCH /v2/orders/{order_id}#0";
const CANCEL: &str = "trading/trading_client/test_order_routes::test_cancel_order_by_id::DELETE /v2/orders/{order_id}#0";
const UNCANCELABLE: &str = "trading/trading_client/test_order_routes::test_cancel_order_throws_uncancelable_error::DELETE /v2/orders/{order_id}#0";
const NOT_FOUND: &str = "trading/trading_client/test_order_routes::test_cancel_order_throws_not_found_error::DELETE /v2/orders/{order_id}#0";
const CANCEL_ALL: &str =
    "trading/trading_client/test_order_routes::test_cancel_orders::DELETE /v2/orders#0";
const PLACED: &str =
    "trading/trading_client/test_order_routes::test_market_order::POST /v2/orders#0";

const ORDER_ID: &str = "61e69015-8549-4bfd-b9c3-01e75843f47d";
const SECRET: &str = "SECRETSENTINELVALUE0001";

fn client(server: &Server) -> Client {
    let config = Config::new(
        Region::US,
        Environment::Paper,
        Credentials::api_key("PKSENTINELKEY0001", SECRET),
    )
    .with_base_url(&server.url)
    .with_max_read_attempts(1);
    Client::new(config).unwrap()
}

fn dec(text: &str) -> Decimal {
    text.parse().unwrap()
}

fn order_id() -> Id {
    ORDER_ID.parse().unwrap()
}

/// A server that answers once with this status and body.
fn serve(status: &str, body: &[u8]) -> Server {
    Server::start(vec![Step::Respond(response(status, &[], body))])
}

/// The response of an upstream fixture, with its recorded status.
fn serve_fixture(id: &str) -> Server {
    let fixture = fixture::upstream(id);
    let status = match fixture.status {
        200 => "200 OK",
        204 => "204 No Content",
        207 => "207 Multi-Status",
        404 => "404 Not Found",
        422 => "422 Unprocessable Entity",
        other => panic!("no status line for {other}"),
    };
    serve(status, &fixture.body)
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

#[tokio::test]
async fn the_order_list_fixture_fails_on_its_placeholder() {
    // Its "hwm": "string" is not a decimal.
    let server = serve_fixture(ORDERS);
    let error = client(&server)
        .orders(&OrdersQuery::new())
        .await
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Decode);
    assert_eq!(error.stage(), Stage::ResponseReceived);
    assert_eq!(error.status(), Some(200));
    let request = only_request(&server);
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, "/v2/orders");
    assert_no_body(&request);
}

/// Lists orders against a one-order list and returns the request's path.
async fn listed(query: &OrdersQuery) -> String {
    let body = format!(
        "[{}]",
        String::from_utf8(fixture::upstream(PLACED).body).unwrap()
    );
    let server = serve("200 OK", body.as_bytes());
    let list = client(&server).orders(query).await.unwrap().into_parts().0;
    assert_eq!(list.len(), 1);
    assert_fixture_order(&list[0], &AAPL_MARKET_BUY);
    only_request(&server).path
}

fn full_query(window: OrderWindow) -> OrdersQuery {
    let mut query = OrdersQuery::new();
    query.status = Some(OrderQueryStatus::All);
    query.limit = Some(500);
    query.window = Some(window);
    query.direction = Some(Sort::Asc);
    query.nested = Some(true);
    query.symbols = Some(vec!["AAPL".to_owned(), "BTC/USD".to_owned()]);
    query.side = Some(OrderSide::Sell);
    query.asset_classes = Some(vec![AssetClass::UsEquity, AssetClass::Crypto]);
    query
}

#[tokio::test]
async fn every_list_parameter_goes_on_the_wire_in_order() {
    let time = OrderWindow::Time {
        after: Some("2024-01-01T12:30:00.5Z".parse().unwrap()),
        until: Some("2024-01-03T00:00:00Z".parse().unwrap()),
    };
    assert_eq!(
        listed(&full_query(time)).await,
        "/v2/orders?status=all&limit=500&after=2024-01-01T12%3A30%3A00.5Z\
         &until=2024-01-03T00%3A00%3A00Z&direction=asc&nested=true\
         &symbols=AAPL%2CBTC%2FUSD&side=sell&asset_class=us_equity%2Ccrypto"
    );
    assert_eq!(
        listed(&full_query(OrderWindow::BeforeOrder(order_id()))).await,
        format!(
            "/v2/orders?status=all&limit=500&direction=asc&nested=true\
             &symbols=AAPL%2CBTC%2FUSD&side=sell&asset_class=us_equity%2Ccrypto\
             &before_order_id={ORDER_ID}"
        )
    );
    assert_eq!(
        listed(&full_query(OrderWindow::AfterOrder(order_id()))).await,
        format!(
            "/v2/orders?status=all&limit=500&direction=asc&nested=true\
             &symbols=AAPL%2CBTC%2FUSD&side=sell&asset_class=us_equity%2Ccrypto\
             &after_order_id={ORDER_ID}"
        )
    );
}

#[tokio::test]
async fn a_time_window_sends_only_its_set_bounds() {
    let mut query = OrdersQuery::new();
    query.window = Some(OrderWindow::Time {
        after: None,
        until: Some("2024-01-03T00:00:00Z".parse().unwrap()),
    });
    assert_eq!(
        listed(&query).await,
        "/v2/orders?until=2024-01-03T00%3A00%3A00Z"
    );
    assert_eq!(listed(&OrdersQuery::new()).await, "/v2/orders");
}

#[tokio::test]
async fn an_order_is_read_by_its_id() {
    for (nested, path) in [
        (false, format!("/v2/orders/{ORDER_ID}")),
        (true, format!("/v2/orders/{ORDER_ID}?nested=true")),
    ] {
        let server = serve_fixture(BY_ID);
        let order = client(&server)
            .order(&order_id(), nested)
            .await
            .unwrap()
            .into_parts()
            .0;
        assert_fixture_order(&order, &AAPL_MARKET_BUY);
        let request = only_request(&server);
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, path);
        assert_no_body(&request);
    }
}

/// Replaces the order against the replace fixture and returns the body sent.
async fn replaced(replace: &ReplaceOrderRequest) -> String {
    let server = serve_fixture(REPLACE);
    let order = client(&server)
        .replace_order(&order_id(), replace)
        .await
        .unwrap()
        .into_parts()
        .0;
    // The fixture answers with the replaced order's own ID.
    assert_fixture_order(&order, &AAPL_MARKET_BUY);
    let request = only_request(&server);
    assert_eq!(request.method, "PATCH");
    assert_eq!(request.path, format!("/v2/orders/{ORDER_ID}"));
    assert_eq!(request.header("Content-Type"), Some("application/json"));
    let body = String::from_utf8(request.body).unwrap();
    assert_eq!(serde_json::to_string(replace).unwrap(), body);
    body
}

#[tokio::test]
async fn a_replace_sends_only_what_is_set() {
    let mut replace = ReplaceOrderRequest::new();
    replace.limit_price = Some(dec("101.25"));
    replace.client_order_id = "replacement-1".parse().unwrap();
    assert_eq!(
        replaced(&replace).await,
        r#"{"limit_price":"101.25","client_order_id":"replacement-1"}"#
    );
}

#[tokio::test]
async fn every_replace_field_is_sent_in_order() {
    let mut replace = ReplaceOrderRequest::new();
    replace.qty = Some(dec("1234567890.1234567890123456789"));
    replace.time_in_force = Some(TimeInForce::Gtc);
    replace.limit_price = Some(dec("101.25"));
    replace.stop_price = Some(dec("99.5"));
    replace.trail = Some(dec("1.5"));
    replace.client_order_id = "replacement-1".parse().unwrap();
    assert_eq!(
        replaced(&replace).await,
        r#"{"qty":"1234567890.1234567890123456789","time_in_force":"gtc","limit_price":"101.25","stop_price":"99.5","trail":"1.5","client_order_id":"replacement-1"}"#
    );
}

#[test]
fn each_replace_has_its_own_generated_id() {
    let first = ReplaceOrderRequest::new();
    let second = ReplaceOrderRequest::default();
    assert_ne!(first.client_order_id, second.client_order_id);
    assert_eq!(first.client_order_id.as_str().len(), 36);
}

#[tokio::test]
async fn a_cancel_has_no_result_but_success() {
    let server = serve_fixture(CANCEL);
    let response = client(&server).cancel_order(&order_id()).await.unwrap();
    assert_eq!(response.meta().status(), 204);
    let request = only_request(&server);
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.path, format!("/v2/orders/{ORDER_ID}"));
    assert_no_body(&request);
}

#[tokio::test]
async fn an_empty_success_is_read_and_dropped_with_or_without_a_body() {
    for (status, body) in [
        ("200 OK", &b""[..]),
        ("200 OK", br#"{"unexpected":true}"#),
        ("204 No Content", b""),
        ("204 No Content", b"not json"),
    ] {
        let server = serve(status, body);
        let response = client(&server).cancel_order(&order_id()).await.unwrap();
        assert_eq!(
            response.meta().status(),
            status[..3].parse::<u16>().unwrap()
        );
    }
}

#[tokio::test]
async fn a_cancel_alpaca_refuses_is_an_api_error() {
    for (id, status) in [(UNCANCELABLE, 422), (NOT_FOUND, 404)] {
        let server = serve_fixture(id);
        let error = client(&server).cancel_order(&order_id()).await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Api);
        assert_eq!(error.status(), Some(status));
        assert_eq!(error.code(), None);
        assert_eq!(error.message(), None);
        assert_eq!(error.client_order_id(), None);
    }
}

async fn cancel_all(server: &Server) -> Vec<CancelResult> {
    let results = client(server)
        .cancel_all_orders()
        .await
        .unwrap()
        .into_parts()
        .0;
    let request = only_request(server);
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.path, "/v2/orders");
    assert_no_body(&request);
    results
}

#[tokio::test]
async fn cancel_all_reports_each_order() {
    let results = cancel_all(&serve_fixture(CANCEL_ALL)).await;
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "497f6eca-6276-4993-bfeb-53cbbbba6f08");
    assert_eq!(results[0].status, 200);
    assert_eq!(results[0].code, None);
    assert_eq!(results[0].message, None);
    assert_eq!(results[0].body, None);
    assert_eq!(results[1].id, "72249bb6-6c89-4ea7-b8cf-73f1a140812b");
    assert_eq!(results[1].status, 404);
    assert_eq!(results[1].code, Some(40410000));
    assert_eq!(results[1].message.as_deref(), Some("order not found"));
    assert_eq!(results[1].body, None);
}

#[tokio::test]
async fn each_kept_text_is_scrubbed_and_bounded() {
    let long = "x".repeat(2000);
    let body = format!(
        r#"[
          {{"id":"a","status":200,"body":{{"note":"{SECRET}"}},"unknown_member":1}},
          {{"id":"b","status":422,"body":"cannot cancel {SECRET}"}},
          {{"id":"c","status":500,"body":{{"detail":"{long}"}}}},
          {{"id":"d","status":204,"body":null}},
          {{"id":"e","status":202,"body":{{"code":1,"message":"queued"}}}}
        ]"#
    );
    let results = cancel_all(&serve("207 Multi-Status", body.as_bytes())).await;
    assert_eq!(results.len(), 5);
    assert_eq!(results[0].body.as_deref(), Some(r#"{"note":"[REDACTED]"}"#));
    assert_eq!(results[0].message, None);
    assert_eq!(results[1].code, None);
    assert_eq!(
        results[1].message.as_deref(),
        Some("cannot cancel [REDACTED]")
    );
    assert_eq!(results[1].body, None);
    // A body with neither a code nor a message is kept whole, then cut.
    let message = results[2].message.as_deref().unwrap();
    assert_eq!(message.len(), 1024);
    assert!(message.starts_with(r#"{"detail":"xxx"#));
    assert_eq!(results[3].body, None);
    // Any 2xx status is a success: its body is kept whole, not read as an
    // error.
    assert_eq!(
        results[4].body.as_deref(),
        Some(r#"{"code":1,"message":"queued"}"#)
    );
    assert_eq!(results[4].code, None);
    assert_eq!(results[4].message, None);
}

#[tokio::test]
async fn an_item_without_its_id_or_status_fails_the_list() {
    for body in [r#"[{"status":200}]"#, r#"[{"id":"a"}]"#] {
        let server = serve("207 Multi-Status", body.as_bytes());
        let error = client(&server).cancel_all_orders().await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Decode, "{body}");
    }
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
    let order = fixture::upstream(REPLACE).body;

    let server = failing_once(&order);
    let replace = ReplaceOrderRequest::new();
    let error = client(&server)
        .replace_order(&order_id(), &replace)
        .await
        .unwrap_err();
    assert_eq!(error.status(), Some(503));
    assert_eq!(error.client_order_id(), Some(&replace.client_order_id));
    assert_eq!(server.requests().len(), 1);

    let server = failing_once(b"");
    let error = client(&server).cancel_order(&order_id()).await.unwrap_err();
    assert_eq!(error.status(), Some(503));
    assert_eq!(error.client_order_id(), None);
    assert_eq!(server.requests().len(), 1);

    let server = failing_once(b"[]");
    let error = client(&server).cancel_all_orders().await.unwrap_err();
    assert_eq!(error.status(), Some(503));
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn a_replace_error_carries_the_replacing_id_whatever_went_wrong() {
    let replace = ReplaceOrderRequest::new();
    for (status, body, kind) in [
        ("422 Unprocessable Entity", &b""[..], ErrorKind::Api),
        ("200 OK", b"{}", ErrorKind::Decode),
    ] {
        let server = serve(status, body);
        let error = client(&server)
            .replace_order(&order_id(), &replace)
            .await
            .unwrap_err();
        assert_eq!(error.kind(), kind);
        assert_eq!(error.client_order_id(), Some(&replace.client_order_id));
    }
}

#[test]
fn every_documented_value_has_its_wire_text() {
    for (text, value) in [
        ("open", OrderQueryStatus::Open),
        ("closed", OrderQueryStatus::Closed),
        ("all", OrderQueryStatus::All),
    ] {
        assert_eq!(text.parse::<OrderQueryStatus>().unwrap(), value);
        assert_eq!(value.to_string(), text);
    }
}

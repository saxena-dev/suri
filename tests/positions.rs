//! Positions: the upstream fixtures field by field, closing one or all
//! positions, exercising options, and mutations made exactly once.

#![cfg(feature = "rest")]

use std::time::Duration;

#[path = "support/fixture.rs"]
mod fixture;
#[path = "support/order.rs"]
mod order;
#[path = "support/server.rs"]
mod server;

use order::{AAPL_MARKET_BUY, Expected, assert_fixture_order};
use server::{Request, Server, Step, response};
use suri::{
    AssetClass, Client, ClosePosition, ClosedPosition, Config, Credentials, Decimal, Environment,
    ErrorKind, Exchange, Inbound, OrderClass, OrderSide, OrderStatus, OrderType, Position,
    PositionIntent, PositionSide, Region, Symbol, TimeInForce, Timestamp,
};

const ROUTES: &str = "trading/trading_client/test_position_routes";
const ALL: &str = "test_get_all_positions::GET /v2/positions#0";
const BY_ASSET_ID: &str = "test_get_open_position_with_asset_id::GET /v2/positions/{asset_id}#0";
const BY_SYMBOL: &str = "test_get_open_position_with_symbol::GET /v2/positions/{symbol}#0";
const CLOSE_BY_ID: &str = "test_close_position_with_id::DELETE /v2/positions/{asset_id}#0";
const CLOSE_BY_SYMBOL: &str = "test_close_position_with_symbol::DELETE /v2/positions/{symbol}#0";
const CLOSE_QTY: &str = "test_close_position_with_qty::DELETE /v2/positions/{asset_id}#0";
const CLOSE_PERCENTAGE: &str =
    "test_close_position_with_percentage::DELETE /v2/positions/{asset_id}#0";
const CLOSE_ALL: &str = "test_close_all_positions::DELETE /v2/positions#0";
const EXERCISE_BY_SYMBOL: &str =
    "test_exercise_options_position_with_symbol::POST /v2/positions/{symbol}/exercise#0";
const EXERCISE_BY_ID: &str =
    "test_exercise_options_position_with_id::POST /v2/positions/{contract_id}/exercise#0";

const ASSET_ID: &str = "904837e3-3b76-47ec-b432-046db621571b";
const SECRET: &str = "SECRETSENTINELVALUE0001";

// Built from the position schema's first example, with the properties it
// lacks set to distinct values, an option's empty exchange and an unknown
// field.
const FULL: &str = r#"{
  "asset_class":"us_option","asset_id":"98359ef7-5124-49f3-85ea-5cf02df6defa",
  "asset_marginable":true,"avg_entry_price":"100.0","avg_entry_swap_rate":"1.0811",
  "change_today":"0.0084","cost_basis":"500.0","current_price":"120.0","exchange":"",
  "lastday_price":"119.0","market_value":"600.0","prev_swap_rate":"1.0822","qty":"5",
  "qty_available":"4","side":"short","swap_rate":"1.0833","symbol":"AAPL250620C00100000",
  "unrealized_intraday_pl":"10.0","unrealized_intraday_plpc":"0.0085",
  "unrealized_pl":"100.0","unrealized_plpc":"0.20",
  "usd":{"avg_entry_price":"108.11","cost_basis":"540.55"},
  "unknown_member":[1]
}"#;

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

fn some(text: &str) -> Option<Decimal> {
    Some(dec(text))
}

fn at(text: &str) -> Timestamp {
    text.parse().unwrap()
}

fn symbol(text: &str) -> Symbol {
    text.parse().unwrap()
}

fn upstream(name: &str) -> Vec<u8> {
    fixture::upstream(&format!("{ROUTES}::{name}")).body
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

/// A POST without a body states its length as zero.
fn assert_empty_post(request: &Request) {
    assert_eq!(request.method, "POST");
    assert_no_body(request);
    assert_eq!(request.header("Content-Length"), Some("0"));
}

/// The one position every upstream position fixture carries.
fn assert_fixture_position(position: &Position) {
    assert_eq!(position.asset_id, ASSET_ID);
    assert_eq!(position.symbol, "AAPL");
    assert_eq!(position.exchange, Inbound::Known(Exchange::Nasdaq));
    assert_eq!(position.asset_class, Inbound::Known(AssetClass::UsEquity));
    assert_eq!(position.avg_entry_price, dec("100.0"));
    assert_eq!(position.qty, dec("5"));
    assert_eq!(position.side, Inbound::Known(PositionSide::Long));
    assert_eq!(position.market_value, some("600.0"));
    assert_eq!(position.cost_basis, dec("500.0"));
    assert_eq!(position.unrealized_pl, some("100.0"));
    assert_eq!(position.unrealized_plpc, some("0.20"));
    assert_eq!(position.unrealized_intraday_pl, some("10.0"));
    assert_eq!(position.unrealized_intraday_plpc, some("0.0084"));
    assert_eq!(position.current_price, some("120.0"));
    assert_eq!(position.lastday_price, some("119.0"));
    assert_eq!(position.change_today, some("0.0084"));
    assert_eq!(position.asset_marginable, None);
    assert_eq!(position.qty_available, None);
    assert_eq!(position.avg_entry_swap_rate, None);
    assert_eq!(position.prev_swap_rate, None);
    assert_eq!(position.swap_rate, None);
    let usd = position.usd.as_ref().unwrap();
    assert_eq!(usd.avg_entry_price, dec("100.0"));
    assert_eq!(usd.market_value, some("600.0"));
    assert_eq!(usd.cost_basis, dec("500.0"));
    assert_eq!(usd.unrealized_pl, some("100.0"));
    assert_eq!(usd.unrealized_plpc, some("0.20"));
    assert_eq!(usd.unrealized_intraday_pl, some("10.0"));
    assert_eq!(usd.unrealized_intraday_plpc, some("0.0084"));
    assert_eq!(usd.current_price, some("120.0"));
    assert_eq!(usd.lastday_price, some("119.0"));
    assert_eq!(usd.change_today, some("0.0084"));
}

#[tokio::test]
async fn the_position_list_fixture_decodes_field_by_field() {
    let server = serve("200 OK", &upstream(ALL));
    let positions = client(&server).positions().await.unwrap().into_parts().0;
    assert_eq!(positions.len(), 1);
    assert_fixture_position(&positions[0]);
    let request = only_request(&server);
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, "/v2/positions");
    assert_no_body(&request);
    assert_eq!(request.header("Content-Length"), None);
}

#[tokio::test]
async fn a_position_is_read_by_asset_id_or_symbol() {
    for (name, key) in [(BY_ASSET_ID, ASSET_ID), (BY_SYMBOL, "AAPL")] {
        let server = serve("200 OK", &upstream(name));
        let position = client(&server)
            .position(&symbol(key))
            .await
            .unwrap()
            .into_parts()
            .0;
        assert_fixture_position(&position);
        let request = only_request(&server);
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, format!("/v2/positions/{key}"));
        assert_no_body(&request);
    }
}

async fn read_position(body: &str) -> Result<Position, suri::Error> {
    let server = serve("200 OK", body.as_bytes());
    client(&server)
        .position(&symbol("AAPL250620C00100000"))
        .await
        .map(|response| response.into_parts().0)
}

#[tokio::test]
async fn every_position_field_decodes() {
    let position = read_position(FULL).await.unwrap();
    assert_eq!(position.asset_id, "98359ef7-5124-49f3-85ea-5cf02df6defa");
    assert_eq!(position.symbol, "AAPL250620C00100000");
    // An option position's exchange is empty.
    assert_eq!(position.exchange, Inbound::Known(Exchange::Empty));
    assert_eq!(position.asset_class, Inbound::Known(AssetClass::UsOption));
    assert_eq!(position.side, Inbound::Known(PositionSide::Short));
    assert_eq!(position.avg_entry_price, dec("100.0"));
    assert_eq!(position.qty, dec("5"));
    assert_eq!(position.market_value, some("600.0"));
    assert_eq!(position.cost_basis, dec("500.0"));
    assert_eq!(position.unrealized_pl, some("100.0"));
    assert_eq!(position.unrealized_plpc, some("0.20"));
    assert_eq!(position.unrealized_intraday_pl, some("10.0"));
    assert_eq!(position.unrealized_intraday_plpc, some("0.0085"));
    assert_eq!(position.current_price, some("120.0"));
    assert_eq!(position.lastday_price, some("119.0"));
    assert_eq!(position.change_today, some("0.0084"));
    assert_eq!(position.asset_marginable, Some(true));
    assert_eq!(position.qty_available, some("4"));
    assert_eq!(position.avg_entry_swap_rate, some("1.0811"));
    assert_eq!(position.prev_swap_rate, some("1.0822"));
    assert_eq!(position.swap_rate, some("1.0833"));
    let usd = position.usd.unwrap();
    assert_eq!(usd.avg_entry_price, dec("108.11"));
    assert_eq!(usd.cost_basis, dec("540.55"));
    assert_eq!(usd.market_value, None);
    assert_eq!(usd.unrealized_pl, None);
    assert_eq!(usd.unrealized_plpc, None);
    assert_eq!(usd.unrealized_intraday_pl, None);
    assert_eq!(usd.unrealized_intraday_plpc, None);
    assert_eq!(usd.current_price, None);
    assert_eq!(usd.lastday_price, None);
    assert_eq!(usd.change_today, None);
}

#[tokio::test]
async fn unknown_values_are_kept_and_a_missing_field_fails() {
    let unknown = FULL
        .replace(r#""side":"short""#, r#""side":"flat""#)
        .replace(r#""exchange":"""#, r#""exchange":"IEX""#);
    let position = read_position(&unknown).await.unwrap();
    assert_eq!(position.side, Inbound::Unknown("flat".to_owned()));
    assert_eq!(position.exchange, Inbound::Unknown("IEX".to_owned()));
    for field in [
        r#""cost_basis":"500.0","#,
        r#""usd":{"avg_entry_price":"108.11","cost_basis":"540.55"},"#,
    ] {
        assert_eq!(FULL.matches(field).count(), 1, "{field}");
        let body = FULL.replace(field, "");
        let result = read_position(&body).await;
        if field.starts_with(r#""usd""#) {
            assert_eq!(result.unwrap().usd, None);
        } else {
            assert_eq!(result.unwrap_err().kind(), ErrorKind::Decode, "{field}");
        }
    }
    let without_cost = FULL.replace(r#","cost_basis":"540.55""#, "");
    let error = read_position(&without_cost).await.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Decode);
}

// The pre-listing IPO position example of Alpaca's IPO guide: the
// market-data fields are null until the security trades.
const IPO: &str = r#"{
  "asset_id":"0a1b2c3d-4e5f-6789-abcd-ef0123456789","symbol":"EXMP260615","exchange":"",
  "asset_class":"ipo","qty":"100","qty_available":"100","side":"long",
  "avg_entry_price":"20","cost_basis":"2000","market_value":null,"current_price":null,
  "lastday_price":null,"change_today":null,"unrealized_pl":null,"unrealized_plpc":null,
  "unrealized_intraday_pl":null,"unrealized_intraday_plpc":null
}"#;

#[tokio::test]
async fn a_position_without_market_prices_decodes() {
    let server = serve("200 OK", format!("[{IPO}]").as_bytes());
    let positions = client(&server).positions().await.unwrap().into_parts().0;
    assert_eq!(positions.len(), 1);
    let position = &positions[0];
    assert_eq!(position.asset_id, "0a1b2c3d-4e5f-6789-abcd-ef0123456789");
    assert_eq!(position.symbol, "EXMP260615");
    assert_eq!(position.exchange, Inbound::Known(Exchange::Empty));
    assert_eq!(position.asset_class, Inbound::Known(AssetClass::Ipo));
    assert_eq!(position.side, Inbound::Known(PositionSide::Long));
    assert_eq!(position.qty, dec("100"));
    assert_eq!(position.qty_available, some("100"));
    assert_eq!(position.avg_entry_price, dec("20"));
    assert_eq!(position.cost_basis, dec("2000"));
    assert_eq!(position.market_value, None);
    assert_eq!(position.unrealized_pl, None);
    assert_eq!(position.unrealized_plpc, None);
    assert_eq!(position.unrealized_intraday_pl, None);
    assert_eq!(position.unrealized_intraday_plpc, None);
    assert_eq!(position.current_price, None);
    assert_eq!(position.lastday_price, None);
    assert_eq!(position.change_today, None);
    assert_eq!(position.asset_marginable, None);
    assert_eq!(position.avg_entry_swap_rate, None);
    assert_eq!(position.prev_swap_rate, None);
    assert_eq!(position.swap_rate, None);
    assert_eq!(position.usd, None);
}

#[tokio::test]
async fn position_decimals_follow_the_exact_rules() {
    let long = "1234567890.1234567890123456789";
    for field in [r#""qty":"5""#, r#""cost_basis":"540.55""#] {
        assert_eq!(FULL.matches(field).count(), 1, "{field}");
        let name = field.split(':').next().unwrap();
        let body = FULL.replace(field, &format!(r#"{name}:"{long}""#));
        let position = read_position(&body).await.unwrap();
        let kept = if name == r#""qty""# {
            position.qty
        } else {
            position.usd.unwrap().cost_basis
        };
        assert_eq!(kept.as_str(), long);
        for bad in ["5.5", r#""5e1""#] {
            let body = FULL.replace(field, &format!("{name}:{bad}"));
            let error = read_position(&body).await.unwrap_err();
            assert_eq!(error.kind(), ErrorKind::Decode, "{name}: {bad}");
        }
    }
}

/// Closes a position against `body` and returns the order and the request.
async fn close(key: &str, how: &ClosePosition, body: &[u8]) -> (suri::Order, Request) {
    let server = serve("200 OK", body);
    let order = client(&server)
        .close_position(&symbol(key), how)
        .await
        .unwrap()
        .into_parts()
        .0;
    let request = only_request(&server);
    assert_eq!(request.method, "DELETE");
    assert_no_body(&request);
    // A DELETE's content has no defined meaning, so no length is stated.
    assert_eq!(request.header("Content-Length"), None);
    (order, request)
}

/// The closing order the close-position fixtures carry.
fn closing(symbol: &'static str) -> Expected {
    Expected {
        symbol,
        qty: None,
        notional: Some("500"),
        ..AAPL_MARKET_BUY
    }
}

#[tokio::test]
async fn closing_a_position_returns_its_order() {
    for (name, key, how, path, symbol) in [
        (
            CLOSE_BY_ID,
            ASSET_ID,
            ClosePosition::All,
            format!("/v2/positions/{ASSET_ID}"),
            "AAPL",
        ),
        (
            CLOSE_BY_SYMBOL,
            "AAPL",
            ClosePosition::All,
            "/v2/positions/AAPL".to_owned(),
            "AAPL",
        ),
        (
            CLOSE_QTY,
            ASSET_ID,
            ClosePosition::Qty(dec("100")),
            format!("/v2/positions/{ASSET_ID}?qty=100"),
            "AAPL`",
        ),
        (
            // Half a percent.
            CLOSE_PERCENTAGE,
            ASSET_ID,
            ClosePosition::Percentage(dec("0.5")),
            format!("/v2/positions/{ASSET_ID}?percentage=0.5"),
            "AAPL`",
        ),
    ] {
        let (order, request) = close(key, &how, &upstream(name)).await;
        assert_eq!(request.path, path);
        assert_fixture_order(&order, &closing(symbol));
    }
}

#[tokio::test]
async fn a_close_sends_its_quantity_exactly_and_encodes_the_symbol() {
    let body = upstream(CLOSE_BY_SYMBOL);
    let long = "1234567890.1234567890123456789";
    let (_, request) = close("AAPL", &ClosePosition::Qty(dec(long)), &body).await;
    assert_eq!(request.path, format!("/v2/positions/AAPL?qty={long}"));
    let (_, request) = close("BTC/USD", &ClosePosition::All, &body).await;
    assert_eq!(request.path, "/v2/positions/BTC%2FUSD");
}

async fn close_all(body: &[u8], cancel_orders: bool) -> (Vec<ClosedPosition>, Request) {
    let server = serve("207 Multi-Status", body);
    let results = client(&server)
        .close_all_positions(cancel_orders)
        .await
        .unwrap()
        .into_parts()
        .0;
    let request = only_request(&server);
    assert_eq!(request.method, "DELETE");
    assert_no_body(&request);
    (results, request)
}

#[tokio::test]
#[allow(deprecated)]
async fn closing_all_positions_reports_each_one() {
    let (results, request) = close_all(&upstream(CLOSE_ALL), true).await;
    assert_eq!(request.path, "/v2/positions?cancel_orders=true");
    assert_eq!(results.len(), 3);

    let closed = &results[0];
    assert_eq!(closed.symbol, "BTCUSD");
    assert_eq!(closed.status, 200);
    assert_eq!(closed.code, None);
    assert_eq!(closed.message, None);
    assert_eq!(closed.body, None);
    let order = closed.order.as_ref().unwrap();
    assert_eq!(order.id, "e6419d12-56e4-4d67-a61b-f6e464e1f8a8");
    assert_eq!(
        order.client_order_id,
        "c6814ede-8c93-4480-9a27-7a00853d9881"
    );
    assert_eq!(order.created_at, at("2022-07-27T08:12:33.66897767Z"));
    assert_eq!(order.updated_at, Some(at("2022-07-27T08:12:33.66902729Z")));
    assert_eq!(
        order.submitted_at,
        Some(at("2022-07-27T08:12:33.667750101Z"))
    );
    assert_eq!(order.filled_at, None);
    assert_eq!(order.expired_at, None);
    assert_eq!(order.expires_at, None);
    assert_eq!(order.canceled_at, None);
    assert_eq!(order.failed_at, None);
    assert_eq!(order.replaced_at, None);
    assert_eq!(order.replaced_by, None);
    assert_eq!(order.replaces, None);
    assert_eq!(
        order.asset_id.as_deref(),
        Some("276e2673-764b-4ab6-a611-caf665ca6340")
    );
    assert_eq!(order.symbol.as_deref(), Some("BTC/USD"));
    assert_eq!(order.asset_class, Some(Inbound::Known(AssetClass::Crypto)));
    assert_eq!(order.notional, None);
    assert_eq!(order.qty, some("0.0283"));
    assert_eq!(order.filled_qty, some("0"));
    assert_eq!(order.filled_avg_price, None);
    assert_eq!(order.order_class, Some(Inbound::Known(OrderClass::Empty)));
    assert_eq!(order.order_type, Some(Inbound::Known(OrderType::Market)));
    assert_eq!(order.r#type, Some(Inbound::Known(OrderType::Market)));
    assert_eq!(order.side, Some(Inbound::Known(OrderSide::Sell)));
    assert_eq!(order.time_in_force, Inbound::Known(TimeInForce::Gtc));
    assert_eq!(order.limit_price, None);
    assert_eq!(order.stop_price, None);
    assert_eq!(order.status, Inbound::Known(OrderStatus::PendingNew));
    assert!(!order.extended_hours);
    assert_eq!(order.legs, None);
    assert_eq!(order.trail_percent, None);
    assert_eq!(order.trail_price, None);
    assert_eq!(order.hwm, None);
    assert_eq!(order.ratio_qty, None);
    assert_eq!(
        order.position_intent,
        Some(Inbound::Known(PositionIntent::SellToClose))
    );

    for (closed, symbol, message) in [
        (
            &results[1],
            "SQQQ",
            "insufficient qty available for order (requested: 1000, available: 0)",
        ),
        (&results[2], "NYC", "account is not allowed to short"),
    ] {
        assert_eq!(closed.symbol, symbol);
        assert_eq!(closed.status, 403);
        assert_eq!(closed.order, None);
        assert_eq!(closed.code, Some(40310000));
        assert_eq!(closed.message.as_deref(), Some(message));
        assert_eq!(closed.body, None);
    }
}

#[tokio::test]
async fn a_successful_item_without_an_order_keeps_what_it_has() {
    let body = format!(
        r#"[
          {{"symbol":"A","status":200}},
          {{"symbol":"B","status":200,"body":null}},
          {{"symbol":"C","status":202,"body":{{"note":"{SECRET}"}},"unknown_member":1}}
        ]"#
    );
    let (results, request) = close_all(body.as_bytes(), false).await;
    assert_eq!(request.path, "/v2/positions");
    assert_eq!(results.len(), 3);
    for closed in &results[..2] {
        assert_eq!(closed.order, None);
        assert_eq!(closed.body, None);
        assert_eq!(closed.code, None);
        assert_eq!(closed.message, None);
    }
    assert_eq!(results[2].status, 202);
    assert_eq!(results[2].order, None);
    assert_eq!(results[2].body.as_deref(), Some(r#"{"note":"[REDACTED]"}"#));
    assert_eq!(results[2].code, None);
    assert_eq!(results[2].message, None);
}

#[tokio::test]
async fn an_item_without_its_symbol_or_status_fails_the_list() {
    for body in [r#"[{"status":200}]"#, r#"[{"symbol":"A"}]"#] {
        let server = serve("207 Multi-Status", body.as_bytes());
        let error = client(&server)
            .close_all_positions(false)
            .await
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Decode, "{body}");
    }
}

#[tokio::test]
async fn an_option_is_exercised_by_symbol_or_contract_id() {
    for (name, key) in [
        (EXERCISE_BY_SYMBOL, "AAPL230616C00150000"),
        (EXERCISE_BY_ID, "8d5e2f9c-0f5b-4b6a-9f1e-0e7c3a0b2c1d"),
    ] {
        let server = serve("200 OK", &upstream(name));
        let response = client(&server).exercise_option(&symbol(key)).await.unwrap();
        assert_eq!(response.meta().status(), 200);
        let request = only_request(&server);
        assert_empty_post(&request);
        assert_eq!(request.path, format!("/v2/positions/{key}/exercise"));
    }
}

#[tokio::test]
async fn exercise_is_declined_by_symbol() {
    let server = serve("200 OK", b"");
    let response = client(&server)
        .decline_exercise(&symbol("AAPL230616C00150000"))
        .await
        .unwrap();
    assert_eq!(response.meta().status(), 200);
    let request = only_request(&server);
    assert_empty_post(&request);
    assert_eq!(
        request.path,
        "/v2/positions/AAPL230616C00150000/do-not-exercise"
    );
}

#[tokio::test]
async fn a_refused_instruction_is_an_api_error_with_its_code() {
    let short = br#"{"code":40310001,"message":"cannot submit DNE for short position"}"#;
    let server = serve("403 Forbidden", short);
    let error = client(&server)
        .decline_exercise(&symbol("AAPL230616C00150000"))
        .await
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Api);
    assert_eq!(error.status(), Some(403));
    assert_eq!(error.code(), Some(40310001));
    assert_eq!(
        error.endpoint(),
        "/v2/positions/{symbol_or_contract_id}/do-not-exercise"
    );
    assert_eq!(
        error.message(),
        Some("cannot submit DNE for short position")
    );

    let invalid = br#"{"code":42210000,"message":"invalid symbol"}"#;
    let server = serve("422 Unprocessable Entity", invalid);
    let error = client(&server)
        .exercise_option(&symbol("NOPE"))
        .await
        .unwrap_err();
    assert_eq!(error.status(), Some(422));
    assert_eq!(error.code(), Some(42210000));
    assert_eq!(
        error.endpoint(),
        "/v2/positions/{symbol_or_contract_id}/exercise"
    );
}

/// A client with the default four attempts, so a read in the same
/// situation would reach a server's second step.
fn retrying_client(server: &Server) -> Client {
    let config = Config::new(
        Region::US,
        Environment::Paper,
        Credentials::api_key("PKSENTINELKEY0001", SECRET),
    )
    .with_base_url(&server.url)
    .with_retry_base(Duration::from_millis(1));
    Client::new(config).unwrap()
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
    let contract = symbol("AAPL230616C00150000");

    let server = failing_once(&upstream(CLOSE_BY_SYMBOL));
    let error = retrying_client(&server)
        .close_position(&symbol("AAPL"), &ClosePosition::All)
        .await
        .unwrap_err();
    assert_eq!(error.status(), Some(503));
    assert_eq!(server.requests().len(), 1);

    let server = failing_once(b"[]");
    let error = retrying_client(&server)
        .close_all_positions(true)
        .await
        .unwrap_err();
    assert_eq!(error.status(), Some(503));
    assert_eq!(server.requests().len(), 1);

    let server = failing_once(b"");
    let error = retrying_client(&server)
        .exercise_option(&contract)
        .await
        .unwrap_err();
    assert_eq!(error.status(), Some(503));
    assert_eq!(server.requests().len(), 1);

    let server = failing_once(b"");
    let error = retrying_client(&server)
        .decline_exercise(&contract)
        .await
        .unwrap_err();
    assert_eq!(error.status(), Some(503));
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn every_documented_value_has_its_wire_text() {
    for (text, value) in [("long", PositionSide::Long), ("short", PositionSide::Short)] {
        assert_eq!(text.parse::<PositionSide>().unwrap(), value);
        assert_eq!(value.to_string(), text);
    }
}

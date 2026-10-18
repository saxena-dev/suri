//! Orders: the request's wire form, the order Alpaca reports, the lookup by
//! client order ID, and a placement made exactly once whatever goes wrong.

#![cfg(feature = "rest")]

#[path = "support/fixture.rs"]
mod fixture;
#[path = "support/order.rs"]
mod order;
#[path = "support/server.rs"]
mod server;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use order::{AAPL_MARKET_BUY, Expected, assert_fixture_order};
use server::{Request, Server, Step, refused_url, response};
use suri::{
    AssetClass, Client, ClientOrderId, Config, Credentials, Decimal, Environment, ErrorKind, Exit,
    Inbound, Leg, Legs, MultiLegKind, Order, OrderClass, OrderKind, OrderRequest, OrderShape,
    OrderSide, OrderStatus, OrderType, PositionIntent, Region, Size, Stage, StopLoss, TakeProfit,
    TimeInForce, Timestamp, Trail,
};

const MARKET: &str =
    "trading/trading_client/test_order_routes::test_market_order::POST /v2/orders#0";
const LIMIT: &str = "trading/trading_client/test_order_routes::test_limit_order::POST /v2/orders#0";
const INTENT_0: &str =
    "trading/trading_client/test_order_routes::test_order_position_intent::POST /v2/orders#0";
const INTENT_1: &str =
    "trading/trading_client/test_order_routes::test_order_position_intent::POST /v2/orders#1";
const BY_CLIENT_ID: &str = "trading/trading_client/test_order_routes::test_get_order_by_client_id::GET /v2/orders:by_client_order_id#0";
const NOT_FOUND: &str = "trading/trading_client/test_order_routes::test_cancel_order_throws_not_found_error::DELETE /v2/orders/{order_id}#0";

// The response example of a multi-leg order on the order placement
// reference page.
const MLEG: &str = r#"{"asset_class":"","asset_id":"","canceled_at":null,"client_order_id":"646b1fe6-b212-4f54-94c6-429e7bcdee04","created_at":"2024-12-10T16:15:53.677230742Z","expired_at":null,"extended_hours":false,"failed_at":null,"filled_at":"2024-12-10T16:15:53.694Z","filled_avg_price":"1.28","filled_qty":"1","hwm":null,"id":"83f37e9f-6b1f-49ed-8fc6-3e6af716323f","legs":[{"asset_class":"us_option","asset_id":"f0ea14b2-8a49-4e9b-89d1-894c6e518a76","canceled_at":null,"client_order_id":"cc8cc104-fe43-476c-b25c-f62650fb73f9","created_at":"2024-12-10T16:15:53.677230742Z","expired_at":null,"expires_at":"2024-12-10T21:00:00Z","extended_hours":false,"failed_at":null,"filled_at":"2024-12-10T16:15:53.694Z","filled_avg_price":"0.43","filled_qty":"3","hwm":null,"id":"df4ff24a-c58a-4e37-8b9f-ef32b83a11f2","legs":null,"limit_price":null,"notional":null,"order_class":"mleg","order_type":"","position_intent":"buy_to_open","qty":"3","ratio_qty":"3","replaced_at":null,"replaced_by":null,"replaces":null,"side":"buy","source":null,"status":"filled","stop_price":null,"submitted_at":"2024-12-10T16:15:53.684952901Z","subtag":null,"symbol":"AAPL241213C00250000","time_in_force":"day","trail_percent":null,"trail_price":null,"type":"","updated_at":"2024-12-10T16:15:53.725091158Z"},{"asset_class":"us_option","asset_id":"f89940db-eeb1-46e6-8f9b-bb1f27a0b395","canceled_at":null,"client_order_id":"0bd2d36d-4af2-4dfb-8418-333a5d5026fa","created_at":"2024-12-10T16:15:53.677230742Z","expired_at":null,"expires_at":"2024-12-10T21:00:00Z","extended_hours":false,"failed_at":null,"filled_at":"2024-12-10T16:15:53.694Z","filled_avg_price":"0.01","filled_qty":"1","hwm":null,"id":"ecd91110-c34d-4e9d-a7bf-a9c27c40f8b5","legs":null,"limit_price":null,"notional":null,"order_class":"mleg","order_type":"","position_intent":"sell_to_open","qty":"1","ratio_qty":"1","replaced_at":null,"replaced_by":null,"replaces":null,"side":"sell","source":null,"status":"filled","stop_price":null,"submitted_at":"2024-12-10T16:15:53.684952901Z","subtag":null,"symbol":"AAPL241213C00260000","time_in_force":"day","trail_percent":null,"trail_price":null,"type":"","updated_at":"2024-12-10T16:15:53.708983759Z"}],"limit_price":"10","notional":null,"order_class":"mleg","order_type":"limit","qty":"1","replaced_at":null,"replaced_by":null,"replaces":null,"side":"","source":null,"status":"filled","stop_price":null,"submitted_at":"2024-12-10T16:15:53.684952901Z","subtag":null,"symbol":"","time_in_force":"day","trail_percent":null,"trail_price":null,"type":"limit","updated_at":"2024-12-10T16:15:53.725139688Z"}"#;

fn config(url: &str) -> Config {
    // The defaults allow four attempts within 60 seconds, so a read would be
    // retried in every failure case below.
    Config::new(
        Region::US,
        Environment::Paper,
        Credentials::api_key("PKSENTINELKEY0001", "SECRETSENTINELVALUE0001"),
    )
    .with_base_url(url)
    .with_retry_base(Duration::from_millis(10))
}

fn client(server: &Server) -> Client {
    Client::new(config(&server.url)).unwrap()
}

fn dec(text: &str) -> Decimal {
    text.parse().unwrap()
}

fn at(text: &str) -> Timestamp {
    text.parse().unwrap()
}

fn id(text: &str) -> ClientOrderId {
    text.parse().unwrap()
}

fn market_buy(symbol: &str, qty: &str) -> OrderShape {
    OrderShape::Simple {
        symbol: symbol.to_owned(),
        side: OrderSide::Buy,
        size: Size::Qty(dec(qty)),
        kind: OrderKind::Market,
    }
}

async fn place(body: &[u8], order: &OrderRequest) -> (Result<Order, suri::Error>, Request) {
    let server = Server::start(vec![Step::Respond(response("200 OK", &[], body))]);
    let result = client(&server)
        .place_order(order)
        .await
        .map(|response| response.into_parts().0);
    (result, server.requests().remove(0))
}

#[tokio::test]
async fn the_placement_fixtures_decode_field_by_field() {
    let order = OrderRequest::new(market_buy("AAPL", "1"), TimeInForce::Day);
    for (fixture_id, expected) in [
        (MARKET, AAPL_MARKET_BUY),
        (
            LIMIT,
            Expected {
                kind: OrderType::Limit,
                limit_price: Some("300"),
                ..AAPL_MARKET_BUY
            },
        ),
        (
            INTENT_0,
            Expected {
                asset_id: "b4695157-0d1d-4da0-8f9e-5c53149389e4",
                symbol: "SPY`",
                ..AAPL_MARKET_BUY
            },
        ),
        (
            INTENT_1,
            Expected {
                kind: OrderType::Limit,
                side: OrderSide::Sell,
                limit_price: Some("300"),
                intent: PositionIntent::SellToOpen,
                ..AAPL_MARKET_BUY
            },
        ),
    ] {
        let (placed, _) = place(&fixture::upstream(fixture_id).body, &order).await;
        assert_fixture_order(&placed.unwrap(), &expected);
    }
}

#[tokio::test]
#[allow(deprecated)]
async fn a_multi_leg_order_decodes_with_its_legs() {
    let (placed, _) = place(
        MLEG.as_bytes(),
        &OrderRequest::new(market_buy("AAPL", "1"), TimeInForce::Day),
    )
    .await;
    let order = placed.unwrap();
    assert_eq!(order.id, "83f37e9f-6b1f-49ed-8fc6-3e6af716323f");
    assert_eq!(
        order.client_order_id,
        "646b1fe6-b212-4f54-94c6-429e7bcdee04"
    );
    assert_eq!(order.created_at, at("2024-12-10T16:15:53.677230742Z"));
    assert_eq!(order.updated_at, Some(at("2024-12-10T16:15:53.725139688Z")));
    assert_eq!(
        order.submitted_at,
        Some(at("2024-12-10T16:15:53.684952901Z"))
    );
    assert_eq!(order.filled_at, Some(at("2024-12-10T16:15:53.694Z")));
    // The parent's empty texts are not documented values of these enums.
    assert_eq!(order.asset_class, Some(Inbound::Unknown(String::new())));
    assert_eq!(order.asset_id.as_deref(), Some(""));
    assert_eq!(order.symbol.as_deref(), Some(""));
    assert_eq!(order.side, Some(Inbound::Unknown(String::new())));
    assert_eq!(order.order_class, Some(Inbound::Known(OrderClass::Mleg)));
    assert_eq!(order.r#type, Some(Inbound::Known(OrderType::Limit)));
    assert_eq!(order.order_type, Some(Inbound::Known(OrderType::Limit)));
    assert_eq!(order.qty, Some(dec("1")));
    assert_eq!(order.filled_qty, Some(dec("1")));
    assert_eq!(order.filled_avg_price, Some(dec("1.28")));
    assert_eq!(order.limit_price, Some(dec("10")));
    assert_eq!(order.status, Inbound::Known(OrderStatus::Filled));
    assert_eq!(order.time_in_force, Inbound::Known(TimeInForce::Day));
    assert_eq!(order.ratio_qty, None);
    assert_eq!(order.position_intent, None);
    assert_eq!(order.cancel_requested_at, None);
    let legs = order.legs.unwrap();
    assert_eq!(legs.len(), 2);
    for (leg, (id, symbol, qty, side, intent, price)) in legs.iter().zip([
        (
            "df4ff24a-c58a-4e37-8b9f-ef32b83a11f2",
            "AAPL241213C00250000",
            "3",
            OrderSide::Buy,
            PositionIntent::BuyToOpen,
            "0.43",
        ),
        (
            "ecd91110-c34d-4e9d-a7bf-a9c27c40f8b5",
            "AAPL241213C00260000",
            "1",
            OrderSide::Sell,
            PositionIntent::SellToOpen,
            "0.01",
        ),
    ]) {
        assert_eq!(leg.id, id);
        assert_eq!(leg.symbol.as_deref(), Some(symbol));
        assert_eq!(leg.asset_class, Some(Inbound::Known(AssetClass::UsOption)));
        assert_eq!(leg.order_class, Some(Inbound::Known(OrderClass::Mleg)));
        assert_eq!(leg.r#type, Some(Inbound::Unknown(String::new())));
        assert_eq!(leg.order_type, Some(Inbound::Unknown(String::new())));
        assert_eq!(leg.side, Some(Inbound::Known(side)));
        assert_eq!(leg.position_intent, Some(Inbound::Known(intent)));
        assert_eq!(leg.qty, Some(dec(qty)));
        assert_eq!(leg.ratio_qty, Some(dec(qty)));
        assert_eq!(leg.filled_qty, Some(dec(qty)));
        assert_eq!(leg.filled_avg_price, Some(dec(price)));
        assert_eq!(leg.expires_at, Some(at("2024-12-10T21:00:00Z")));
        assert_eq!(leg.cancel_requested_at, None);
        assert_eq!(leg.status, Inbound::Known(OrderStatus::Filled));
        assert_eq!(leg.legs, None);
    }
}

#[tokio::test]
async fn unknown_values_are_kept_and_a_missing_field_fails() {
    let body = String::from_utf8(fixture::upstream(MARKET).body).unwrap();
    let order = OrderRequest::new(market_buy("AAPL", "1"), TimeInForce::Day);
    let unknown = body
        .replace(r#""status": "accepted""#, r#""status": "frozen""#)
        .replace(r#""time_in_force": "day""#, r#""time_in_force": "gtd""#);
    let placed = place(unknown.as_bytes(), &order).await.0.unwrap();
    assert_eq!(placed.status, Inbound::Unknown("frozen".to_owned()));
    assert_eq!(placed.time_in_force, Inbound::Unknown("gtd".to_owned()));
    for field in [
        r#""extended_hours": false,"#,
        r#""created_at": "2021-03-16T18:38:01.942282Z","#,
        r#""time_in_force": "day","#,
    ] {
        assert_eq!(body.matches(field).count(), 1, "{field}");
        let without = body.replace(field, "");
        let error = place(without.as_bytes(), &order).await.0.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Decode, "{field}");
    }
}

#[tokio::test]
async fn order_decimals_follow_the_exact_rules() {
    let body = String::from_utf8(fixture::upstream(MARKET).body).unwrap();
    let order = OrderRequest::new(market_buy("AAPL", "1"), TimeInForce::Day);
    let long = "1234567890.1234567890123456789";
    let with_long = body.replace(r#""qty": 1,"#, &format!(r#""qty": "{long}","#));
    let placed = place(with_long.as_bytes(), &order).await.0.unwrap();
    assert_eq!(placed.qty.unwrap().as_str(), long);
    for bad in ["1.5", r#""1e2""#] {
        let with_bad = body.replace(r#""qty": 1,"#, &format!(r#""qty": {bad},"#));
        assert_ne!(with_bad, body);
        let error = place(with_bad.as_bytes(), &order).await.0.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Decode, "{bad}");
    }
}

/// Places the order and returns the body sent, after checking the request
/// line, the content type, and that the order's JSON is that body.
async fn sent_body(order: &OrderRequest) -> String {
    let (placed, request) = place(&fixture::upstream(MARKET).body, order).await;
    placed.unwrap();
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/v2/orders");
    assert_eq!(request.header("Content-Type"), Some("application/json"));
    let body = String::from_utf8(request.body).unwrap();
    assert_eq!(serde_json::to_string(order).unwrap(), body);
    body
}

#[tokio::test]
async fn each_simple_order_is_sent_exactly() {
    let simple = |side, size, kind| OrderShape::Simple {
        symbol: "AAPL".to_owned(),
        side,
        size,
        kind,
    };
    let mut by_notional = OrderRequest::new(
        simple(
            OrderSide::Buy,
            Size::Notional(dec("500")),
            OrderKind::Limit {
                limit_price: dec("150"),
            },
        ),
        TimeInForce::Day,
    );
    by_notional.extended_hours = Some(true);
    by_notional.position_intent = Some(PositionIntent::BuyToOpen);
    let cases = [
        (
            OrderRequest::new(
                simple(OrderSide::Buy, Size::Qty(dec("2")), OrderKind::Market),
                TimeInForce::Day,
            ),
            r#"{"symbol":"AAPL","qty":"2","side":"buy","type":"market","time_in_force":"day","client_order_id":"order-1"}"#,
        ),
        (
            by_notional,
            r#"{"symbol":"AAPL","notional":"500","side":"buy","type":"limit","time_in_force":"day","limit_price":"150","extended_hours":true,"client_order_id":"order-1","position_intent":"buy_to_open"}"#,
        ),
        (
            OrderRequest::new(
                simple(
                    OrderSide::Sell,
                    Size::Qty(dec("1")),
                    OrderKind::Stop {
                        stop_price: dec("140"),
                    },
                ),
                TimeInForce::Gtc,
            ),
            r#"{"symbol":"AAPL","qty":"1","side":"sell","type":"stop","time_in_force":"gtc","stop_price":"140","client_order_id":"order-1"}"#,
        ),
        (
            OrderRequest::new(
                simple(
                    OrderSide::Sell,
                    Size::Qty(dec("1")),
                    OrderKind::StopLimit {
                        stop_price: dec("140"),
                        limit_price: dec("139.5"),
                    },
                ),
                TimeInForce::Day,
            ),
            r#"{"symbol":"AAPL","qty":"1","side":"sell","type":"stop_limit","time_in_force":"day","limit_price":"139.5","stop_price":"140","client_order_id":"order-1"}"#,
        ),
        (
            OrderRequest::new(
                simple(
                    OrderSide::Sell,
                    Size::Qty(dec("1")),
                    OrderKind::TrailingStop(Trail::Price(dec("1.00"))),
                ),
                TimeInForce::Day,
            ),
            r#"{"symbol":"AAPL","qty":"1","side":"sell","type":"trailing_stop","time_in_force":"day","trail_price":"1.00","client_order_id":"order-1"}"#,
        ),
        (
            OrderRequest::new(
                simple(
                    OrderSide::Sell,
                    Size::Qty(dec("1")),
                    OrderKind::TrailingStop(Trail::Percent(dec("1.5"))),
                ),
                TimeInForce::Day,
            ),
            r#"{"symbol":"AAPL","qty":"1","side":"sell","type":"trailing_stop","time_in_force":"day","trail_percent":"1.5","client_order_id":"order-1"}"#,
        ),
        (
            // A crypto order, good until canceled.
            OrderRequest::new(market_buy("BTC/USD", "0.0001"), TimeInForce::Gtc),
            r#"{"symbol":"BTC/USD","qty":"0.0001","side":"buy","type":"market","time_in_force":"gtc","client_order_id":"order-1"}"#,
        ),
        (
            // More than 17 significant digits, sent as written.
            OrderRequest::new(
                market_buy("AAPL", "1234567890.1234567890123456789"),
                TimeInForce::Day,
            ),
            r#"{"symbol":"AAPL","qty":"1234567890.1234567890123456789","side":"buy","type":"market","time_in_force":"day","client_order_id":"order-1"}"#,
        ),
    ];
    for (mut order, expected) in cases {
        order.client_order_id = id("order-1");
        assert_eq!(sent_body(&order).await, expected);
    }
}

#[tokio::test]
async fn each_order_class_is_sent_exactly() {
    let mut stop_limit_loss = StopLoss::new(dec("190"));
    stop_limit_loss.limit_price = Some(dec("189.5"));
    let mut oco_loss = StopLoss::new(dec("299"));
    oco_loss.limit_price = Some(dec("298.5"));
    let mut bought = Leg::new("AAPL241213C00250000", dec("3"));
    bought.side = Some(OrderSide::Buy);
    bought.position_intent = Some(PositionIntent::BuyToOpen);
    let mut sold = Leg::new("AAPL241213C00260000", dec("1"));
    sold.side = Some(OrderSide::Sell);
    sold.position_intent = Some(PositionIntent::SellToOpen);
    let cases = [
        (
            OrderShape::Bracket {
                symbol: "SPY".to_owned(),
                side: OrderSide::Buy,
                size: Size::Qty(dec("100")),
                kind: OrderKind::Limit {
                    limit_price: dec("200"),
                },
                take_profit: TakeProfit::new(dec("210")),
                stop_loss: stop_limit_loss,
            },
            r#"{"symbol":"SPY","qty":"100","side":"buy","type":"limit","time_in_force":"gtc","limit_price":"200","client_order_id":"order-1","order_class":"bracket","take_profit":{"limit_price":"210"},"stop_loss":{"stop_price":"190","limit_price":"189.5"}}"#,
        ),
        (
            OrderShape::Oto {
                symbol: "SPY".to_owned(),
                side: OrderSide::Buy,
                size: Size::Qty(dec("100")),
                kind: OrderKind::Market,
                exit: Exit::StopLoss(StopLoss::new(dec("150"))),
            },
            r#"{"symbol":"SPY","qty":"100","side":"buy","type":"market","time_in_force":"gtc","client_order_id":"order-1","order_class":"oto","stop_loss":{"stop_price":"150"}}"#,
        ),
        (
            OrderShape::Oto {
                symbol: "SPY".to_owned(),
                side: OrderSide::Buy,
                size: Size::Qty(dec("100")),
                kind: OrderKind::Market,
                exit: Exit::TakeProfit(TakeProfit::new(dec("301"))),
            },
            r#"{"symbol":"SPY","qty":"100","side":"buy","type":"market","time_in_force":"gtc","client_order_id":"order-1","order_class":"oto","take_profit":{"limit_price":"301"}}"#,
        ),
        (
            OrderShape::Oco {
                symbol: "SPY".to_owned(),
                side: OrderSide::Sell,
                size: Size::Qty(dec("100")),
                take_profit: TakeProfit::new(dec("301")),
                stop_loss: oco_loss,
            },
            r#"{"symbol":"SPY","qty":"100","side":"sell","type":"limit","time_in_force":"gtc","client_order_id":"order-1","order_class":"oco","take_profit":{"limit_price":"301"},"stop_loss":{"stop_price":"299","limit_price":"298.5"}}"#,
        ),
        (
            OrderShape::MultiLeg {
                qty: dec("3"),
                kind: MultiLegKind::Limit {
                    limit_price: dec("10"),
                },
                legs: Legs::new(vec![bought, sold]).unwrap(),
            },
            r#"{"qty":"3","type":"limit","time_in_force":"gtc","limit_price":"10","client_order_id":"order-1","order_class":"mleg","legs":[{"symbol":"AAPL241213C00250000","ratio_qty":"3","side":"buy","position_intent":"buy_to_open"},{"symbol":"AAPL241213C00260000","ratio_qty":"1","side":"sell","position_intent":"sell_to_open"}]}"#,
        ),
        (
            OrderShape::MultiLeg {
                qty: dec("1"),
                kind: MultiLegKind::Market,
                legs: Legs::new(vec![
                    Leg::new("AAPL241213C00250000", dec("1")),
                    Leg::new("AAPL241213P00240000", dec("2")),
                ])
                .unwrap(),
            },
            r#"{"qty":"1","type":"market","time_in_force":"gtc","client_order_id":"order-1","order_class":"mleg","legs":[{"symbol":"AAPL241213C00250000","ratio_qty":"1"},{"symbol":"AAPL241213P00240000","ratio_qty":"2"}]}"#,
        ),
    ];
    for (shape, expected) in cases {
        let mut order = OrderRequest::new(shape, TimeInForce::Gtc);
        order.client_order_id = id("order-1");
        assert_eq!(sent_body(&order).await, expected);
    }
}

#[test]
fn a_multi_leg_order_takes_at_most_four_legs() {
    let leg = Leg::new("AAPL241213C00250000", dec("1"));
    assert_eq!(Legs::new(vec![leg.clone(); 4]).unwrap().as_slice().len(), 4);
    let error = Legs::new(vec![leg; 5]).unwrap_err();
    assert_eq!(error.to_string(), "a multi-leg order takes at most 4 legs");
}

#[test]
fn a_client_order_id_has_1_to_128_characters() {
    for text in ["a".repeat(128), "é".repeat(128), "x".to_owned()] {
        assert_eq!(text.parse::<ClientOrderId>().unwrap().as_str(), text);
    }
    for (text, message) in [
        (String::new(), "client order ID must not be empty"),
        (
            "a".repeat(129),
            "client order ID must be at most 128 characters",
        ),
        (
            "é".repeat(129),
            "client order ID must be at most 128 characters",
        ),
    ] {
        let error = text.parse::<ClientOrderId>().unwrap_err();
        assert_eq!(error.to_string(), message);
    }
}

#[test]
fn a_new_request_has_its_own_generated_id() {
    let first = OrderRequest::new(market_buy("AAPL", "1"), TimeInForce::Day);
    let second = OrderRequest::new(market_buy("AAPL", "1"), TimeInForce::Day);
    assert_ne!(first.client_order_id, second.client_order_id);
    assert_eq!(first.client_order_id.as_str().len(), 36);
}

#[tokio::test]
async fn an_order_is_looked_up_by_its_client_order_id() {
    let server = Server::start(vec![Step::Respond(response(
        "200 OK",
        &[],
        &fixture::upstream(BY_CLIENT_ID).body,
    ))]);
    let order = client(&server)
        .order_by_client_order_id(&id("eb9e2aaa-f71a-4f51-b5b4-52a6c565dad4"))
        .await
        .unwrap()
        .into_parts()
        .0;
    assert_fixture_order(&order, &AAPL_MARKET_BUY);
    let request = &server.requests()[0];
    assert_eq!(request.method, "GET");
    // The colon stays unencoded.
    assert_eq!(
        request.path,
        "/v2/orders:by_client_order_id?client_order_id=eb9e2aaa-f71a-4f51-b5b4-52a6c565dad4"
    );
    assert_eq!(request.header("Content-Type"), None);
    assert!(request.body.is_empty());
}

#[tokio::test]
async fn an_unknown_client_order_id_is_an_api_error() {
    let fixture = fixture::upstream(NOT_FOUND);
    assert_eq!(fixture.status, 404);
    let server = Server::start(vec![Step::Respond(response(
        "404 Not Found",
        &[],
        &fixture.body,
    ))]);
    let error = client(&server)
        .order_by_client_order_id(&id("order-1"))
        .await
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Api);
    assert_eq!(error.status(), Some(404));
    assert_eq!(error.code(), None);
    assert_eq!(error.client_order_id(), None);
}

/// Places an order whose response never comes, then looks it up by the ID
/// read from the request before it was sent; `lookup` is the lookup's
/// response.
async fn timed_out_then_looked_up(lookup: Vec<u8>) -> Result<Order, suri::Error> {
    let server = Server::start(vec![Step::Hold, Step::Respond(lookup)]);
    let client = Client::new(
        config(&server.url)
            .with_connect_timeout(Duration::from_secs(1))
            .with_request_timeout(Duration::from_secs(1)),
    )
    .unwrap();
    let order = OrderRequest::new(market_buy("AAPL", "1"), TimeInForce::Day);
    let known = order.client_order_id.clone();

    let error = client.place_order(&order).await.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Timeout);
    assert_eq!(error.stage(), Stage::Unknown);
    assert!(error.may_have_reached_broker());
    assert_eq!(error.attempt(), 1);
    assert_eq!(error.client_order_id(), Some(&known));
    assert_eq!(server.requests().len(), 1);

    let result = client
        .order_by_client_order_id(error.client_order_id().unwrap())
        .await
        .map(|response| response.into_parts().0);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1].path,
        format!("/v2/orders:by_client_order_id?client_order_id={known}")
    );
    result
}

// real-time: reqwest's request timeout runs on a real socket, which a paused
// clock cannot drive.
#[tokio::test]
async fn a_timed_out_order_that_was_placed_is_found_by_its_id() {
    let body = fixture::upstream(BY_CLIENT_ID).body;
    let order = timed_out_then_looked_up(response("200 OK", &[], &body))
        .await
        .unwrap();
    assert_eq!(order.id, "61e69015-8549-4bfd-b9c3-01e75843f47d");
}

// real-time: as above.
#[tokio::test]
async fn a_timed_out_order_that_was_not_placed_is_not_found() {
    let error = timed_out_then_looked_up(response(
        "404 Not Found",
        &[],
        br#"{"code":40410000,"message":"order not found"}"#,
    ))
    .await
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Api);
    assert_eq!(error.status(), Some(404));
    assert_eq!(error.code(), Some(40410000));
    assert_eq!(error.message(), Some("order not found"));
}

/// Places one order against `first` followed by a spare success, and
/// returns the error, the number of requests received, and the order's ID.
async fn one_placement(
    first: Step,
    adjust: impl FnOnce(Config) -> Config,
) -> (suri::Error, usize, ClientOrderId) {
    let server = Server::start(vec![
        first,
        Step::Respond(response("200 OK", &[], &fixture::upstream(MARKET).body)),
    ]);
    let client = Client::new(adjust(config(&server.url))).unwrap();
    let order = OrderRequest::new(market_buy("AAPL", "1"), TimeInForce::Day);
    let error = client.place_order(&order).await.unwrap_err();
    (error, server.requests().len(), order.client_order_id)
}

#[tokio::test]
async fn a_failed_placement_is_never_retried_and_carries_its_id() {
    let reset = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 1)
    .to_string();
    for (first, kind, status) in [
        (
            Step::Respond(response("503 Service Unavailable", &[], b"")),
            ErrorKind::Api,
            Some(503),
        ),
        (
            Step::Respond(response(
                "429 Too Many Requests",
                &[("X-RateLimit-Reset", &reset)],
                b"",
            )),
            ErrorKind::Api,
            Some(429),
        ),
        (Step::Drop, ErrorKind::Transport, None),
        (
            Step::Respond(response(
                "307 Temporary Redirect",
                &[("Location", "https://h/v2/orders")],
                b"",
            )),
            ErrorKind::Redirect,
            Some(307),
        ),
    ] {
        let (error, requests, known) = one_placement(first, |c| c).await;
        assert_eq!(requests, 1, "{kind:?}");
        assert_eq!(error.kind(), kind);
        assert_eq!(error.status(), status);
        assert!(error.may_have_reached_broker());
        assert_eq!(error.attempt(), 1);
        assert_eq!(error.client_order_id(), Some(&known));
    }
}

// real-time: as above; a read would retry well inside the 60 s deadline.
#[tokio::test]
async fn a_timed_out_placement_is_not_retried() {
    let (error, requests, known) = one_placement(Step::Hold, |c| {
        c.with_connect_timeout(Duration::from_secs(1))
            .with_request_timeout(Duration::from_secs(1))
    })
    .await;
    assert_eq!(requests, 1);
    assert_eq!(error.kind(), ErrorKind::Timeout);
    assert_eq!(error.attempt(), 1);
    assert_eq!(error.client_order_id(), Some(&known));
}

#[tokio::test]
async fn a_refused_placement_never_reached_alpaca_and_carries_its_id() {
    let client = Client::new(config(&refused_url())).unwrap();
    let order = OrderRequest::new(market_buy("AAPL", "1"), TimeInForce::Day);
    let error = client.place_order(&order).await.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Transport);
    assert!(!error.may_have_reached_broker());
    assert_eq!(error.attempt(), 1);
    assert_eq!(error.client_order_id(), Some(&order.client_order_id));
}

#[tokio::test]
async fn a_decode_error_carries_the_id_too() {
    let order = OrderRequest::new(market_buy("AAPL", "1"), TimeInForce::Day);
    let error = place(b"{}", &order).await.0.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Decode);
    assert_eq!(error.client_order_id(), Some(&order.client_order_id));
}

/// Asserts that each (text, value) pair parses and writes back exactly.
fn round_trip<T>(pairs: &[(&str, T)])
where
    T: std::str::FromStr + PartialEq + std::fmt::Debug + std::fmt::Display,
    T::Err: std::fmt::Debug,
{
    for (text, value) in pairs {
        assert_eq!(&text.parse::<T>().unwrap(), value, "{text}");
        assert_eq!(value.to_string(), *text);
    }
}

#[test]
fn every_documented_value_has_its_wire_text() {
    use OrderType as T;
    round_trip(&[
        ("market", T::Market),
        ("limit", T::Limit),
        ("stop", T::Stop),
        ("stop_limit", T::StopLimit),
        ("trailing_stop", T::TrailingStop),
    ]);
    use TimeInForce as F;
    round_trip(&[
        ("day", F::Day),
        ("gtc", F::Gtc),
        ("opg", F::Opg),
        ("cls", F::Cls),
        ("ioc", F::Ioc),
        ("fok", F::Fok),
    ]);
    use OrderClass as C;
    round_trip(&[
        ("simple", C::Simple),
        ("bracket", C::Bracket),
        ("oco", C::Oco),
        ("oto", C::Oto),
        ("mleg", C::Mleg),
        ("", C::Empty),
    ]);
    use PositionIntent as P;
    round_trip(&[
        ("buy_to_open", P::BuyToOpen),
        ("buy_to_close", P::BuyToClose),
        ("sell_to_open", P::SellToOpen),
        ("sell_to_close", P::SellToClose),
    ]);
}

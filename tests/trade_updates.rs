//! Typed trade updates: the documented examples field by field, every
//! event, exact decimals, and the messages suri keeps raw.

#![cfg(feature = "stream")]

#[path = "support/ws_server.rs"]
mod ws_server;

use std::time::Duration;

use suri::{
    AssetClass, Client, Config, Credentials, Decimal, Environment, Inbound, OrderClass, OrderSide,
    OrderStatus, OrderType, Region, TimeInForce, Timestamp, TradeEvent, TradeStreamMessage,
    TradeUpdate,
};
use tokio_tungstenite::tungstenite::Message;
use ws_server::{Act, WsServer, opened};

fn dec(text: &str) -> Decimal {
    text.parse().unwrap()
}

fn at(text: &str) -> Timestamp {
    text.parse().unwrap()
}

/// Serves each text as one frame after the opening and returns what
/// `next` delivers for each.
async fn deliver(frames: &[&str]) -> Vec<TradeStreamMessage> {
    let acts = frames
        .iter()
        .map(|frame| Act::Send(Message::text(*frame)))
        .chain([Act::Pause(Duration::from_secs(5))])
        .collect();
    let server = WsServer::start(vec![opened(acts)]);
    let config = Config::new(
        Region::US,
        Environment::Paper,
        Credentials::api_key("PKSENTINELKEY0001", "SECRETSENTINELVALUE0001"),
    )
    .with_base_url(&server.url);
    let mut stream = Client::new(config).unwrap().trade_updates().await.unwrap();
    let mut messages = Vec::new();
    for _ in frames {
        messages.push(stream.next().await.unwrap());
    }
    messages
}

async fn update(frame: &str) -> TradeUpdate {
    match deliver(&[frame]).await.remove(0) {
        TradeStreamMessage::Update(update) => update,
        other => panic!("not a trade update: {other:?}"),
    }
}

/// A `trade_updates` message whose order holds only the required fields.
fn minimal(event: &str, extra: &str) -> String {
    format!(r#"{{"stream":"trade_updates","data":{{"event":"{event}","order":{ORDER}{extra}}}}}"#)
}

const ORDER: &str = r#"{"id":"o1","client_order_id":"c1","created_at":"2025-01-21T07:32:40Z","status":"new","extended_hours":false,"time_in_force":"day"}"#;

/// The documented fill (the docs' example, as sent).
const FILL: &str = r#"{"stream":"trade_updates","data":{"event":"fill","execution_id":"2f63ea93-423d-4169-b3f6-3fdafc10c418","order":{"asset_class":"crypto","asset_id":"1cf35270-99ee-44e2-a77f-6fab902c7f80","cancel_requested_at":null,"canceled_at":null,"client_order_id":"4642fd68-d59a-47d7-a9ac-e22f536828d1","created_at":"2022-04-19T13:45:04.981350886-04:00","expired_at":null,"extended_hours":false,"failed_at":null,"filled_at":"2022-04-19T17:45:05.024916716Z","filled_avg_price":"105.8988475","filled_qty":"1790.86","hwm":null,"id":"a5be8f5e-fdfa-41f5-a644-7a74fe947a8f","legs":null,"limit_price":null,"notional":null,"order_class":"","order_type":"market","qty":"1790.86","replaced_at":null,"replaced_by":null,"replaces":null,"side":"sell","status":"filled","stop_price":null,"submitted_at":"2022-04-19T13:45:04.980944666-04:00","symbol":"SOLUSD","time_in_force":"gtc","trail_percent":null,"trail_price":null,"type":"market","updated_at":"2022-04-19T13:45:05.027690731-04:00"},"position_qty":"0","price":"105.8988475","qty":"1790.86","timestamp":"2022-04-19T17:45:05.024916716Z"}}"#;

/// The documented multi-leg fill (the docs' example, as sent).
const MLEG_FILL: &str = r#"{"stream":"trade_updates","data":{"at":"2025-01-21T07:32:40.70095Z","event_id":"01JJ3WE73W5PG672TC4XACXH5R","event":"fill","timestamp":"2025-01-21T07:32:40.695569506Z","order":{"id":"31cd620f-3bd5-41b7-8bb2-6834524679d0","client_order_id":"fe999618-6435-497b-9fdd-a63d3da3615f","created_at":"2025-01-21T07:32:40.678963102Z","updated_at":"2025-01-21T07:32:40.699359002Z","submitted_at":"2025-01-21T07:32:40.691562346Z","filled_at":"2025-01-21T07:32:40.695569506Z","expired_at":null,"cancel_requested_at":null,"canceled_at":null,"failed_at":null,"replaced_at":null,"replaced_by":null,"replaces":null,"asset_id":"00000000-0000-0000-0000-000000000000","symbol":"","asset_class":"","notional":null,"qty":"1","filled_qty":"1","filled_avg_price":"1.62","order_class":"mleg","order_type":"limit","type":"limit","side":"buy","time_in_force":"day","limit_price":"2","stop_price":null,"status":"filled","extended_hours":false,"legs":[{"id":"3cbe69ef-241c-43ba-9d8c-09361930a1af","client_order_id":"e868fb88-ce92-442b-91be-4b16defbc883","created_at":"2025-01-21T07:32:40.678963102Z","updated_at":"2025-01-21T07:32:40.697474882Z","submitted_at":"2025-01-21T07:32:40.687356797Z","filled_at":"2025-01-21T07:32:40.695564076Z","expired_at":null,"cancel_requested_at":null,"canceled_at":null,"failed_at":null,"replaced_at":null,"replaced_by":null,"replaces":null,"asset_id":"925af3ed-5c00-4ef1-b89b-e4bd05f04486","symbol":"AAPL250321P00200000","asset_class":"us_option","notional":null,"qty":"1","filled_qty":"1","filled_avg_price":"1.6","order_class":"mleg","order_type":"","type":"","side":"buy","time_in_force":"day","limit_price":null,"stop_price":null,"status":"filled","extended_hours":false,"legs":null,"trail_percent":null,"trail_price":null,"hwm":null,"ratio_qty":"1"},{"id":"ec694de5-5028-4347-8f89-d8ea00c9341f","client_order_id":"0a1bf1e1-6992-4c23-85a6-9469bbe05f1a","created_at":"2025-01-21T07:32:40.678963102Z","updated_at":"2025-01-21T07:32:40.699294952Z","submitted_at":"2025-01-21T07:32:40.691562346Z","filled_at":"2025-01-21T07:32:40.695569506Z","expired_at":null,"cancel_requested_at":null,"canceled_at":null,"failed_at":null,"replaced_at":null,"replaced_by":null,"replaces":null,"asset_id":"9f8c3d65-f5f7-42cd-acbc-9636cc32d3b5","symbol":"AAPL250321C00380000","asset_class":"us_option","notional":null,"qty":"1","filled_qty":"1","filled_avg_price":"0.02","order_class":"mleg","order_type":"","type":"","side":"buy","time_in_force":"day","limit_price":null,"stop_price":null,"status":"filled","extended_hours":false,"legs":null,"trail_percent":null,"trail_price":null,"hwm":null,"ratio_qty":"1"}],"trail_percent":null,"trail_price":null,"hwm":null},"price":"1.62","qty":"1","position_qtys":{"AAPL250321P00200000":"1","AAPL250321C00380000":"1"},"legs":[{"execution_id":"69a70e98-f370-427d-bcd3-834dc4800aed","qty":"1","price":"1.6","order_id":"3cbe69ef-241c-43ba-9d8c-09361930a1af","symbol":"AAPL250321P00200000","timestamp":"2025-01-21T07:32:40.695564076Z"},{"execution_id":"fb878d87-569e-49f3-b42e-a09ad06e3d3a","qty":"1","price":"0.02","order_id":"ec694de5-5028-4347-8f89-d8ea00c9341f","symbol":"AAPL250321C00380000","timestamp":"2025-01-21T07:32:40.695569506Z"}]}}"#;

#[tokio::test]
#[allow(deprecated)]
async fn the_documented_fill_is_read_field_by_field() {
    let update = update(FILL).await;
    assert_eq!(update.event, Inbound::Known(TradeEvent::Fill));
    assert_eq!(
        update.execution_id.as_deref(),
        Some("2f63ea93-423d-4169-b3f6-3fdafc10c418")
    );
    assert_eq!(update.position_qty, Some(dec("0")));
    assert_eq!(update.price, Some(dec("105.8988475")));
    assert_eq!(update.qty, Some(dec("1790.86")));
    assert_eq!(update.timestamp, Some(at("2022-04-19T17:45:05.024916716Z")));
    assert_eq!(update.at, None);
    assert_eq!(update.event_id, None);
    assert_eq!(update.position_qtys, None);
    assert_eq!(update.legs, None);

    let order = &update.order;
    assert_eq!(order.asset_class, Some(Inbound::Known(AssetClass::Crypto)));
    assert_eq!(
        order.asset_id.as_deref(),
        Some("1cf35270-99ee-44e2-a77f-6fab902c7f80")
    );
    assert_eq!(order.cancel_requested_at, None);
    assert_eq!(order.canceled_at, None);
    assert_eq!(
        order.client_order_id,
        "4642fd68-d59a-47d7-a9ac-e22f536828d1"
    );
    // The offset times are the instants they name.
    assert_eq!(order.created_at, at("2022-04-19T17:45:04.981350886Z"));
    assert_eq!(order.expired_at, None);
    assert_eq!(order.expires_at, None);
    assert!(!order.extended_hours);
    assert_eq!(order.failed_at, None);
    assert_eq!(order.filled_at, Some(at("2022-04-19T17:45:05.024916716Z")));
    assert_eq!(order.filled_avg_price, Some(dec("105.8988475")));
    assert_eq!(order.filled_qty, Some(dec("1790.86")));
    assert_eq!(order.hwm, None);
    assert_eq!(order.id, "a5be8f5e-fdfa-41f5-a644-7a74fe947a8f");
    assert_eq!(order.legs, None);
    assert_eq!(order.limit_price, None);
    assert_eq!(order.notional, None);
    assert_eq!(order.order_class, Some(Inbound::Known(OrderClass::Empty)));
    assert_eq!(order.order_type, Some(Inbound::Known(OrderType::Market)));
    assert_eq!(order.qty, Some(dec("1790.86")));
    assert_eq!(order.replaced_at, None);
    assert_eq!(order.replaced_by, None);
    assert_eq!(order.replaces, None);
    assert_eq!(order.side, Some(Inbound::Known(OrderSide::Sell)));
    assert_eq!(order.status, Inbound::Known(OrderStatus::Filled));
    assert_eq!(order.stop_price, None);
    assert_eq!(
        order.submitted_at,
        Some(at("2022-04-19T17:45:04.980944666Z"))
    );
    assert_eq!(order.symbol.as_deref(), Some("SOLUSD"));
    assert_eq!(order.time_in_force, Inbound::Known(TimeInForce::Gtc));
    assert_eq!(order.trail_percent, None);
    assert_eq!(order.trail_price, None);
    assert_eq!(order.r#type, Some(Inbound::Known(OrderType::Market)));
    assert_eq!(order.updated_at, Some(at("2022-04-19T17:45:05.027690731Z")));
    assert_eq!(order.ratio_qty, None);
    assert_eq!(order.position_intent, None);
}

#[tokio::test]
#[allow(deprecated)]
async fn the_documented_multi_leg_fill_is_read_field_by_field() {
    let update = update(MLEG_FILL).await;
    assert_eq!(update.event, Inbound::Known(TradeEvent::Fill));
    assert_eq!(update.at, Some(at("2025-01-21T07:32:40.70095Z")));
    assert_eq!(
        update.event_id.as_deref(),
        Some("01JJ3WE73W5PG672TC4XACXH5R")
    );
    assert_eq!(update.timestamp, Some(at("2025-01-21T07:32:40.695569506Z")));
    assert_eq!(update.execution_id, None);
    assert_eq!(update.price, Some(dec("1.62")));
    assert_eq!(update.qty, Some(dec("1")));
    assert_eq!(update.position_qty, None);
    let position_qtys = update.position_qtys.as_ref().unwrap();
    assert_eq!(position_qtys.len(), 2);
    assert_eq!(position_qtys["AAPL250321P00200000"], dec("1"));
    assert_eq!(position_qtys["AAPL250321C00380000"], dec("1"));

    let fills = update.legs.as_ref().unwrap();
    assert_eq!(fills.len(), 2);
    let expected = [
        (
            "69a70e98-f370-427d-bcd3-834dc4800aed",
            "1.6",
            "3cbe69ef-241c-43ba-9d8c-09361930a1af",
            "AAPL250321P00200000",
            "2025-01-21T07:32:40.695564076Z",
        ),
        (
            "fb878d87-569e-49f3-b42e-a09ad06e3d3a",
            "0.02",
            "ec694de5-5028-4347-8f89-d8ea00c9341f",
            "AAPL250321C00380000",
            "2025-01-21T07:32:40.695569506Z",
        ),
    ];
    for (fill, (execution_id, price, order_id, symbol, time)) in fills.iter().zip(expected) {
        assert_eq!(fill.execution_id.as_deref(), Some(execution_id));
        assert_eq!(fill.qty, Some(dec("1")));
        assert_eq!(fill.price, Some(dec(price)));
        assert_eq!(fill.order_id.as_deref(), Some(order_id));
        assert_eq!(fill.symbol.as_deref(), Some(symbol));
        assert_eq!(fill.timestamp, Some(at(time)));
    }

    let order = &update.order;
    assert_eq!(order.id, "31cd620f-3bd5-41b7-8bb2-6834524679d0");
    assert_eq!(
        order.client_order_id,
        "fe999618-6435-497b-9fdd-a63d3da3615f"
    );
    assert_eq!(order.created_at, at("2025-01-21T07:32:40.678963102Z"));
    assert_eq!(order.updated_at, Some(at("2025-01-21T07:32:40.699359002Z")));
    assert_eq!(
        order.submitted_at,
        Some(at("2025-01-21T07:32:40.691562346Z"))
    );
    assert_eq!(order.filled_at, Some(at("2025-01-21T07:32:40.695569506Z")));
    assert_eq!(order.expired_at, None);
    assert_eq!(order.expires_at, None);
    assert_eq!(order.cancel_requested_at, None);
    assert_eq!(order.canceled_at, None);
    assert_eq!(order.failed_at, None);
    assert_eq!(order.replaced_at, None);
    assert_eq!(order.replaced_by, None);
    assert_eq!(order.replaces, None);
    assert_eq!(
        order.asset_id.as_deref(),
        Some("00000000-0000-0000-0000-000000000000")
    );
    // A multi-leg parent's empty symbol and asset class are kept as sent.
    assert_eq!(order.symbol.as_deref(), Some(""));
    assert_eq!(order.asset_class, Some(Inbound::Unknown(String::new())));
    assert_eq!(order.notional, None);
    assert_eq!(order.qty, Some(dec("1")));
    assert_eq!(order.filled_qty, Some(dec("1")));
    assert_eq!(order.filled_avg_price, Some(dec("1.62")));
    assert_eq!(order.order_class, Some(Inbound::Known(OrderClass::Mleg)));
    assert_eq!(order.order_type, Some(Inbound::Known(OrderType::Limit)));
    assert_eq!(order.r#type, Some(Inbound::Known(OrderType::Limit)));
    assert_eq!(order.side, Some(Inbound::Known(OrderSide::Buy)));
    assert_eq!(order.time_in_force, Inbound::Known(TimeInForce::Day));
    assert_eq!(order.limit_price, Some(dec("2")));
    assert_eq!(order.stop_price, None);
    assert_eq!(order.status, Inbound::Known(OrderStatus::Filled));
    assert!(!order.extended_hours);
    assert_eq!(order.trail_percent, None);
    assert_eq!(order.trail_price, None);
    assert_eq!(order.hwm, None);
    assert_eq!(order.ratio_qty, None);
    assert_eq!(order.position_intent, None);

    let legs = order.legs.as_ref().unwrap();
    assert_eq!(legs.len(), 2);
    let expected = [
        (
            "3cbe69ef-241c-43ba-9d8c-09361930a1af",
            "e868fb88-ce92-442b-91be-4b16defbc883",
            "2025-01-21T07:32:40.697474882Z",
            "2025-01-21T07:32:40.687356797Z",
            "2025-01-21T07:32:40.695564076Z",
            "925af3ed-5c00-4ef1-b89b-e4bd05f04486",
            "AAPL250321P00200000",
            "1.6",
        ),
        (
            "ec694de5-5028-4347-8f89-d8ea00c9341f",
            "0a1bf1e1-6992-4c23-85a6-9469bbe05f1a",
            "2025-01-21T07:32:40.699294952Z",
            "2025-01-21T07:32:40.691562346Z",
            "2025-01-21T07:32:40.695569506Z",
            "9f8c3d65-f5f7-42cd-acbc-9636cc32d3b5",
            "AAPL250321C00380000",
            "0.02",
        ),
    ];
    for (leg, (id, client_id, updated, submitted, filled, asset_id, symbol, average)) in
        legs.iter().zip(expected)
    {
        assert_eq!(leg.id, id);
        assert_eq!(leg.client_order_id, client_id);
        assert_eq!(leg.created_at, at("2025-01-21T07:32:40.678963102Z"));
        assert_eq!(leg.updated_at, Some(at(updated)));
        assert_eq!(leg.submitted_at, Some(at(submitted)));
        assert_eq!(leg.filled_at, Some(at(filled)));
        assert_eq!(leg.expired_at, None);
        assert_eq!(leg.expires_at, None);
        assert_eq!(leg.cancel_requested_at, None);
        assert_eq!(leg.canceled_at, None);
        assert_eq!(leg.failed_at, None);
        assert_eq!(leg.replaced_at, None);
        assert_eq!(leg.replaced_by, None);
        assert_eq!(leg.replaces, None);
        assert_eq!(leg.asset_id.as_deref(), Some(asset_id));
        assert_eq!(leg.symbol.as_deref(), Some(symbol));
        assert_eq!(leg.asset_class, Some(Inbound::Known(AssetClass::UsOption)));
        assert_eq!(leg.notional, None);
        assert_eq!(leg.qty, Some(dec("1")));
        assert_eq!(leg.filled_qty, Some(dec("1")));
        assert_eq!(leg.filled_avg_price, Some(dec(average)));
        assert_eq!(leg.order_class, Some(Inbound::Known(OrderClass::Mleg)));
        // A leg's empty order type is kept as sent.
        assert_eq!(leg.order_type, Some(Inbound::Unknown(String::new())));
        assert_eq!(leg.r#type, Some(Inbound::Unknown(String::new())));
        assert_eq!(leg.side, Some(Inbound::Known(OrderSide::Buy)));
        assert_eq!(leg.time_in_force, Inbound::Known(TimeInForce::Day));
        assert_eq!(leg.limit_price, None);
        assert_eq!(leg.stop_price, None);
        assert_eq!(leg.status, Inbound::Known(OrderStatus::Filled));
        assert!(!leg.extended_hours);
        assert_eq!(leg.legs, None);
        assert_eq!(leg.trail_percent, None);
        assert_eq!(leg.trail_price, None);
        assert_eq!(leg.hwm, None);
        assert_eq!(leg.ratio_qty, Some(dec("1")));
        assert_eq!(leg.position_intent, None);
    }
}

const EVENTS: [(&str, TradeEvent); 17] = [
    ("new", TradeEvent::New),
    ("fill", TradeEvent::Fill),
    ("partial_fill", TradeEvent::PartialFill),
    ("canceled", TradeEvent::Canceled),
    ("expired", TradeEvent::Expired),
    ("done_for_day", TradeEvent::DoneForDay),
    ("replaced", TradeEvent::Replaced),
    ("accepted", TradeEvent::Accepted),
    ("rejected", TradeEvent::Rejected),
    ("pending_new", TradeEvent::PendingNew),
    ("stopped", TradeEvent::Stopped),
    ("pending_cancel", TradeEvent::PendingCancel),
    ("pending_replace", TradeEvent::PendingReplace),
    ("calculated", TradeEvent::Calculated),
    ("suspended", TradeEvent::Suspended),
    ("order_replace_rejected", TradeEvent::OrderReplaceRejected),
    ("order_cancel_rejected", TradeEvent::OrderCancelRejected),
];

#[test]
fn every_event_has_its_documented_text() {
    for (text, event) in EVENTS {
        assert_eq!(event.as_str(), text);
        assert_eq!(event.to_string(), text);
        assert_eq!(text.parse::<TradeEvent>(), Ok(event));
    }
}

#[tokio::test]
async fn every_event_and_an_unknown_one_arrive_as_updates() {
    let frames: Vec<String> = EVENTS
        .iter()
        .map(|(text, _)| minimal(text, ""))
        .chain([minimal("restated", "")])
        .collect();
    let frames: Vec<&str> = frames.iter().map(String::as_str).collect();
    let messages = deliver(&frames).await;
    let expected = EVENTS
        .iter()
        .map(|(_, event)| Inbound::Known(*event))
        .chain([Inbound::Unknown("restated".to_owned())]);
    for (message, expected) in messages.into_iter().zip(expected) {
        let TradeStreamMessage::Update(update) = message else {
            panic!("not a trade update: {message:?}");
        };
        assert_eq!(update.event, expected);
        assert_eq!(update.order.id, "o1");
        assert_eq!(update.order.cancel_requested_at, None);
        assert_eq!(update.timestamp, None);
    }
}

#[tokio::test]
async fn unknown_members_are_ignored() {
    let frame = minimal(
        "fill",
        r#","surprise":1,"legs":[{"qty":"1","surprise":[true]}]"#,
    );
    let update = update(&frame).await;
    assert_eq!(update.legs.unwrap()[0].qty, Some(dec("1")));
}

#[tokio::test]
async fn a_cancel_requested_time_is_read() {
    let frame = r#"{"stream":"trade_updates","data":{"event":"pending_cancel","order":{"id":"o1","client_order_id":"c1","created_at":"2025-01-21T07:32:40Z","cancel_requested_at":"2025-01-21T07:33:00.5Z","status":"pending_cancel","extended_hours":false,"time_in_force":"day"}}}"#;
    let update = update(frame).await;
    assert_eq!(
        update.order.cancel_requested_at,
        Some(at("2025-01-21T07:33:00.5Z"))
    );
}

#[tokio::test]
async fn other_streams_and_unreadable_updates_are_kept_raw() {
    let other = r#"{"stream":"other","data":{"x":1}}"#;
    // No order: the update cannot be read, and the stream goes on.
    let no_order = r#"{"stream":"trade_updates","data":{"event":"fill","price":"1"}}"#;
    let after = minimal("new", "");
    let messages = deliver(&[other, no_order, &after]).await;
    let TradeStreamMessage::Other(message) = &messages[0] else {
        panic!("not another stream's message: {:?}", messages[0]);
    };
    assert_eq!(message.stream, "other");
    assert_eq!(message.data, r#"{"x":1}"#);
    let TradeStreamMessage::Undecoded(message) = &messages[1] else {
        panic!("not kept raw: {:?}", messages[1]);
    };
    assert_eq!(message.stream, "trade_updates");
    assert_eq!(message.data, r#"{"event":"fill","price":"1"}"#);
    assert!(matches!(messages[2], TradeStreamMessage::Update(_)));
}

/// A decimal of 30 significant digits, beyond any binary float.
const LONG: &str = "12345678901234.5678901234567891";

#[tokio::test]
async fn every_decimal_keeps_its_exact_text() {
    let extra = format!(
        r#","price":"{LONG}","qty":"{LONG}","position_qty":"-{LONG}","position_qtys":{{"A":"{LONG}"}},"legs":[{{"qty":"{LONG}","price":"{LONG}"}}]"#
    );
    let update = update(&minimal("partial_fill", &extra)).await;
    assert_eq!(update.price.unwrap().as_str(), LONG);
    assert_eq!(update.qty.unwrap().as_str(), LONG);
    assert_eq!(update.position_qty.unwrap().as_str(), format!("-{LONG}"));
    assert_eq!(update.position_qtys.unwrap()["A"].as_str(), LONG);
    let leg = &update.legs.unwrap()[0];
    assert_eq!(leg.qty.as_ref().unwrap().as_str(), LONG);
    assert_eq!(leg.price.as_ref().unwrap().as_str(), LONG);
}

#[tokio::test]
async fn a_float_or_an_exponent_in_a_decimal_is_kept_raw() {
    // Each decimal field, as a fraction and as an exponent.
    let mut frames = Vec::new();
    for bad in ["1.5", r#""1e5""#] {
        for field in ["price", "qty", "position_qty"] {
            frames.push(minimal("fill", &format!(r#","{field}":{bad}"#)));
        }
        frames.push(minimal(
            "fill",
            &format!(r#","position_qtys":{{"A":{bad}}}"#),
        ));
        for field in ["qty", "price"] {
            frames.push(minimal(
                "fill",
                &format!(r#","legs":[{{"{field}":{bad}}}]"#),
            ));
        }
    }
    // The other causes the contract names: no event, a malformed time.
    frames.push(format!(
        r#"{{"stream":"trade_updates","data":{{"order":{ORDER}}}}}"#
    ));
    frames.push(minimal("fill", r#","timestamp":"2025-01-21 07:32:40""#));
    let texts: Vec<&str> = frames.iter().map(String::as_str).collect();
    for (message, frame) in deliver(&texts).await.into_iter().zip(&frames) {
        let TradeStreamMessage::Undecoded(raw) = &message else {
            panic!("{frame}: {message:?}");
        };
        assert!(frame.contains(&raw.data), "{frame}");
    }
}

#[tokio::test]
async fn a_json_integer_in_a_decimal_keeps_its_digits() {
    let update = update(&minimal("fill", r#","qty":5,"price":-7"#)).await;
    assert_eq!(update.qty.unwrap().as_str(), "5");
    assert_eq!(update.price.unwrap().as_str(), "-7");
}

#[tokio::test]
async fn close_returns_a_waiting_update_typed() {
    let server = WsServer::start(vec![opened(vec![
        Act::Send(Message::text(FILL)),
        Act::ReadAll,
    ])]);
    let config = Config::new(
        Region::US,
        Environment::Paper,
        Credentials::api_key("PKSENTINELKEY0001", "SECRETSENTINELVALUE0001"),
    )
    .with_base_url(&server.url);
    let stream = Client::new(config).unwrap().trade_updates().await.unwrap();
    let (messages, result) = stream.close().await;
    result.unwrap();
    assert_eq!(messages.len(), 1);
    let TradeStreamMessage::Update(update) = &messages[0] else {
        panic!("not a trade update: {:?}", messages[0]);
    };
    assert_eq!(update.event, Inbound::Known(TradeEvent::Fill));
    assert_eq!(update.order.id, "a5be8f5e-fdfa-41f5-a644-7a74fe947a8f");
}

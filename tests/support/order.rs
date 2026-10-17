//! The upstream order fixtures' shared fields, asserted one by one.

#![allow(dead_code, reason = "each test binary uses part of the helper")]

use suri::{AssetClass, Decimal, Inbound, Order, OrderClass, OrderSide, OrderStatus, OrderType};
use suri::{PositionIntent, TimeInForce, Timestamp};

fn dec(text: &str) -> Decimal {
    text.parse().unwrap()
}

fn at(text: &str) -> Timestamp {
    text.parse().unwrap()
}

/// The fields in which the placement fixtures differ.
pub struct Expected {
    pub asset_id: &'static str,
    pub symbol: &'static str,
    pub kind: OrderType,
    pub side: OrderSide,
    pub limit_price: Option<&'static str>,
    pub qty: Option<&'static str>,
    pub notional: Option<&'static str>,
    pub intent: PositionIntent,
}

#[allow(deprecated)]
pub fn assert_fixture_order(order: &Order, expected: &Expected) {
    assert_eq!(order.id, "61e69015-8549-4bfd-b9c3-01e75843f47d");
    assert_eq!(
        order.client_order_id,
        "eb9e2aaa-f71a-4f51-b5b4-52a6c565dad4"
    );
    assert_eq!(order.created_at, at("2021-03-16T18:38:01.942282Z"));
    assert_eq!(order.updated_at, Some(at("2021-03-16T18:38:01.942282Z")));
    assert_eq!(order.submitted_at, Some(at("2021-03-16T18:38:01.937734Z")));
    assert_eq!(order.filled_at, None);
    assert_eq!(order.expired_at, None);
    assert_eq!(order.expires_at, None);
    assert_eq!(order.canceled_at, None);
    assert_eq!(order.failed_at, None);
    assert_eq!(order.replaced_at, None);
    assert_eq!(order.replaced_by, None);
    assert_eq!(order.replaces, None);
    assert_eq!(order.asset_id.as_deref(), Some(expected.asset_id));
    // The fixtures' stray backtick is kept as sent.
    assert_eq!(order.symbol.as_deref(), Some(expected.symbol));
    assert_eq!(
        order.asset_class,
        Some(Inbound::Known(AssetClass::UsEquity))
    );
    assert_eq!(order.notional, expected.notional.map(dec));
    // The placement fixtures send the JSON number 1.
    assert_eq!(order.qty, expected.qty.map(dec));
    assert_eq!(order.filled_qty, Some(dec("0")));
    assert_eq!(order.filled_avg_price, None);
    assert_eq!(order.order_class, Some(Inbound::Known(OrderClass::Simple)));
    assert_eq!(order.order_type, Some(Inbound::Known(expected.kind)));
    assert_eq!(order.r#type, Some(Inbound::Known(expected.kind)));
    assert_eq!(order.side, Some(Inbound::Known(expected.side)));
    assert_eq!(order.time_in_force, Inbound::Known(TimeInForce::Day));
    // Sent as the JSON number 300 where present.
    assert_eq!(order.limit_price, expected.limit_price.map(dec));
    assert_eq!(order.stop_price, None);
    assert_eq!(order.status, Inbound::Known(OrderStatus::Accepted));
    assert!(!order.extended_hours);
    assert_eq!(order.legs, None);
    assert_eq!(order.trail_percent, None);
    assert_eq!(order.trail_price, None);
    assert_eq!(order.hwm, None);
    assert_eq!(order.ratio_qty, None);
    assert_eq!(order.position_intent, Some(Inbound::Known(expected.intent)));
    // commission is a Broker field and is ignored.
}

pub const AAPL_MARKET_BUY: Expected = Expected {
    asset_id: "904837e3-3b76-47ec-b432-046db621571b",
    symbol: "AAPL`",
    kind: OrderType::Market,
    side: OrderSide::Buy,
    limit_price: None,
    qty: Some("1"),
    notional: None,
    intent: PositionIntent::BuyToOpen,
};

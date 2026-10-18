//! Orders: placing them at most once, never retried, and what Alpaca
//! reports of them.

use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::BuildHasher;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use reqwest::Method;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::value::RawValue;

use crate::client::{Endpoint, Request};
use crate::error::{details, sanitise};
use crate::wire::{Sort, wire_enum};
use crate::{AssetClass, Client, Decimal, Error, Id, Inbound, Response, Timestamp};

mod request;

pub use request::{
    Exit, Leg, Legs, MultiLegKind, OrderKind, OrderRequest, OrderShape, ParseLegsError, Size,
    StopLoss, TakeProfit, Trail,
};

wire_enum! {
    /// The side of an order or a fill.
    OrderSide {
        /// Buy.
        Buy => "buy",
        /// Sell.
        Sell => "sell",
    }
}

wire_enum! {
    /// The status of an order.
    OrderStatus {
        /// The order has been received and routed; the usual initial state.
        New => "new",
        /// The order has been partially filled.
        PartiallyFilled => "partially_filled",
        /// The order has been filled; no further updates will occur.
        Filled => "filled",
        /// The order is done executing for the day.
        DoneForDay => "done_for_day",
        /// The order has been canceled; no further updates will occur.
        Canceled => "canceled",
        /// The order has expired; no further updates will occur.
        Expired => "expired",
        /// The order was replaced by another order.
        Replaced => "replaced",
        /// The order is waiting to be canceled.
        PendingCancel => "pending_cancel",
        /// The order is waiting to be replaced.
        PendingReplace => "pending_replace",
        /// The order has been received but not yet routed.
        Accepted => "accepted",
        /// The order has been routed but not yet accepted for execution.
        PendingNew => "pending_new",
        /// The order has been received and accepted for bidding.
        AcceptedForBidding => "accepted_for_bidding",
        /// The order has been stopped; a trade is guaranteed.
        Stopped => "stopped",
        /// The order has been rejected; no further updates will occur.
        Rejected => "rejected",
        /// The order has been suspended and cannot trade.
        Suspended => "suspended",
        /// The order is complete; settlement calculations are pending.
        Calculated => "calculated",
        /// The order is held.
        Held => "held",
    }
}

wire_enum! {
    /// The type of an order.
    OrderType {
        /// A market order.
        Market => "market",
        /// A limit order.
        Limit => "limit",
        /// A stop order.
        Stop => "stop",
        /// A stop-limit order.
        StopLimit => "stop_limit",
        /// A trailing stop order.
        TrailingStop => "trailing_stop",
    }
}

wire_enum! {
    /// How long an order stays live.
    TimeInForce {
        /// For the trading day it is live on.
        Day => "day",
        /// Until canceled.
        Gtc => "gtc",
        /// In the opening auction only.
        Opg => "opg",
        /// In the closing auction only.
        Cls => "cls",
        /// Immediate or cancel: what does not fill at once is canceled.
        Ioc => "ioc",
        /// Fill or kill: the whole quantity fills at once or the order is
        /// canceled.
        Fok => "fok",
    }
}

wire_enum! {
    /// The class of an order.
    OrderClass {
        /// A single order.
        Simple => "simple",
        /// An entry with a take-profit and a stop-loss exit.
        Bracket => "bracket",
        /// One-cancels-other: a take-profit and a stop-loss exit.
        Oco => "oco",
        /// One-triggers-other: an entry with one exit.
        Oto => "oto",
        /// A multi-leg options order.
        Mleg => "mleg",
        /// No class (`""`), which Alpaca treats as simple.
        Empty => "",
    }
}

wire_enum! {
    /// Whether an order opens or closes a position.
    PositionIntent {
        /// Buy to open a long position.
        BuyToOpen => "buy_to_open",
        /// Buy to close a short position.
        BuyToClose => "buy_to_close",
        /// Sell to open a short position.
        SellToOpen => "sell_to_open",
        /// Sell to close a long position.
        SellToClose => "sell_to_close",
    }
}

/// The ID a caller gives an order, readable before the order is sent, so an
/// order whose response was lost can be looked up by it.
///
/// [`OrderRequest::new`] fills one with [`ClientOrderId::generate`]; a caller
/// may parse its own instead, of 1 to 128 characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientOrderId(String);

impl ClientOrderId {
    /// A new random ID in the form of a version 4 UUID, lower case.
    pub fn generate() -> ClientOrderId {
        static COUNT: AtomicU64 = AtomicU64::new(0);
        let count = COUNT.fetch_add(1, Ordering::Relaxed);
        let now = Instant::now();
        let thread = std::thread::current().id();
        // Each RandomState has its own keys, seeded from the operating
        // system's randomness, so the two halves are independent.
        let half = || RandomState::new().hash_one((count, now, thread));
        let mut bits = (u128::from(half()) << 64) | u128::from(half());
        bits = (bits & !(0xf << 76)) | (0x4 << 76);
        bits = (bits & !(0x3 << 62)) | (0x2 << 62);
        let hex = format!("{bits:032x}");
        ClientOrderId(format!(
            "{}-{}-{}-{}-{}",
            &hex[..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..]
        ))
    }

    /// The ID's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ClientOrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for ClientOrderId {
    type Err = ParseClientOrderIdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let kind = if text.is_empty() {
            IdKind::Empty
        } else if text.chars().count() > 128 {
            IdKind::TooLong
        } else {
            return Ok(ClientOrderId(text.to_owned()));
        };
        Err(ParseClientOrderIdError { kind })
    }
}

impl Serialize for ClientOrderId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// The error returned when text cannot be a client order ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseClientOrderIdError {
    kind: IdKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdKind {
    Empty,
    TooLong,
}

impl fmt::Display for ParseClientOrderIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            IdKind::Empty => "client order ID must not be empty",
            IdKind::TooLong => "client order ID must be at most 128 characters",
        })
    }
}

impl std::error::Error for ParseClientOrderIdError {}

/// An order, as Alpaca reports it.
///
/// Prices and quantities are exact [`Decimal`]s. A multi-leg order's parent
/// has no symbol or side, and its legs no type. Fields Alpaca adds later
/// are ignored until suri models them.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Order {
    /// The order ID.
    pub id: String,
    /// The client order ID.
    pub client_order_id: String,
    /// When the order was created.
    pub created_at: Timestamp,
    /// The order's status.
    pub status: Inbound<OrderStatus>,
    /// Whether it may fill in extended hours.
    pub extended_hours: bool,
    /// How long it stays live.
    pub time_in_force: Inbound<TimeInForce>,
    /// When the order was last updated.
    pub updated_at: Option<Timestamp>,
    /// When the order was submitted.
    pub submitted_at: Option<Timestamp>,
    /// When the order filled.
    pub filled_at: Option<Timestamp>,
    /// When the order expired.
    pub expired_at: Option<Timestamp>,
    /// When the order expires.
    pub expires_at: Option<Timestamp>,
    /// When cancellation was requested; sent on the `trade_updates` stream,
    /// absent from REST responses.
    pub cancel_requested_at: Option<Timestamp>,
    /// When the order was canceled.
    pub canceled_at: Option<Timestamp>,
    /// When the order failed.
    pub failed_at: Option<Timestamp>,
    /// When the order was replaced.
    pub replaced_at: Option<Timestamp>,
    /// The ID of the order that replaced it.
    pub replaced_by: Option<String>,
    /// The ID of the order it replaced.
    pub replaces: Option<String>,
    /// The asset ID.
    pub asset_id: Option<String>,
    /// The symbol; absent from a multi-leg order's parent.
    pub symbol: Option<String>,
    /// The asset class.
    pub asset_class: Option<Inbound<AssetClass>>,
    /// The amount in dollars, for an order by amount.
    pub notional: Option<Decimal>,
    /// The quantity, for an order by quantity.
    pub qty: Option<Decimal>,
    /// The quantity filled.
    pub filled_qty: Option<Decimal>,
    /// The average fill price.
    pub filled_avg_price: Option<Decimal>,
    /// The order's class.
    pub order_class: Option<Inbound<OrderClass>>,
    /// The order's type; absent from a multi-leg order's legs.
    pub r#type: Option<Inbound<OrderType>>,
    /// The order's type.
    #[deprecated(note = "use type")]
    pub order_type: Option<Inbound<OrderType>>,
    /// Buy or sell; absent from a multi-leg order's parent.
    pub side: Option<Inbound<OrderSide>>,
    /// The limit price.
    pub limit_price: Option<Decimal>,
    /// The stop price.
    pub stop_price: Option<Decimal>,
    /// A trailing stop's distance in dollars.
    pub trail_price: Option<Decimal>,
    /// A trailing stop's distance in percent.
    pub trail_percent: Option<Decimal>,
    /// A trailing stop's high-water mark.
    pub hwm: Option<Decimal>,
    /// A leg's quantity for each unit of a multi-leg order.
    pub ratio_qty: Option<Decimal>,
    /// Whether the order opens or closes a position.
    pub position_intent: Option<Inbound<PositionIntent>>,
    /// The orders of a bracket, OCO, OTO or multi-leg order.
    pub legs: Option<Vec<Order>>,
}

wire_enum! {
    /// Which orders to list by status.
    OrderQueryStatus {
        /// Open orders.
        Open => "open",
        /// Closed orders.
        Closed => "closed",
        /// Every order.
        All => "all",
    }
}

/// Which orders to list by when they were submitted: a time window, or the
/// orders before or after one order.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum OrderWindow {
    /// Orders submitted after `after` and until `until`.
    Time {
        /// Only orders submitted after this instant.
        after: Option<Timestamp>,
        /// Only orders submitted until this instant.
        until: Option<Timestamp>,
    },
    /// Orders submitted before this order.
    BeforeOrder(Id),
    /// Orders submitted after this order.
    AfterOrder(Id),
}

/// Which orders to list. One page per call.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OrdersQuery {
    /// Open, closed or all orders; open unless set.
    pub status: Option<OrderQueryStatus>,
    /// The most orders on a page.
    pub limit: Option<u32>,
    /// When the orders were submitted.
    pub window: Option<OrderWindow>,
    /// The order of the list.
    pub direction: Option<Sort>,
    /// Whether to nest a multi-leg, bracket, OCO or OTO order's legs in it.
    pub nested: Option<bool>,
    /// Only orders for these symbols.
    pub symbols: Option<Vec<String>>,
    /// Only buy or only sell orders.
    pub side: Option<OrderSide>,
    /// Only orders in these asset classes; every class unless set.
    pub asset_classes: Option<Vec<AssetClass>>,
}

impl OrdersQuery {
    /// A query with Alpaca's defaults.
    pub fn new() -> OrdersQuery {
        OrdersQuery::default()
    }
}

/// The changes to an open order. Only the fields set are sent; the
/// replacing order keeps the rest from the order it replaces.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReplaceOrderRequest {
    /// The new quantity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qty: Option<Decimal>,
    /// The new time in force.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_in_force: Option<TimeInForce>,
    /// The new limit price.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_price: Option<Decimal>,
    /// The new stop price.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_price: Option<Decimal>,
    /// The new trailing distance, in the trailing stop's own unit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trail: Option<Decimal>,
    /// The replacing order's own ID; replace it to use your own.
    pub client_order_id: ClientOrderId,
}

impl ReplaceOrderRequest {
    /// A replacement that changes nothing yet, with a generated client
    /// order ID for the replacing order.
    pub fn new() -> ReplaceOrderRequest {
        ReplaceOrderRequest {
            qty: None,
            time_in_force: None,
            limit_price: None,
            stop_price: None,
            trail: None,
            client_order_id: ClientOrderId::generate(),
        }
    }
}

impl Default for ReplaceOrderRequest {
    fn default() -> ReplaceOrderRequest {
        ReplaceOrderRequest::new()
    }
}

/// What happened to one order when all were cancelled.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct CancelResult {
    /// The order ID.
    pub id: String,
    /// The HTTP status of this order's cancellation.
    pub status: u16,
    /// For a failed cancellation, Alpaca's error code.
    pub code: Option<u64>,
    /// For a failed cancellation, Alpaca's message, scrubbed and bounded as
    /// [`Error::message`] is.
    pub message: Option<String>,
    /// For a successful cancellation, the body sent with it, if any, as
    /// text, scrubbed and bounded the same way.
    pub body: Option<String>,
}

/// One item of a cancel-all response, as sent.
#[derive(Deserialize)]
struct WireCancel {
    id: String,
    status: u16,
    body: Option<Box<RawValue>>,
}

const PLACE_ORDER: Endpoint = Endpoint {
    method: Method::POST,
    template: "/v2/orders",
};

const ORDER_BY_CLIENT_ORDER_ID: Endpoint = Endpoint {
    method: Method::GET,
    template: "/v2/orders:by_client_order_id",
};

const ORDERS: Endpoint = Endpoint {
    method: Method::GET,
    template: "/v2/orders",
};

const ORDER: Endpoint = Endpoint {
    method: Method::GET,
    template: "/v2/orders/{order_id}",
};

const REPLACE_ORDER: Endpoint = Endpoint {
    method: Method::PATCH,
    template: "/v2/orders/{order_id}",
};

const CANCEL_ORDER: Endpoint = Endpoint {
    method: Method::DELETE,
    template: "/v2/orders/{order_id}",
};

const CANCEL_ALL_ORDERS: Endpoint = Endpoint {
    method: Method::DELETE,
    template: "/v2/orders",
};

impl Client {
    /// Places an order (`POST /v2/orders`). It is made once and never
    /// retried. Every error carries the order's client order ID: after an
    /// error that [may have reached Alpaca](Error::may_have_reached_broker),
    /// look the order up by it before doing anything else.
    pub async fn place_order(&self, order: &OrderRequest) -> Result<Response<Order>, Error> {
        let request = Request::new(&PLACE_ORDER, &["v2", "orders"]).json(order);
        self.send_request(request)
            .await
            .map_err(|error| error.with_client_order_id(order.client_order_id.clone()))
    }

    /// Reads an order by its client order ID (`GET
    /// /v2/orders:by_client_order_id`).
    pub async fn order_by_client_order_id(
        &self,
        client_order_id: &ClientOrderId,
    ) -> Result<Response<Order>, Error> {
        let mut request = Request::new(
            &ORDER_BY_CLIENT_ORDER_ID,
            &["v2", "orders:by_client_order_id"],
        );
        request.pair("client_order_id", client_order_id.as_str());
        self.send_request(request).await
    }

    /// Lists one page of orders (`GET /v2/orders`).
    pub async fn orders(&self, query: &OrdersQuery) -> Result<Response<Vec<Order>>, Error> {
        let mut request = Request::new(&ORDERS, &["v2", "orders"]);
        if let Some(status) = &query.status {
            request.pair("status", status.as_str());
        }
        if let Some(limit) = query.limit {
            request.pair("limit", limit.to_string());
        }
        if let Some(OrderWindow::Time { after, until }) = &query.window {
            if let Some(after) = after {
                request.pair("after", after.to_string());
            }
            if let Some(until) = until {
                request.pair("until", until.to_string());
            }
        }
        if let Some(direction) = &query.direction {
            request.pair("direction", direction.as_str());
        }
        if let Some(nested) = query.nested {
            request.pair("nested", nested.to_string());
        }
        if let Some(symbols) = &query.symbols {
            request.pair("symbols", symbols.join(","));
        }
        if let Some(side) = &query.side {
            request.pair("side", side.as_str());
        }
        if let Some(classes) = &query.asset_classes {
            let names: Vec<&str> = classes.iter().map(AssetClass::as_str).collect();
            request.pair("asset_class", names.join(","));
        }
        match &query.window {
            Some(OrderWindow::BeforeOrder(id)) => request.pair("before_order_id", id.as_str()),
            Some(OrderWindow::AfterOrder(id)) => request.pair("after_order_id", id.as_str()),
            _ => {}
        }
        self.send_request(request).await
    }

    /// Reads one order (`GET /v2/orders/{order_id}`); `nested` asks for a
    /// multi-leg, bracket, OCO or OTO order's legs in it.
    pub async fn order(&self, id: &Id, nested: bool) -> Result<Response<Order>, Error> {
        let segments = ["v2", "orders", id.as_str()];
        let mut request = Request::new(&ORDER, &segments);
        if nested {
            request.pair("nested", "true");
        }
        self.send_request(request).await
    }

    /// Replaces an open order (`PATCH /v2/orders/{order_id}`) and returns
    /// the replacing order. It is made once and never retried. Every error
    /// carries the replacing order's client order ID. Success does not
    /// guarantee the replacement: the order may fill first.
    pub async fn replace_order(
        &self,
        id: &Id,
        replace: &ReplaceOrderRequest,
    ) -> Result<Response<Order>, Error> {
        let segments = ["v2", "orders", id.as_str()];
        let request = Request::new(&REPLACE_ORDER, &segments).json(replace);
        self.send_request(request)
            .await
            .map_err(|error| error.with_client_order_id(replace.client_order_id.clone()))
    }

    /// Asks Alpaca to cancel an open order (`DELETE /v2/orders/{order_id}`).
    /// Success means the request was accepted; a 422 `Api` error, that the
    /// order can no longer be cancelled. It is made once and never retried.
    pub async fn cancel_order(&self, id: &Id) -> Result<Response<()>, Error> {
        let segments = ["v2", "orders", id.as_str()];
        let request = Request::new(&CANCEL_ORDER, &segments).no_content();
        self.send_request(request).await
    }

    /// Asks Alpaca to cancel every open order (`DELETE /v2/orders`), with
    /// one result per order. It is made once and never retried.
    pub async fn cancel_all_orders(&self) -> Result<Response<Vec<CancelResult>>, Error> {
        let request = Request::new(&CANCEL_ALL_ORDERS, &["v2", "orders"]);
        let (items, meta) = self
            .send_request::<Vec<WireCancel>>(request)
            .await?
            .into_parts();
        let credentials = self.credentials();
        let results = items
            .into_iter()
            .map(|item| {
                let text = item.body.as_deref().map(RawValue::get);
                let (code, message, body) = if (200..300).contains(&item.status) {
                    let body = text.map(|text| sanitise(text, &credentials));
                    (None, None, body)
                } else {
                    let (code, message) = details(text.map(str::as_bytes), &credentials);
                    (code, message, None)
                };
                CancelResult {
                    id: item.id,
                    status: item.status,
                    code,
                    message,
                    body,
                }
            })
            .collect();
        Ok(Response::new(results, meta))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::ClientOrderId;

    #[test]
    fn generated_ids_are_distinct_version_4_uuids() {
        let ids: HashSet<ClientOrderId> = (0..10_000).map(|_| ClientOrderId::generate()).collect();
        assert_eq!(ids.len(), 10_000);
        for id in &ids {
            let text = id.as_str().as_bytes();
            assert_eq!(text.len(), 36, "{id}");
            for (at, byte) in text.iter().enumerate() {
                match at {
                    8 | 13 | 18 | 23 => assert_eq!(*byte, b'-', "{id}"),
                    14 => assert_eq!(*byte, b'4', "{id}"),
                    19 => assert!(b"89ab".contains(byte), "{id}"),
                    _ => assert!(matches!(byte, b'0'..=b'9' | b'a'..=b'f'), "{id}"),
                }
            }
        }
    }
}

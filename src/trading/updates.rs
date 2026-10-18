//! The `trade_updates` stream's messages, typed: an event and the order it
//! is about.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::Order;
use crate::stream::StreamMessage;
use crate::value::wire::wire_enum;
use crate::{Decimal, Inbound, Timestamp};

wire_enum! {
    /// What happened to an order.
    TradeEvent {
        /// The order was routed to the exchanges.
        New => "new",
        /// The order filled completely.
        Fill => "fill",
        /// Part of the order filled.
        PartialFill => "partial_fill",
        /// A requested cancellation was processed.
        Canceled => "canceled",
        /// The order reached the end of its time in force.
        Expired => "expired",
        /// The order is done for the day.
        DoneForDay => "done_for_day",
        /// A requested replacement was processed.
        Replaced => "replaced",
        /// Alpaca received the order but has not routed it yet.
        Accepted => "accepted",
        /// The order was rejected.
        Rejected => "rejected",
        /// The order was routed but not yet accepted for execution.
        PendingNew => "pending_new",
        /// A trade is guaranteed for the order but has not occurred.
        Stopped => "stopped",
        /// The order is awaiting cancellation.
        PendingCancel => "pending_cancel",
        /// The order is awaiting replacement.
        PendingReplace => "pending_replace",
        /// The order is complete for the day; settlement calculations are
        /// pending.
        Calculated => "calculated",
        /// The order is suspended and not eligible for trading.
        Suspended => "suspended",
        /// A replacement was rejected.
        OrderReplaceRejected => "order_replace_rejected",
        /// A cancellation was rejected.
        OrderCancelRejected => "order_cancel_rejected",
    }
}

/// One `trade_updates` message: the event and the order, with the fields
/// that only some events carry.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TradeUpdate {
    /// What happened.
    pub event: Inbound<TradeEvent>,
    /// The order, as the REST API reports it.
    pub order: Order,
    /// When it happened; sent with a fill, a partial fill, a cancellation,
    /// an expiry, a replacement and a rejection.
    pub timestamp: Option<Timestamp>,
    /// The fill's execution ID.
    pub execution_id: Option<String>,
    /// The price per share of a fill or partial fill.
    pub price: Option<Decimal>,
    /// The quantity of a fill or partial fill.
    pub qty: Option<Decimal>,
    /// The position's size after the event: positive long, negative short.
    pub position_qty: Option<Decimal>,
    /// The `at` time of a multi-leg fill's update, as sent (the docs do not
    /// say what it marks).
    pub at: Option<Timestamp>,
    /// The update's ID; seen on multi-leg fills.
    pub event_id: Option<String>,
    /// Each leg's position size after a multi-leg fill, by symbol.
    pub position_qtys: Option<BTreeMap<String, Decimal>>,
    /// The fills of a multi-leg order's legs.
    pub legs: Option<Vec<LegFill>>,
}

/// The fill of one leg of a multi-leg order.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LegFill {
    /// The fill's execution ID.
    pub execution_id: Option<String>,
    /// The leg's order ID.
    pub order_id: Option<String>,
    /// The leg's symbol.
    pub symbol: Option<String>,
    /// The quantity filled.
    pub qty: Option<Decimal>,
    /// The price per unit.
    pub price: Option<Decimal>,
    /// When the leg filled.
    pub timestamp: Option<Timestamp>,
}

/// A message of the stream that [`Client::trade_updates`](crate::Client::trade_updates)
/// opens.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "each message is moved once, to the caller; a box would cost an allocation per update"
)]
pub enum TradeStreamMessage {
    /// A trade update.
    Update(TradeUpdate),
    /// A `trade_updates` message suri could not read as a trade update, kept
    /// as sent: its order ID is still there to reconcile with.
    Undecoded(StreamMessage),
    /// A message of another stream, as sent.
    Other(StreamMessage),
}

/// Types a stream message: a `trade_updates` message whose `data` reads as
/// a trade update, else the message as sent.
pub(crate) fn typed(message: StreamMessage) -> TradeStreamMessage {
    if message.stream != "trade_updates" {
        return TradeStreamMessage::Other(message);
    }
    match serde_json::from_str(&message.data) {
        Ok(update) => TradeStreamMessage::Update(update),
        Err(_) => TradeStreamMessage::Undecoded(message),
    }
}

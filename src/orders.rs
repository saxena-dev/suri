//! Orders: placing them exactly once, and what Alpaca reports of them.

use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::BuildHasher;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use reqwest::Method;
use serde::{Deserialize, Serialize, Serializer};

use crate::client::{Endpoint, Request};
use crate::wire::wire_enum;
use crate::{AssetClass, Client, Decimal, Error, Inbound, Response, Timestamp};

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

/// How much to trade: a quantity, or an amount of money.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Size {
    /// A number of shares or units, fractional where the asset allows it.
    Qty(Decimal),
    /// An amount in dollars.
    Notional(Decimal),
}

/// An order's type with the prices it needs.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum OrderKind {
    /// At the market price.
    Market,
    /// At this price or better.
    Limit {
        /// The limit price.
        limit_price: Decimal,
    },
    /// A market order once the price reaches the stop.
    Stop {
        /// The stop price.
        stop_price: Decimal,
    },
    /// A limit order once the price reaches the stop.
    StopLimit {
        /// The stop price.
        stop_price: Decimal,
        /// The limit price.
        limit_price: Decimal,
    },
    /// A stop that follows the price at a distance.
    TrailingStop(Trail),
}

/// A trailing stop's distance from the price.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Trail {
    /// In dollars.
    Price(Decimal),
    /// In percent.
    Percent(Decimal),
}

/// A multi-leg order's type: market or limit only.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum MultiLegKind {
    /// At the market price.
    Market,
    /// At this net price or better; a negative price is a credit.
    Limit {
        /// The limit price.
        limit_price: Decimal,
    },
}

/// A take-profit exit: a limit order.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TakeProfit {
    /// The limit price.
    pub limit_price: Decimal,
}

impl TakeProfit {
    /// A take-profit at this limit price.
    pub fn new(limit_price: Decimal) -> TakeProfit {
        TakeProfit { limit_price }
    }
}

/// A stop-loss exit: a stop order, or a stop-limit order when it has a
/// limit price.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StopLoss {
    /// The stop price.
    pub stop_price: Decimal,
    /// The limit price, for a stop-limit exit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_price: Option<Decimal>,
}

impl StopLoss {
    /// A stop-loss at this stop price, with no limit price.
    pub fn new(stop_price: Decimal) -> StopLoss {
        StopLoss {
            stop_price,
            limit_price: None,
        }
    }
}

/// The one exit of a one-triggers-other order.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Exit {
    /// A take-profit.
    TakeProfit(TakeProfit),
    /// A stop-loss.
    StopLoss(StopLoss),
}

/// One leg of a multi-leg order.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Leg {
    /// The option contract's symbol.
    pub symbol: String,
    /// The leg's quantity for each unit of the order's quantity.
    pub ratio_qty: Decimal,
    /// The leg's side.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<OrderSide>,
    /// Whether the leg opens or closes a position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_intent: Option<PositionIntent>,
}

impl Leg {
    /// A leg of this contract and ratio.
    pub fn new(symbol: impl Into<String>, ratio_qty: Decimal) -> Leg {
        Leg {
            symbol: symbol.into(),
            ratio_qty,
            side: None,
            position_intent: None,
        }
    }
}

/// The legs of a multi-leg order: at most four.
#[derive(Debug, Clone, PartialEq)]
pub struct Legs(Vec<Leg>);

impl Legs {
    /// The legs, or an error if there are more than four.
    pub fn new(legs: Vec<Leg>) -> Result<Legs, ParseLegsError> {
        if legs.len() > 4 {
            return Err(ParseLegsError {
                kind: LegsKind::TooMany,
            });
        }
        Ok(Legs(legs))
    }

    /// The legs, in order.
    pub fn as_slice(&self) -> &[Leg] {
        &self.0
    }
}

/// The error returned when a multi-leg order would have too many legs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseLegsError {
    kind: LegsKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegsKind {
    TooMany,
}

impl fmt::Display for ParseLegsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            LegsKind::TooMany => "a multi-leg order takes at most 4 legs",
        })
    }
}

impl std::error::Error for ParseLegsError {}

/// What an order is: its class and the parts that class requires.
///
/// Every part a class requires is a field of its variant, so an order
/// missing one cannot be built. A simple market order:
///
/// ```
/// # use suri::{OrderKind, OrderShape, OrderSide, Size};
/// let shape = OrderShape::Simple {
///     symbol: "AAPL".to_owned(),
///     side: OrderSide::Buy,
///     size: Size::Qty("1".parse()?),
///     kind: OrderKind::Market,
/// };
/// # Ok::<(), suri::ParseDecimalError>(())
/// ```
///
/// Without its kind, it does not compile:
///
/// ```compile_fail
/// # use suri::{OrderKind, OrderShape, OrderSide, Size};
/// let shape = OrderShape::Simple {
///     symbol: "AAPL".to_owned(),
///     side: OrderSide::Buy,
///     size: Size::Qty("1".parse()?),
/// };
/// # Ok::<(), suri::ParseDecimalError>(())
/// ```
///
/// A bracket order, and the same without its stop-loss, which does not
/// compile:
///
/// ```
/// # use suri::{OrderKind, OrderShape, OrderSide, Size, StopLoss, TakeProfit};
/// let shape = OrderShape::Bracket {
///     symbol: "AAPL".to_owned(),
///     side: OrderSide::Buy,
///     size: Size::Qty("1".parse()?),
///     kind: OrderKind::Market,
///     take_profit: TakeProfit::new("200".parse()?),
///     stop_loss: StopLoss::new("150".parse()?),
/// };
/// # Ok::<(), suri::ParseDecimalError>(())
/// ```
///
/// ```compile_fail
/// # use suri::{OrderKind, OrderShape, OrderSide, Size, StopLoss, TakeProfit};
/// let shape = OrderShape::Bracket {
///     symbol: "AAPL".to_owned(),
///     side: OrderSide::Buy,
///     size: Size::Qty("1".parse()?),
///     kind: OrderKind::Market,
///     take_profit: TakeProfit::new("200".parse()?),
/// };
/// # Ok::<(), suri::ParseDecimalError>(())
/// ```
///
/// A one-triggers-other order, and the same without its exit:
///
/// ```
/// # use suri::{Exit, OrderKind, OrderShape, OrderSide, Size, TakeProfit};
/// let shape = OrderShape::Oto {
///     symbol: "AAPL".to_owned(),
///     side: OrderSide::Buy,
///     size: Size::Qty("1".parse()?),
///     kind: OrderKind::Market,
///     exit: Exit::TakeProfit(TakeProfit::new("200".parse()?)),
/// };
/// # Ok::<(), suri::ParseDecimalError>(())
/// ```
///
/// ```compile_fail
/// # use suri::{Exit, OrderKind, OrderShape, OrderSide, Size, TakeProfit};
/// let shape = OrderShape::Oto {
///     symbol: "AAPL".to_owned(),
///     side: OrderSide::Buy,
///     size: Size::Qty("1".parse()?),
///     kind: OrderKind::Market,
/// };
/// # Ok::<(), suri::ParseDecimalError>(())
/// ```
///
/// A one-cancels-other order, and the same without its take-profit:
///
/// ```
/// # use suri::{OrderShape, OrderSide, Size, StopLoss, TakeProfit};
/// let shape = OrderShape::Oco {
///     symbol: "AAPL".to_owned(),
///     side: OrderSide::Sell,
///     size: Size::Qty("1".parse()?),
///     take_profit: TakeProfit::new("200".parse()?),
///     stop_loss: StopLoss::new("150".parse()?),
/// };
/// # Ok::<(), suri::ParseDecimalError>(())
/// ```
///
/// ```compile_fail
/// # use suri::{OrderShape, OrderSide, Size, StopLoss, TakeProfit};
/// let shape = OrderShape::Oco {
///     symbol: "AAPL".to_owned(),
///     side: OrderSide::Sell,
///     size: Size::Qty("1".parse()?),
///     stop_loss: StopLoss::new("150".parse()?),
/// };
/// # Ok::<(), suri::ParseDecimalError>(())
/// ```
///
/// A multi-leg order, and the same without its legs:
///
/// ```
/// # use suri::{Leg, Legs, MultiLegKind, OrderShape};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let shape = OrderShape::MultiLeg {
///     qty: "1".parse()?,
///     kind: MultiLegKind::Market,
///     legs: Legs::new(vec![Leg::new("AAPL241213C00250000", "1".parse()?)])?,
/// };
/// # Ok(())
/// # }
/// ```
///
/// ```compile_fail
/// # use suri::{Leg, Legs, MultiLegKind, OrderShape};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let shape = OrderShape::MultiLeg {
///     qty: "1".parse()?,
///     kind: MultiLegKind::Market,
/// };
/// # Ok(())
/// # }
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum OrderShape {
    /// A single order.
    Simple {
        /// The symbol.
        symbol: String,
        /// Buy or sell.
        side: OrderSide,
        /// How much.
        size: Size,
        /// The type and its prices.
        kind: OrderKind,
    },
    /// An entry order with a take-profit and a stop-loss exit.
    Bracket {
        /// The symbol.
        symbol: String,
        /// Buy or sell.
        side: OrderSide,
        /// How much.
        size: Size,
        /// The entry's type and its prices.
        kind: OrderKind,
        /// The take-profit exit.
        take_profit: TakeProfit,
        /// The stop-loss exit.
        stop_loss: StopLoss,
    },
    /// An entry order that triggers one exit.
    Oto {
        /// The symbol.
        symbol: String,
        /// Buy or sell.
        side: OrderSide,
        /// How much.
        size: Size,
        /// The entry's type and its prices.
        kind: OrderKind,
        /// The exit.
        exit: Exit,
    },
    /// Two exits, one canceling the other when it fills; sent as a limit
    /// order.
    Oco {
        /// The symbol.
        symbol: String,
        /// Buy or sell.
        side: OrderSide,
        /// How much.
        size: Size,
        /// The take-profit exit.
        take_profit: TakeProfit,
        /// The stop-loss exit.
        stop_loss: StopLoss,
    },
    /// A multi-leg options order, with no symbol or side of its own.
    MultiLeg {
        /// The number of units of the strategy.
        qty: Decimal,
        /// Market or limit.
        kind: MultiLegKind,
        /// The legs.
        legs: Legs,
    },
}

/// An order to place.
///
/// Its JSON (`Serialize`) is exactly the body sent. `new` fills
/// `client_order_id` with a generated ID, so it can be read, logged or
/// stored before the order is sent.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct OrderRequest {
    /// What the order is.
    pub shape: OrderShape,
    /// How long it stays live.
    pub time_in_force: TimeInForce,
    /// The ID to look the order up by; replace it to use your own.
    pub client_order_id: ClientOrderId,
    /// Whether it may fill in extended hours.
    pub extended_hours: Option<bool>,
    /// Whether it opens or closes a position.
    pub position_intent: Option<PositionIntent>,
}

impl OrderRequest {
    /// An order of this shape and time in force, with a generated client
    /// order ID.
    pub fn new(shape: OrderShape, time_in_force: TimeInForce) -> OrderRequest {
        OrderRequest {
            shape,
            time_in_force,
            client_order_id: ClientOrderId::generate(),
            extended_hours: None,
            position_intent: None,
        }
    }
}

/// The body of an order placement, in the order Alpaca's docs list it.
#[derive(Serialize)]
struct WireOrder<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    symbol: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    qty: Option<&'a Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notional: Option<&'a Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    side: Option<OrderSide>,
    r#type: OrderType,
    time_in_force: TimeInForce,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit_price: Option<&'a Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_price: Option<&'a Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trail_price: Option<&'a Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trail_percent: Option<&'a Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extended_hours: Option<bool>,
    client_order_id: &'a ClientOrderId,
    #[serde(skip_serializing_if = "Option::is_none")]
    order_class: Option<OrderClass>,
    #[serde(skip_serializing_if = "Option::is_none")]
    take_profit: Option<&'a TakeProfit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_loss: Option<&'a StopLoss>,
    #[serde(skip_serializing_if = "Option::is_none")]
    legs: Option<&'a [Leg]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    position_intent: Option<PositionIntent>,
}

impl<'a> WireOrder<'a> {
    /// Sets the type and the prices it needs.
    fn kind(&mut self, kind: &'a OrderKind) {
        match kind {
            OrderKind::Market => self.r#type = OrderType::Market,
            OrderKind::Limit { limit_price } => {
                self.r#type = OrderType::Limit;
                self.limit_price = Some(limit_price);
            }
            OrderKind::Stop { stop_price } => {
                self.r#type = OrderType::Stop;
                self.stop_price = Some(stop_price);
            }
            OrderKind::StopLimit {
                stop_price,
                limit_price,
            } => {
                self.r#type = OrderType::StopLimit;
                self.stop_price = Some(stop_price);
                self.limit_price = Some(limit_price);
            }
            OrderKind::TrailingStop(trail) => {
                self.r#type = OrderType::TrailingStop;
                match trail {
                    Trail::Price(price) => self.trail_price = Some(price),
                    Trail::Percent(percent) => self.trail_percent = Some(percent),
                }
            }
        }
    }

    /// Sets the symbol, size and side shared by every class but multi-leg.
    fn single(&mut self, symbol: &'a str, side: OrderSide, size: &'a Size) {
        self.symbol = Some(symbol);
        self.side = Some(side);
        match size {
            Size::Qty(qty) => self.qty = Some(qty),
            Size::Notional(notional) => self.notional = Some(notional),
        }
    }
}

impl Serialize for OrderRequest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut wire = WireOrder {
            symbol: None,
            qty: None,
            notional: None,
            side: None,
            r#type: OrderType::Market,
            time_in_force: self.time_in_force,
            limit_price: None,
            stop_price: None,
            trail_price: None,
            trail_percent: None,
            extended_hours: self.extended_hours,
            client_order_id: &self.client_order_id,
            order_class: None,
            take_profit: None,
            stop_loss: None,
            legs: None,
            position_intent: self.position_intent,
        };
        match &self.shape {
            OrderShape::Simple {
                symbol,
                side,
                size,
                kind,
            } => {
                wire.single(symbol, *side, size);
                wire.kind(kind);
            }
            OrderShape::Bracket {
                symbol,
                side,
                size,
                kind,
                take_profit,
                stop_loss,
            } => {
                wire.single(symbol, *side, size);
                wire.kind(kind);
                wire.order_class = Some(OrderClass::Bracket);
                wire.take_profit = Some(take_profit);
                wire.stop_loss = Some(stop_loss);
            }
            OrderShape::Oto {
                symbol,
                side,
                size,
                kind,
                exit,
            } => {
                wire.single(symbol, *side, size);
                wire.kind(kind);
                wire.order_class = Some(OrderClass::Oto);
                match exit {
                    Exit::TakeProfit(take_profit) => wire.take_profit = Some(take_profit),
                    Exit::StopLoss(stop_loss) => wire.stop_loss = Some(stop_loss),
                }
            }
            OrderShape::Oco {
                symbol,
                side,
                size,
                take_profit,
                stop_loss,
            } => {
                wire.single(symbol, *side, size);
                wire.r#type = OrderType::Limit;
                wire.order_class = Some(OrderClass::Oco);
                wire.take_profit = Some(take_profit);
                wire.stop_loss = Some(stop_loss);
            }
            OrderShape::MultiLeg { qty, kind, legs } => {
                wire.qty = Some(qty);
                if let MultiLegKind::Limit { limit_price } = kind {
                    wire.r#type = OrderType::Limit;
                    wire.limit_price = Some(limit_price);
                }
                wire.order_class = Some(OrderClass::Mleg);
                wire.legs = Some(legs.as_slice());
            }
        }
        wire.serialize(serializer)
    }
}

/// An order, as Alpaca reports it.
///
/// Prices and quantities are exact [`Decimal`]s. A multi-leg order's parent
/// has no symbol or side, and its legs no type. Fields Alpaca adds later
/// are ignored until suri models them.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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

const PLACE_ORDER: Endpoint = Endpoint {
    method: Method::POST,
    template: "/v2/orders",
};

const ORDER_BY_CLIENT_ORDER_ID: Endpoint = Endpoint {
    method: Method::GET,
    template: "/v2/orders:by_client_order_id",
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

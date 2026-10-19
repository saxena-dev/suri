//! What an order placement sends: the order's shape, its parts and its
//! exact wire form.

use std::fmt;

use serde::{Serialize, Serializer};

use super::{ClientOrderId, OrderClass, OrderSide, OrderType, PositionIntent, TimeInForce};
use crate::Decimal;

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
/// # use suri::trading::{OrderKind, OrderShape, OrderSide, Size};
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
/// # use suri::trading::{OrderKind, OrderShape, OrderSide, Size};
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
/// # use suri::trading::{OrderKind, OrderShape, OrderSide, Size, StopLoss, TakeProfit};
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
/// # use suri::trading::{OrderKind, OrderShape, OrderSide, Size, StopLoss, TakeProfit};
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
/// # use suri::trading::{Exit, OrderKind, OrderShape, OrderSide, Size, TakeProfit};
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
/// # use suri::trading::{Exit, OrderKind, OrderShape, OrderSide, Size, TakeProfit};
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
/// # use suri::trading::{OrderShape, OrderSide, Size, StopLoss, TakeProfit};
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
/// # use suri::trading::{OrderShape, OrderSide, Size, StopLoss, TakeProfit};
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
/// # use suri::trading::{Leg, Legs, MultiLegKind, OrderShape};
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
/// # use suri::trading::{Leg, Legs, MultiLegKind, OrderShape};
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
    /// Two exits, one cancelling the other when it fills; sent as a limit
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

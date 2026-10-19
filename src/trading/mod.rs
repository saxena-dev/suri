//! The Trading API: your account, orders and positions, the assets and
//! option contracts you can trade, the market clock and calendar,
//! watchlists, and the `trade_updates` stream.
//!
//! Every call is a method on [`Client`](crate::Client); this module holds
//! the types those methods take and return.
//!
//! | To… | Call |
//! |---|---|
//! | read the account | [`account`], with [`account_configurations`] and [`update_account_configurations`] for its settings |
//! | read what happened to the account | [`account_activities`] and [`portfolio_history`] |
//! | place an order | [`place_order`] with an [`OrderRequest`] |
//! | find orders | [`orders`], [`order`] and [`order_by_client_order_id`] |
//! | change or cancel orders | [`replace_order`], [`cancel_order`] and [`cancel_all_orders`] |
//! | read or close positions | [`positions`], [`position`], [`close_position`] and [`close_all_positions`] |
//! | exercise an option, or decline to | [`exercise_option`] and [`decline_exercise`] |
//! | look up what you can trade | [`assets`], [`asset`], [`option_contracts`] and [`option_contract`] |
//! | know when the market is open | [`clock`] and [`calendar`] |
//! | keep watchlists | [`watchlists`], [`watchlist`], [`create_watchlist`], [`update_watchlist`], [`add_to_watchlist`], [`remove_from_watchlist`] and [`delete_watchlist`] |
//! | follow your orders as they change | [`trade_updates`], with the `stream` feature |
//!
//! # Orders, safely
//!
//! suri sends an order at most once: no status, failed connection or
//! timeout makes it send the order again, because a lost response does not
//! mean a lost order. [`OrderRequest::new`] gives every order a
//! [`ClientOrderId`] before it is sent, so you can store it first. When a
//! placement fails and [`may_have_reached_broker`] is true, look the order
//! up by that ID before you decide anything: an order means it was placed,
//! and a 404 means Alpaca had no order with that ID when it answered.
//!
//! ```no_run
//! use suri::trading::{OrderKind, OrderRequest, OrderShape, OrderSide, Size, TimeInForce};
//!
//! # async fn run(client: suri::Client) -> Result<(), Box<dyn std::error::Error>> {
//! let shape = OrderShape::Simple {
//!     symbol: "AAPL".to_owned(),
//!     side: OrderSide::Buy,
//!     size: Size::Qty("1".parse()?),
//!     kind: OrderKind::Market,
//! };
//! let request = OrderRequest::new(shape, TimeInForce::Day);
//! // Keep the ID before sending: it is how you find the order again.
//! let id = request.client_order_id.clone();
//! match client.place_order(&request).await {
//!     Ok(placed) => println!("placed {}", placed.data().id),
//!     Err(error) if error.may_have_reached_broker() => {
//!         // Do not send it again: find out whether it was placed.
//!         match client.order_by_client_order_id(&id).await {
//!             Ok(found) => println!("placed as {}", found.data().id),
//!             Err(lookup) if lookup.status() == Some(404) => {
//!                 println!("Alpaca had no order {id} when it answered");
//!             }
//!             // Still unknown: the lookup failed too.
//!             Err(lookup) => return Err(lookup.into()),
//!         }
//!     }
//!     Err(error) => return Err(error.into()),
//! }
//! # Ok(()) }
//! ```
//!
//! [`account`]: crate::Client::account
//! [`account_configurations`]: crate::Client::account_configurations
//! [`update_account_configurations`]: crate::Client::update_account_configurations
//! [`account_activities`]: crate::Client::account_activities
//! [`portfolio_history`]: crate::Client::portfolio_history
//! [`place_order`]: crate::Client::place_order
//! [`orders`]: crate::Client::orders
//! [`order`]: crate::Client::order
//! [`order_by_client_order_id`]: crate::Client::order_by_client_order_id
//! [`replace_order`]: crate::Client::replace_order
//! [`cancel_order`]: crate::Client::cancel_order
//! [`cancel_all_orders`]: crate::Client::cancel_all_orders
//! [`positions`]: crate::Client::positions
//! [`position`]: crate::Client::position
//! [`close_position`]: crate::Client::close_position
//! [`close_all_positions`]: crate::Client::close_all_positions
//! [`exercise_option`]: crate::Client::exercise_option
//! [`decline_exercise`]: crate::Client::decline_exercise
//! [`assets`]: crate::Client::assets
//! [`asset`]: crate::Client::asset
//! [`option_contracts`]: crate::Client::option_contracts
//! [`option_contract`]: crate::Client::option_contract
//! [`clock`]: crate::Client::clock
//! [`calendar`]: crate::Client::calendar
//! [`watchlists`]: crate::Client::watchlists
//! [`watchlist`]: crate::Client::watchlist
//! [`create_watchlist`]: crate::Client::create_watchlist
//! [`update_watchlist`]: crate::Client::update_watchlist
//! [`add_to_watchlist`]: crate::Client::add_to_watchlist
//! [`remove_from_watchlist`]: crate::Client::remove_from_watchlist
//! [`delete_watchlist`]: crate::Client::delete_watchlist
//! [`trade_updates`]: crate::Client::trade_updates
//! [`may_have_reached_broker`]: crate::Error::may_have_reached_broker

mod account;
mod assets;
mod calendar;
mod clock;
mod orders;
mod positions;
#[cfg(feature = "stream")]
mod updates;
mod watchlists;

pub use account::{
    Account, AccountConfigurations, AccountStatus, ActivitiesQuery, Activity, ActivityCategory,
    ActivityFilter, ActivityStatus, ActivityType, ConfigurationsUpdate, FillType, HistoryRange,
    IntradayReporting, NonTradeActivity, PnlReset, PortfolioHistory, PortfolioHistoryQuery,
    TradeActivity, TradeConfirmEmail,
};
pub use assets::{
    Asset, AssetAttribute, AssetClass, AssetStatus, AssetsQuery, BorrowStatus, ContractType,
    Deliverable, DeliverableType, Exchange, ExerciseStyle, OptionContract, OptionContracts,
    OptionContractsQuery, SettlementMethod, SettlementType,
};
pub use calendar::{CalendarDateType, CalendarDay, CalendarQuery, ClockTime};
pub use clock::Clock;
pub use orders::{
    CancelResult, ClientOrderId, Exit, Leg, Legs, MultiLegKind, Order, OrderClass, OrderKind,
    OrderQueryStatus, OrderRequest, OrderShape, OrderSide, OrderStatus, OrderType, OrderWindow,
    OrdersQuery, ParseClientOrderIdError, ParseLegsError, PositionIntent, ReplaceOrderRequest,
    Size, StopLoss, TakeProfit, TimeInForce, Trail,
};
pub use positions::{ClosePosition, ClosedPosition, Position, PositionSide, UsdPositionValues};
#[cfg(feature = "stream")]
pub use updates::{
    LegFill, StreamMessage, TradeEvent, TradeStreamMessage, TradeUpdate, TradeUpdates,
};
pub use watchlists::{CreateWatchlist, Watchlist, WatchlistUpdate};

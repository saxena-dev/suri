//! The Trading API: account, assets, calendar, clock, orders, positions,
//! watchlists, and the `trade_updates` stream.
//!
//! The methods that return these types are on `Client`: `account`,
//! `account_configurations`, `update_account_configurations`,
//! `account_activities` and `portfolio_history`; `assets`, `asset`,
//! `option_contracts` and `option_contract`; `calendar` and `clock`;
//! `place_order`, `order_by_client_order_id`, `orders`, `order`,
//! `replace_order`, `cancel_order` and `cancel_all_orders`; `positions`,
//! `position`, `close_position`, `close_all_positions`, `exercise_option`
//! and `decline_exercise`; `watchlists`, `watchlist`, `create_watchlist`,
//! `update_watchlist`, `add_to_watchlist`, `remove_from_watchlist` and
//! `delete_watchlist`; and, with the `stream` feature, `trade_updates`.

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

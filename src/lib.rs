//! A Rust client for the Alpaca Trading and Market Data APIs.
//!
//! Pre-release: the market clock and calendar, the account's endpoints,
//! assets, option contracts and orders are the only ones yet.
//!
//! The [contract](https://github.com/saxena-dev/suri/blob/main/docs/contract.md)
//! states every behaviour suri promises.

#[cfg(feature = "rest")]
mod account;
#[cfg(feature = "rest")]
mod assets;
#[cfg(feature = "rest")]
mod calendar;
#[cfg(feature = "rest")]
mod client;
#[cfg(feature = "rest")]
mod clock;
#[cfg(feature = "rest")]
mod config;
mod decimal;
#[cfg(feature = "rest")]
mod error;
mod inbound;
#[cfg(feature = "rest")]
mod orders;
mod path;
#[cfg(feature = "rest")]
mod response;
mod time;
#[cfg(feature = "rest")]
mod wire;

#[cfg(feature = "rest")]
pub use account::{
    Account, AccountConfigurations, AccountStatus, ActivitiesQuery, Activity, ActivityCategory,
    ActivityFilter, ActivityStatus, ActivityType, ConfigurationsUpdate, FillType, HistoryRange,
    IntradayReporting, NonTradeActivity, PnlReset, PortfolioHistory, PortfolioHistoryQuery,
    TradeActivity, TradeConfirmEmail,
};
#[cfg(feature = "rest")]
pub use assets::{
    Asset, AssetAttribute, AssetClass, AssetStatus, AssetsQuery, BorrowStatus, ContractType,
    Deliverable, DeliverableType, Exchange, ExerciseStyle, OptionContract, OptionContracts,
    OptionContractsQuery, SettlementMethod, SettlementType,
};
#[cfg(feature = "rest")]
pub use calendar::{CalendarDateType, CalendarDay, CalendarQuery, ClockTime};
#[cfg(feature = "rest")]
pub use client::Client;
#[cfg(feature = "rest")]
pub use clock::Clock;
#[cfg(feature = "rest")]
pub use config::{Config, ConfigError, Credentials, Environment, Region};
pub use decimal::{Decimal, ParseDecimalError};
#[cfg(feature = "rest")]
pub use error::{Error, ErrorKind, Stage};
pub use inbound::Inbound;
#[cfg(feature = "rest")]
pub use orders::{
    CancelResult, ClientOrderId, Exit, Leg, Legs, MultiLegKind, Order, OrderClass, OrderKind,
    OrderQueryStatus, OrderRequest, OrderShape, OrderSide, OrderStatus, OrderType, OrderWindow,
    OrdersQuery, ParseClientOrderIdError, ParseLegsError, PositionIntent, ReplaceOrderRequest,
    Size, StopLoss, TakeProfit, TimeInForce, Trail,
};
pub use path::{Id, ParsePathValueError, Symbol};
#[cfg(feature = "rest")]
pub use response::{RateLimit, Response, ResponseMeta};
pub use time::{Date, ParseTimeError, Timestamp};
#[cfg(feature = "rest")]
pub use wire::{ParseEnumError, Sort};

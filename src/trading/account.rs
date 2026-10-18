//! The trading account.

use std::collections::BTreeMap;
use std::fmt;

use reqwest::Method;
use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize};

use super::orders::{OrderSide, OrderStatus};
use crate::rest::client::{Endpoint, Request};
use crate::value::wire::{Sort, wire_enum};
use crate::{Client, Date, Decimal, Error, Id, Inbound, Response, Timestamp};

wire_enum! {
    /// The status of an account.
    AccountStatus {
        /// The account is inactive.
        Inactive => "INACTIVE",
        /// The account can trade on paper only.
        PaperOnly => "PAPER_ONLY",
        /// The account is onboarding.
        Onboarding => "ONBOARDING",
        /// The account application submission failed.
        SubmissionFailed => "SUBMISSION_FAILED",
        /// The account application has been submitted for review.
        Submitted => "SUBMITTED",
        /// The account information is being updated.
        AccountUpdated => "ACCOUNT_UPDATED",
        /// The final account approval is pending.
        ApprovalPending => "APPROVAL_PENDING",
        /// The account is active for trading.
        Active => "ACTIVE",
        /// The account application has been rejected.
        Rejected => "REJECTED",
        /// The account is closed.
        AccountClosed => "ACCOUNT_CLOSED",
        /// The account is approved.
        Approved => "APPROVED",
        /// The account's closure is pending.
        AccountClosedPending => "ACCOUNT_CLOSED_PENDING",
        /// The account needs an action from its holder.
        ActionRequired => "ACTION_REQUIRED",
        /// The account is limited.
        Limited => "LIMITED",
    }
}

/// The trading account: its status, balances and restrictions.
///
/// Money and quantities are exact [`Decimal`]s. Only `id` and `status` are
/// always present; every other field is `None` when the response leaves it
/// out. Fields Alpaca adds later are ignored until suri models them.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Account {
    /// The account ID.
    pub id: String,
    /// The account's status.
    pub status: Inbound<AccountStatus>,
    /// The account number.
    pub account_number: Option<String>,
    /// The account's crypto trading status.
    pub crypto_status: Option<Inbound<AccountStatus>>,
    /// The account's currency, such as `USD`.
    pub currency: Option<String>,
    /// When the account was created.
    pub created_at: Option<Timestamp>,
    /// The date of the snapshot the `last_*` fields belong to, as sent.
    pub balance_asof: Option<String>,
    /// Whether the account is prohibited from trading activity.
    pub account_blocked: Option<bool>,
    /// Whether the account may short.
    pub shorting_enabled: Option<bool>,
    /// Whether the holder has suspended trading.
    pub trade_suspended_by_user: Option<bool>,
    /// Whether the account may not place orders.
    pub trading_blocked: Option<bool>,
    /// Whether the account may not request transfers.
    pub transfers_blocked: Option<bool>,
    /// Fees collected.
    pub accrued_fees: Option<Decimal>,
    /// Current buying power.
    pub buying_power: Option<Decimal>,
    /// Cash balance; negative when borrowed.
    pub cash: Option<Decimal>,
    /// Cash plus the market value of long and short positions.
    pub equity: Option<Decimal>,
    /// Reg T initial margin requirement.
    pub initial_margin: Option<Decimal>,
    /// Intraday adjustments by non-trade activities.
    pub intraday_adjustments: Option<Decimal>,
    /// Equity at the previous trading day's close.
    pub last_equity: Option<Decimal>,
    /// Maintenance margin requirement at the previous trading day's close.
    pub last_maintenance_margin: Option<Decimal>,
    /// Market value of long positions.
    pub long_market_value: Option<Decimal>,
    /// Maintenance margin requirement.
    pub maintenance_margin: Option<Decimal>,
    /// Buying power multiplier, as sent (`1`, `2` or `4`).
    pub multiplier: Option<Decimal>,
    /// Buying power without margin.
    pub non_marginable_buying_power: Option<Decimal>,
    /// Buying power for options.
    pub options_buying_power: Option<Decimal>,
    /// Pending regulatory fees.
    pub pending_reg_taf_fees: Option<Decimal>,
    /// Cash pending transfer in.
    pub pending_transfer_in: Option<Decimal>,
    /// Cash pending transfer out.
    pub pending_transfer_out: Option<Decimal>,
    /// Total value of cash and positions.
    #[deprecated(note = "Alpaca deprecates this field; it equals `equity`")]
    pub portfolio_value: Option<Decimal>,
    /// Reg T buying power.
    pub regt_buying_power: Option<Decimal>,
    /// Market value of short positions.
    pub short_market_value: Option<Decimal>,
    /// Special memorandum account value.
    pub sma: Option<Decimal>,
    /// The options trading level Alpaca approved (`0` to `3` today).
    #[serde(default, deserialize_with = "int_or_digits")]
    pub options_approved_level: Option<u8>,
    /// The options trading level in effect (`0` to `3` today).
    #[serde(default, deserialize_with = "int_or_digits")]
    pub options_trading_level: Option<u8>,
}

/// Reads a small integer sent either as a JSON integer or as a string of
/// digits, and `null` as `None`.
pub(crate) fn int_or_digits<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u8>, D::Error> {
    deserializer.deserialize_any(IntOrDigits)
}

struct IntOrDigits;

impl Visitor<'_> for IntOrDigits {
    type Value = Option<u8>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an integer from 0 to 255, as a number or a string of digits")
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Option<u8>, E> {
        u8::try_from(value)
            .map(Some)
            .map_err(|_| E::invalid_value(de::Unexpected::Unsigned(value), &self))
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Option<u8>, E> {
        let digits = !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
        digits
            .then(|| text.parse::<u8>().ok())
            .flatten()
            .map(Some)
            .ok_or_else(|| E::invalid_value(de::Unexpected::Str(text), &self))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Option<u8>, E> {
        Ok(None)
    }
}

const ACCOUNT: Endpoint = Endpoint {
    method: Method::GET,
    template: "/v2/account",
};

impl Client {
    /// Reads the trading account (`GET /v2/account`).
    pub async fn account(&self) -> Result<Response<Account>, Error> {
        self.send(&ACCOUNT, &["v2", "account"]).await
    }
}

wire_enum! {
    /// Whether order fills are confirmed by email.
    TradeConfirmEmail {
        /// Every fill is confirmed by email.
        All => "all",
        /// No fill is confirmed by email.
        None => "none",
    }
}

/// The account's configuration. Every field is optional: Alpaca's schema
/// requires none.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AccountConfigurations {
    /// Whether overnight trading is disabled.
    pub disable_overnight_trading: Option<bool>,
    /// Whether the account may trade fractional quantities.
    pub fractional_trading: Option<bool>,
    /// The highest margin multiplier, as sent (`1`, `2` or `4`).
    pub max_margin_multiplier: Option<Decimal>,
    /// The highest options trading level the account asks for (`0` to `3`
    /// today).
    #[serde(default, deserialize_with = "int_or_digits")]
    pub max_options_trading_level: Option<u8>,
    /// Whether the account trades long only.
    pub no_shorting: Option<bool>,
    /// Whether orders for publicly traded partnerships are accepted without
    /// an exception.
    pub ptp_no_exception_entry: Option<bool>,
    /// Whether new orders are blocked.
    pub suspend_trade: Option<bool>,
    /// Whether order fills are confirmed by email.
    pub trade_confirm_email: Option<Inbound<TradeConfirmEmail>>,
}

/// A change to the account's configuration: only the fields set are sent,
/// and a field left `None` keeps its current value.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct ConfigurationsUpdate {
    /// Disable overnight trading.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_overnight_trading: Option<bool>,
    /// Allow fractional quantities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fractional_trading: Option<bool>,
    /// The highest margin multiplier (`1`, `2` or `4`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_margin_multiplier: Option<Decimal>,
    /// The highest options trading level (`0` to `3`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_options_trading_level: Option<u8>,
    /// Trade long only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_shorting: Option<bool>,
    /// Accept orders for publicly traded partnerships without an exception.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ptp_no_exception_entry: Option<bool>,
    /// Block new orders.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suspend_trade: Option<bool>,
    /// Confirm order fills by email.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trade_confirm_email: Option<TradeConfirmEmail>,
}

impl ConfigurationsUpdate {
    /// An update that changes nothing until fields are set.
    pub fn new() -> ConfigurationsUpdate {
        ConfigurationsUpdate::default()
    }
}

wire_enum! {
    /// The type of an account activity. `FILL` is a trade; every other type
    /// is a non-trade activity.
    ActivityType {
        /// An order fill.
        Fill => "FILL",
        /// Cash transactions (deposits and withdrawals).
        Trans => "TRANS",
        /// Every type except those in TRANS, DIV and FILL.
        Misc => "MISC",
        /// ACATS in (cash).
        Acatc => "ACATC",
        /// ACATS in (securities).
        Acats => "ACATS",
        /// Crypto fee.
        Cfee => "CFEE",
        /// Capital gain distribution.
        Cgd => "CGD",
        /// Cash deposit.
        Csd => "CSD",
        /// Cash withdrawal.
        Csw => "CSW",
        /// Dividend.
        Div => "DIV",
        /// Dividend (capital gain long term).
        Divcgl => "DIVCGL",
        /// Dividend (capital gain short term).
        Divcgs => "DIVCGS",
        /// Dividend fee.
        Divfee => "DIVFEE",
        /// Dividend adjusted (foreign tax withheld).
        Divft => "DIVFT",
        /// Dividend adjusted (NRA withheld).
        Divnra => "DIVNRA",
        /// Dividend return of capital.
        Divroc => "DIVROC",
        /// Dividend adjusted (tefra withheld).
        Divtw => "DIVTW",
        /// Dividend (tax exempt).
        Divtxex => "DIVTXEX",
        /// Fee.
        Fee => "FEE",
        /// Interest (credit or margin).
        Int => "INT",
        /// Interest adjusted (NRA withheld).
        Intnra => "INTNRA",
        /// Interest adjusted (tefra withheld).
        Inttw => "INTTW",
        /// Journal entry.
        Jnl => "JNL",
        /// Journal entry (cash).
        Jnlc => "JNLC",
        /// Journal entry (stock).
        Jnls => "JNLS",
        /// Merger or acquisition.
        Ma => "MA",
        /// Name change.
        Nc => "NC",
        /// Option assignment.
        Opasn => "OPASN",
        /// Option corporate action.
        Opca => "OPCA",
        /// Option cash settlement.
        Opcsh => "OPCSH",
        /// Option exercise.
        Opexc => "OPEXC",
        /// Option expiration.
        Opexp => "OPEXP",
        /// Option trade.
        Optrd => "OPTRD",
        /// Pass-through charge.
        Ptc => "PTC",
        /// Pass-through rebate.
        Ptr => "PTR",
        /// Reorganisation.
        Reo => "REO",
        /// Reorganisation (with a cash or stock component).
        Reorg => "REORG",
        /// Spin-off.
        Spin => "SPIN",
        /// Stock split.
        Split => "SPLIT",
        /// Free of payment transfer.
        Fopt => "FOPT",
        /// On-chain transaction.
        Oct => "OCT",
    }
}

wire_enum! {
    /// Whether a fill completed its order.
    FillType {
        /// The fill completed the order.
        Fill => "fill",
        /// The order is partly filled.
        PartialFill => "partial_fill",
    }
}

wire_enum! {
    /// The status of a non-trade activity.
    ActivityStatus {
        /// The activity was executed.
        Executed => "executed",
        /// The activity corrects an earlier one.
        Correct => "correct",
        /// The activity was canceled.
        Canceled => "canceled",
    }
}

wire_enum! {
    /// A group of activity types to list.
    ActivityCategory {
        /// Fills.
        TradeActivity => "trade_activity",
        /// Everything but fills.
        NonTradeActivity => "non_trade_activity",
    }
}

/// An account activity: a fill, or anything else that changed the account.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Activity {
    /// A fill (`activity_type` is `FILL`).
    Trade(TradeActivity),
    /// Any other activity, such as a dividend, a fee or a transfer.
    NonTrade(NonTradeActivity),
}

/// A fill.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct TradeActivity {
    /// The activity ID; pass the last one on a page as the next page's token.
    pub id: String,
    /// The activity type, `FILL`.
    pub activity_type: Inbound<ActivityType>,
    /// The order's filled quantity so far.
    pub cum_qty: Option<Decimal>,
    /// The order's quantity left to fill.
    pub leaves_qty: Option<Decimal>,
    /// The fill's price per share.
    pub price: Option<Decimal>,
    /// The fill's quantity.
    pub qty: Option<Decimal>,
    /// The filled order's ID.
    pub order_id: Option<String>,
    /// The symbol.
    pub symbol: Option<String>,
    /// The order's status after the fill.
    pub order_status: Option<Inbound<OrderStatus>>,
    /// The side.
    pub side: Option<Inbound<OrderSide>>,
    /// When the fill happened.
    pub transaction_time: Option<Timestamp>,
    /// Whether the fill completed the order.
    pub r#type: Option<Inbound<FillType>>,
}

/// An activity other than a fill.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct NonTradeActivity {
    /// The activity ID; pass the last one on a page as the next page's token.
    pub id: String,
    /// The activity type.
    pub activity_type: Inbound<ActivityType>,
    /// The sub-type, such as `SDIV` for a dividend, as sent.
    pub activity_sub_type: Option<String>,
    /// When the activity was created.
    pub created_at: Option<Timestamp>,
    /// The currency.
    pub currency: Option<String>,
    /// The CUSIP, where one applies.
    pub cusip: Option<String>,
    /// Links sibling activities.
    pub group_id: Option<String>,
    /// The symbol, where one applies.
    pub symbol: Option<String>,
    /// The date the activity occurred or settled.
    pub date: Option<Date>,
    /// The net amount of money, positive or negative.
    pub net_amount: Option<Decimal>,
    /// The amount per share (dividends).
    pub per_share_amount: Option<Decimal>,
    /// The number of shares (dividends).
    pub qty: Option<Decimal>,
    /// The activity's status.
    pub status: Option<Inbound<ActivityStatus>>,
}

/// Both activity shapes in one record, decoded straight from the bytes and
/// then split by `activity_type`.
#[derive(Deserialize)]
struct WireActivity {
    id: String,
    activity_type: Inbound<ActivityType>,
    cum_qty: Option<Decimal>,
    leaves_qty: Option<Decimal>,
    price: Option<Decimal>,
    qty: Option<Decimal>,
    order_id: Option<String>,
    symbol: Option<String>,
    order_status: Option<Inbound<OrderStatus>>,
    side: Option<Inbound<OrderSide>>,
    transaction_time: Option<Timestamp>,
    r#type: Option<Inbound<FillType>>,
    activity_sub_type: Option<String>,
    created_at: Option<Timestamp>,
    currency: Option<String>,
    cusip: Option<String>,
    group_id: Option<String>,
    date: Option<Date>,
    net_amount: Option<Decimal>,
    per_share_amount: Option<Decimal>,
    status: Option<Inbound<ActivityStatus>>,
}

impl<'de> Deserialize<'de> for Activity {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = WireActivity::deserialize(deserializer)?;
        Ok(match wire.activity_type {
            Inbound::Known(ActivityType::Fill) => Activity::Trade(TradeActivity {
                id: wire.id,
                activity_type: wire.activity_type,
                cum_qty: wire.cum_qty,
                leaves_qty: wire.leaves_qty,
                price: wire.price,
                qty: wire.qty,
                order_id: wire.order_id,
                symbol: wire.symbol,
                order_status: wire.order_status,
                side: wire.side,
                transaction_time: wire.transaction_time,
                r#type: wire.r#type,
            }),
            _ => Activity::NonTrade(NonTradeActivity {
                id: wire.id,
                activity_type: wire.activity_type,
                activity_sub_type: wire.activity_sub_type,
                created_at: wire.created_at,
                currency: wire.currency,
                cusip: wire.cusip,
                group_id: wire.group_id,
                symbol: wire.symbol,
                date: wire.date,
                net_amount: wire.net_amount,
                per_share_amount: wire.per_share_amount,
                qty: wire.qty,
                status: wire.status,
            }),
        })
    }
}

/// Which activity types to list.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum ActivityFilter {
    /// These types only.
    Types(Vec<ActivityType>),
    /// Fills only, or everything but fills.
    Category(ActivityCategory),
}

/// What to list of the account's activities. One page per call: to read the
/// next page, set `page_token` to the last activity's `id`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ActivitiesQuery {
    /// Which types to list.
    pub filter: Option<ActivityFilter>,
    /// Only the fills of this order.
    pub order_id: Option<Id>,
    /// Only activities created on this date.
    pub date: Option<Date>,
    /// Only activities created before this instant.
    pub until: Option<Timestamp>,
    /// Only activities created after this instant.
    pub after: Option<Timestamp>,
    /// The order of the list; newest first unless set.
    pub direction: Option<Sort>,
    /// The most activities on a page (Alpaca's maximum is 100).
    pub page_size: Option<u32>,
    /// The `id` of the last activity of the previous page.
    pub page_token: Option<String>,
}

impl ActivitiesQuery {
    /// A query for every activity, newest first.
    pub fn new() -> ActivitiesQuery {
        ActivitiesQuery::default()
    }
}

wire_enum! {
    /// Which hours portfolio history reports within a day.
    IntradayReporting {
        /// Regular market hours only.
        MarketHours => "market_hours",
        /// Pre-market and after-hours as well.
        ExtendedHours => "extended_hours",
        /// Around the clock.
        Continuous => "continuous",
    }
}

wire_enum! {
    /// How intraday profit and loss is reset.
    PnlReset {
        /// Never reset: relative to the previous trading day's close.
        NoReset => "no_reset",
        /// Reset to the previous close each trading day.
        PerDay => "per_day",
    }
}

/// The window of portfolio history to read. Alpaca accepts at most two of a
/// period, a start and an end.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum HistoryRange {
    /// This period up to now, such as `1M` or `7D`.
    Period(String),
    /// From this instant.
    Start(Timestamp),
    /// Up to this instant.
    End(Timestamp),
    /// This period, ending at this instant.
    PeriodEnding {
        /// The period, such as `1M`.
        period: String,
        /// The end.
        end: Timestamp,
    },
    /// This period, starting at this instant.
    PeriodStarting {
        /// The start.
        start: Timestamp,
        /// The period, such as `1M`.
        period: String,
    },
    /// From one instant to another.
    Between {
        /// The start.
        start: Timestamp,
        /// The end.
        end: Timestamp,
    },
}

/// What to read of the account's portfolio history.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PortfolioHistoryQuery {
    /// The window; a month up to now unless set.
    pub range: Option<HistoryRange>,
    /// The resolution, such as `1Min`, `15Min`, `1H` or `1D`.
    pub timeframe: Option<String>,
    /// Which hours to report within a day.
    pub intraday_reporting: Option<IntradayReporting>,
    /// How intraday profit and loss is reset.
    pub pnl_reset: Option<PnlReset>,
    /// `ALL`, `NONE` or a comma-separated list of activity types.
    pub cashflow_types: Option<String>,
    /// Whether to include extended hours.
    #[deprecated(note = "Alpaca deprecates this parameter; use intraday_reporting")]
    pub extended_hours: Option<bool>,
}

impl PortfolioHistoryQuery {
    /// A query with Alpaca's defaults.
    pub fn new() -> PortfolioHistoryQuery {
        PortfolioHistoryQuery::default()
    }
}

/// The account's equity and profit and loss over time, one value per window.
///
/// The values are JSON numbers, read as the nearest `f64`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PortfolioHistory {
    /// The start of each window.
    #[serde(deserialize_with = "unix_seconds")]
    pub timestamp: Vec<Timestamp>,
    /// Equity at the end of each window.
    pub equity: Vec<Option<f64>>,
    /// Profit and loss since `base_value`, in dollars.
    pub profit_loss: Vec<Option<f64>>,
    /// Profit and loss since `base_value`, as a fraction.
    pub profit_loss_pct: Vec<Option<f64>>,
    /// The basis of the profit and loss, in dollars.
    pub base_value: Option<f64>,
    /// The trading date whose closing equity is `base_value`.
    pub base_value_asof: Option<Date>,
    /// The resolution, such as `15Min` or `1D`.
    pub timeframe: String,
    /// Cash flows accumulated to the end of each window, by activity type.
    #[serde(default)]
    pub cashflow: BTreeMap<String, Vec<f64>>,
}

/// Reads an array of integer Unix seconds as instants.
fn unix_seconds<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Timestamp>, D::Error> {
    Vec::<i64>::deserialize(deserializer)?
        .into_iter()
        .map(|seconds| {
            Timestamp::from_unix(seconds, 0).ok_or_else(|| {
                de::Error::invalid_value(
                    de::Unexpected::Signed(seconds),
                    &"Unix seconds within years 0000 to 9999",
                )
            })
        })
        .collect()
}

const ACCOUNT_CONFIGURATIONS: Endpoint = Endpoint {
    method: Method::GET,
    template: "/v2/account/configurations",
};

const UPDATE_ACCOUNT_CONFIGURATIONS: Endpoint = Endpoint {
    method: Method::PATCH,
    template: "/v2/account/configurations",
};

const ACCOUNT_ACTIVITIES: Endpoint = Endpoint {
    method: Method::GET,
    template: "/v2/account/activities",
};

const PORTFOLIO_HISTORY: Endpoint = Endpoint {
    method: Method::GET,
    template: "/v2/account/portfolio/history",
};

impl Client {
    /// Reads the account's configuration (`GET /v2/account/configurations`).
    pub async fn account_configurations(&self) -> Result<Response<AccountConfigurations>, Error> {
        self.send(
            &ACCOUNT_CONFIGURATIONS,
            &["v2", "account", "configurations"],
        )
        .await
    }

    /// Changes the account's configuration (`PATCH
    /// /v2/account/configurations`), sending only the fields set. It changes
    /// state, so it is made once and never retried.
    pub async fn update_account_configurations(
        &self,
        update: &ConfigurationsUpdate,
    ) -> Result<Response<AccountConfigurations>, Error> {
        let request = Request::new(
            &UPDATE_ACCOUNT_CONFIGURATIONS,
            &["v2", "account", "configurations"],
        )
        .json(update);
        self.send_request(request).await
    }

    /// Lists one page of the account's activities (`GET
    /// /v2/account/activities`).
    pub async fn account_activities(
        &self,
        query: &ActivitiesQuery,
    ) -> Result<Response<Vec<Activity>>, Error> {
        let mut request = Request::new(&ACCOUNT_ACTIVITIES, &["v2", "account", "activities"]);
        match &query.filter {
            Some(ActivityFilter::Types(types)) => {
                let names: Vec<&str> = types.iter().map(ActivityType::as_str).collect();
                request.pair("activity_types", names.join(","));
            }
            Some(ActivityFilter::Category(category)) => {
                request.pair("category", category.as_str());
            }
            None => {}
        }
        if let Some(order_id) = &query.order_id {
            request.pair("order_id", order_id.as_str());
        }
        if let Some(date) = &query.date {
            request.pair("date", date.to_string());
        }
        if let Some(until) = &query.until {
            request.pair("until", until.to_string());
        }
        if let Some(after) = &query.after {
            request.pair("after", after.to_string());
        }
        if let Some(direction) = &query.direction {
            request.pair("direction", direction.as_str());
        }
        if let Some(page_size) = query.page_size {
            request.pair("page_size", page_size.to_string());
        }
        if let Some(page_token) = &query.page_token {
            request.pair("page_token", page_token.as_str());
        }
        self.send_request(request).await
    }

    /// Reads the account's portfolio history (`GET
    /// /v2/account/portfolio/history`).
    pub async fn portfolio_history(
        &self,
        query: &PortfolioHistoryQuery,
    ) -> Result<Response<PortfolioHistory>, Error> {
        let (period, start, end) = match &query.range {
            None => (None, None, None),
            Some(HistoryRange::Period(period)) => (Some(period), None, None),
            Some(HistoryRange::Start(start)) => (None, Some(start), None),
            Some(HistoryRange::End(end)) => (None, None, Some(end)),
            Some(HistoryRange::PeriodEnding { period, end }) => (Some(period), None, Some(end)),
            Some(HistoryRange::PeriodStarting { start, period }) => {
                (Some(period), Some(start), None)
            }
            Some(HistoryRange::Between { start, end }) => (None, Some(start), Some(end)),
        };
        let mut request = Request::new(
            &PORTFOLIO_HISTORY,
            &["v2", "account", "portfolio", "history"],
        );
        if let Some(period) = period {
            request.pair("period", period.as_str());
        }
        if let Some(timeframe) = &query.timeframe {
            request.pair("timeframe", timeframe.as_str());
        }
        if let Some(reporting) = &query.intraday_reporting {
            request.pair("intraday_reporting", reporting.as_str());
        }
        if let Some(start) = start {
            request.pair("start", start.to_string());
        }
        if let Some(pnl_reset) = &query.pnl_reset {
            request.pair("pnl_reset", pnl_reset.as_str());
        }
        if let Some(end) = end {
            request.pair("end", end.to_string());
        }
        #[allow(deprecated)]
        if let Some(extended_hours) = query.extended_hours {
            request.pair("extended_hours", extended_hours.to_string());
        }
        if let Some(cashflow_types) = &query.cashflow_types {
            request.pair("cashflow_types", cashflow_types.as_str());
        }
        self.send_request(request).await
    }
}

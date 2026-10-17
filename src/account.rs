//! The trading account.

use std::fmt;

use reqwest::Method;
use serde::Deserialize;
use serde::de::{self, Deserializer, Visitor};

use crate::client::Endpoint;
use crate::wire::wire_enum;
use crate::{Client, Decimal, Error, Inbound, Response, Timestamp};

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

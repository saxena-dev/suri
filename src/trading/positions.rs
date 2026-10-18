//! Positions: reading them, closing them, and exercising options.

use reqwest::Method;
use serde::Deserialize;
use serde_json::value::RawValue;

use super::{AssetClass, Exchange, Order};
use crate::rest::client::{Endpoint, Host, Request};
use crate::rest::error::{details, sanitise};
use crate::value::wire::wire_enum;
use crate::{Client, Decimal, Error, Inbound, Response, Symbol};

wire_enum! {
    /// Whether a position is long or short.
    PositionSide {
        /// Long.
        Long => "long",
        /// Short.
        Short => "short",
    }
}

/// An open position.
///
/// Prices, quantities and amounts are exact [`Decimal`]s; a percent change
/// is by a factor of 1. The market value, the current and last day's
/// prices, the change and the unrealized profit and loss are `None` while
/// Alpaca has no market price for the asset, as for an allocated IPO
/// before it starts trading. Fields Alpaca adds later are ignored until
/// suri models them.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Position {
    /// The asset ID; for an option, the contract ID.
    pub asset_id: String,
    /// The symbol.
    pub symbol: String,
    /// The listing exchange; [`Exchange::Empty`] for an option or an IPO
    /// not yet listed.
    pub exchange: Inbound<Exchange>,
    /// The asset class.
    pub asset_class: Inbound<AssetClass>,
    /// Long or short.
    pub side: Inbound<PositionSide>,
    /// The average entry price.
    pub avg_entry_price: Decimal,
    /// The number of shares or contracts.
    pub qty: Decimal,
    /// The total dollar amount of the position.
    pub market_value: Option<Decimal>,
    /// The total cost basis in dollars.
    pub cost_basis: Decimal,
    /// The unrealized profit or loss in dollars.
    pub unrealized_pl: Option<Decimal>,
    /// The unrealized profit or loss, by a factor of 1.
    pub unrealized_plpc: Option<Decimal>,
    /// The unrealized profit or loss in dollars for the day.
    pub unrealized_intraday_pl: Option<Decimal>,
    /// The unrealized profit or loss for the day, by a factor of 1.
    pub unrealized_intraday_plpc: Option<Decimal>,
    /// The current price per share.
    pub current_price: Option<Decimal>,
    /// The last trading day's closing price per share.
    pub lastday_price: Option<Decimal>,
    /// The change from the last day's price, by a factor of 1.
    pub change_today: Option<Decimal>,
    /// Whether the asset can be bought on margin.
    pub asset_marginable: Option<bool>,
    /// The quantity not held by open orders.
    pub qty_available: Option<Decimal>,
    /// The weighted average exchange rate when the position was entered.
    pub avg_entry_swap_rate: Option<Decimal>,
    /// The exchange rate at the previous close.
    pub prev_swap_rate: Option<Decimal>,
    /// The current exchange rate to US dollars.
    pub swap_rate: Option<Decimal>,
    /// The position's values in US dollars.
    pub usd: Option<UsdPositionValues>,
}

/// A position's values in US dollars.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct UsdPositionValues {
    /// The average entry price.
    pub avg_entry_price: Decimal,
    /// The total cost basis.
    pub cost_basis: Decimal,
    /// The total market value.
    pub market_value: Option<Decimal>,
    /// The unrealized profit or loss.
    pub unrealized_pl: Option<Decimal>,
    /// The unrealized profit or loss, by a factor of 1.
    pub unrealized_plpc: Option<Decimal>,
    /// The unrealized profit or loss for the day.
    pub unrealized_intraday_pl: Option<Decimal>,
    /// The unrealized profit or loss for the day, by a factor of 1.
    pub unrealized_intraday_plpc: Option<Decimal>,
    /// The current price per share.
    pub current_price: Option<Decimal>,
    /// The last trading day's closing price per share.
    pub lastday_price: Option<Decimal>,
    /// The change from the last day's price, by a factor of 1.
    pub change_today: Option<Decimal>,
}

/// How much of a position to close.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum ClosePosition {
    /// All of it.
    All,
    /// This many shares or contracts.
    Qty(Decimal),
    /// This percentage of it, from 0 to 100: `0.5` is half a percent.
    Percentage(Decimal),
}

/// What happened to one position when all were closed.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct ClosedPosition {
    /// The position's symbol.
    pub symbol: String,
    /// The HTTP status of this position's closing.
    pub status: u16,
    /// For a successful closing, the order that closes the position.
    pub order: Option<Order>,
    /// For a failed closing, Alpaca's error code.
    pub code: Option<u64>,
    /// For a failed closing, Alpaca's message, scrubbed and bounded as
    /// [`Error::message`] is.
    pub message: Option<String>,
    /// For a successful closing whose body is not an order, that body as
    /// text, scrubbed and bounded the same way.
    pub body: Option<String>,
}

/// One item of a close-all response, as sent.
#[derive(Deserialize)]
struct WireClosed {
    symbol: String,
    status: u16,
    body: Option<Box<RawValue>>,
}

const POSITIONS: Endpoint = Endpoint {
    host: Host::Trading,
    method: Method::GET,
    template: "/v2/positions",
};

const POSITION: Endpoint = Endpoint {
    host: Host::Trading,
    method: Method::GET,
    template: "/v2/positions/{symbol_or_asset_id}",
};

const CLOSE_POSITION: Endpoint = Endpoint {
    host: Host::Trading,
    method: Method::DELETE,
    template: "/v2/positions/{symbol_or_asset_id}",
};

const CLOSE_ALL_POSITIONS: Endpoint = Endpoint {
    host: Host::Trading,
    method: Method::DELETE,
    template: "/v2/positions",
};

const EXERCISE: Endpoint = Endpoint {
    host: Host::Trading,
    method: Method::POST,
    template: "/v2/positions/{symbol_or_contract_id}/exercise",
};

const DO_NOT_EXERCISE: Endpoint = Endpoint {
    host: Host::Trading,
    method: Method::POST,
    template: "/v2/positions/{symbol_or_contract_id}/do-not-exercise",
};

impl Client {
    /// Lists the open positions (`GET /v2/positions`).
    pub async fn positions(&self) -> Result<Response<Vec<Position>>, Error> {
        self.send(&POSITIONS, &["v2", "positions"]).await
    }

    /// Reads one open position by symbol or asset ID (`GET
    /// /v2/positions/{symbol_or_asset_id}`).
    pub async fn position(&self, symbol: &Symbol) -> Result<Response<Position>, Error> {
        self.send(&POSITION, &["v2", "positions", symbol.as_str()])
            .await
    }

    /// Closes a position, or part of it, by placing an order (`DELETE
    /// /v2/positions/{symbol_or_asset_id}`), and returns that order. It is
    /// made once and never retried.
    pub async fn close_position(
        &self,
        symbol: &Symbol,
        close: &ClosePosition,
    ) -> Result<Response<Order>, Error> {
        let segments = ["v2", "positions", symbol.as_str()];
        let mut request = Request::new(&CLOSE_POSITION, &segments);
        match close {
            ClosePosition::All => {}
            ClosePosition::Qty(qty) => request.pair("qty", qty.as_str()),
            ClosePosition::Percentage(percentage) => {
                request.pair("percentage", percentage.as_str());
            }
        }
        self.send_request(request).await
    }

    /// Closes every open position (`DELETE /v2/positions`), with one result
    /// per position; `cancel_orders` cancels the open orders first. It is
    /// made once and never retried.
    pub async fn close_all_positions(
        &self,
        cancel_orders: bool,
    ) -> Result<Response<Vec<ClosedPosition>>, Error> {
        let mut request = Request::new(&CLOSE_ALL_POSITIONS, &["v2", "positions"]);
        if cancel_orders {
            request.pair("cancel_orders", "true");
        }
        let (items, meta) = self
            .send_request::<Vec<WireClosed>>(request)
            .await?
            .into_parts();
        let credentials = self.credentials();
        let results = items
            .into_iter()
            .map(|item| {
                let text = item.body.as_deref().map(RawValue::get);
                let mut closed = ClosedPosition {
                    symbol: item.symbol,
                    status: item.status,
                    order: None,
                    code: None,
                    message: None,
                    body: None,
                };
                if (200..300).contains(&item.status) {
                    match text.map(serde_json::from_str::<Order>) {
                        Some(Ok(order)) => closed.order = Some(order),
                        Some(Err(_)) => closed.body = text.map(|text| sanitise(text, &credentials)),
                        None => {}
                    }
                } else {
                    (closed.code, closed.message) = details(text.map(str::as_bytes), &credentials);
                }
                closed
            })
            .collect();
        Ok(Response::new(results, meta))
    }

    /// Exercises a held option contract (`POST
    /// /v2/positions/{symbol_or_contract_id}/exercise`). Success means the
    /// instruction was submitted. It is made once and never retried.
    pub async fn exercise_option(&self, contract: &Symbol) -> Result<Response<()>, Error> {
        let segments = ["v2", "positions", contract.as_str(), "exercise"];
        let request = Request::new(&EXERCISE, &segments).no_content();
        self.send_request(request).await
    }

    /// Instructs Alpaca not to exercise a held option contract at expiry
    /// (`POST /v2/positions/{symbol_or_contract_id}/do-not-exercise`).
    /// Success means the instruction was submitted. It is made once and
    /// never retried. Alpaca's options guide asks users to contact its
    /// support for this instead.
    pub async fn decline_exercise(&self, contract: &Symbol) -> Result<Response<()>, Error> {
        let segments = ["v2", "positions", contract.as_str(), "do-not-exercise"];
        let request = Request::new(&DO_NOT_EXERCISE, &segments).no_content();
        self.send_request(request).await
    }
}

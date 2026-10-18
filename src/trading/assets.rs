//! Assets and option contracts.

use reqwest::Method;
use serde::Deserialize;

use crate::rest::client::{Endpoint, Host, Request};
use crate::value::wire::wire_enum;
use crate::{Client, Date, Decimal, Error, Inbound, Response, Symbol};

wire_enum! {
    /// The kind of instrument an asset is.
    AssetClass {
        /// A US equity.
        UsEquity => "us_equity",
        /// A US option.
        UsOption => "us_option",
        /// A cryptocurrency.
        Crypto => "crypto",
        /// A cryptocurrency perpetual future.
        CryptoPerp => "crypto_perp",
        /// A US Treasury.
        Treasury => "treasury",
        /// A corporate bond.
        Corporate => "corporate",
        /// A non-US equity.
        GlobalEquity => "global_equity",
        /// A US index.
        UsIndex => "us_index",
        /// A US equity option chain.
        UsEquityChain => "us_equity_chain",
        /// An IPO indication of interest.
        Ipo => "ipo",
    }
}

wire_enum! {
    /// The exchange an asset is listed on.
    Exchange {
        /// `AMEX`.
        Amex => "AMEX",
        /// `ARCA`.
        Arca => "ARCA",
        /// `BATS`.
        Bats => "BATS",
        /// `NYSE`.
        Nyse => "NYSE",
        /// `NASDAQ`.
        Nasdaq => "NASDAQ",
        /// `NYSEARCA`.
        Nysearca => "NYSEARCA",
        /// `OTC`.
        Otc => "OTC",
        /// `CRYPTO`.
        Crypto => "CRYPTO",
        /// No exchange (`""`), as an option position reports.
        Empty => "",
    }
}

wire_enum! {
    /// Whether an asset or option contract is active.
    AssetStatus {
        /// Active.
        Active => "active",
        /// Inactive.
        Inactive => "inactive",
    }
}

wire_enum! {
    /// A characteristic of an asset.
    AssetAttribute {
        /// A publicly traded partnership without a qualified notice.
        PtpNoException => "ptp_no_exception",
        /// A publicly traded partnership open without general restrictions.
        PtpWithException => "ptp_with_exception",
        /// Accepting limit orders only, before secondary-market trading.
        Ipo => "ipo",
        /// The equity has listed options.
        HasOptions => "has_options",
        /// The asset's options close at 4:15 pm ET.
        OptionsLateClose => "options_late_close",
        /// Fractional orders are accepted in extended hours.
        FractionalEhEnabled => "fractional_eh_enabled",
        /// Eligible for overnight trading.
        OvernightTradable => "overnight_tradable",
        /// Eligible for overnight trading, but halted overnight.
        OvernightHalted => "overnight_halted",
    }
}

wire_enum! {
    /// How readily a US equity can be borrowed to sell short.
    BorrowStatus {
        /// Easy to borrow.
        EasyToBorrow => "easy_to_borrow",
        /// Hard to borrow.
        HardToBorrow => "hard_to_borrow",
    }
}

/// A tradable instrument and what Alpaca allows with it.
///
/// Requirements, sizes and increments are exact [`Decimal`]s. Fields Alpaca
/// adds later are ignored until suri models them.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Asset {
    /// The asset ID.
    pub id: String,
    /// The symbol.
    pub symbol: String,
    /// The asset's name.
    pub name: String,
    /// The kind of instrument.
    pub class: Inbound<AssetClass>,
    /// The listing exchange.
    pub exchange: Inbound<Exchange>,
    /// Whether the asset is active.
    pub status: Inbound<AssetStatus>,
    /// Whether it can be traded on Alpaca.
    pub tradable: bool,
    /// Whether it can be bought on margin.
    pub marginable: bool,
    /// Whether it can be sold short.
    pub shortable: bool,
    /// Whether fractional quantities can be traded.
    pub fractionable: bool,
    /// The asset's characteristics.
    pub attributes: Option<Vec<Inbound<AssetAttribute>>>,
    /// How readily it can be borrowed; US equities only.
    pub borrow_status: Option<Inbound<BorrowStatus>>,
    /// The CUSIP.
    pub cusip: Option<String>,
    /// The margin requirement for a long position, in percent.
    pub margin_requirement_long: Option<Decimal>,
    /// The margin requirement for a short position, in percent.
    pub margin_requirement_short: Option<Decimal>,
    /// The smallest order quantity.
    pub min_order_size: Option<Decimal>,
    /// The smallest quantity increment.
    pub min_trade_increment: Option<Decimal>,
    /// The smallest price increment.
    pub price_increment: Option<Decimal>,
    /// The margin requirement in percent, a JSON number read as the nearest
    /// `f64`.
    #[deprecated(note = "use margin_requirement_long and margin_requirement_short")]
    pub maintenance_margin_requirement: Option<f64>,
}

/// Which assets to list.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AssetsQuery {
    /// Only assets with this status; every status unless set.
    pub status: Option<AssetStatus>,
    /// Only assets of this class; US equities unless set.
    pub asset_class: Option<AssetClass>,
    /// Only assets on this exchange.
    pub exchange: Option<Exchange>,
    /// Only assets with any of these attributes.
    pub attributes: Option<Vec<AssetAttribute>>,
}

impl AssetsQuery {
    /// A query with Alpaca's defaults.
    pub fn new() -> AssetsQuery {
        AssetsQuery::default()
    }
}

wire_enum! {
    /// Whether an option is a call or a put.
    ContractType {
        /// A call.
        Call => "call",
        /// A put.
        Put => "put",
    }
}

wire_enum! {
    /// When an option can be exercised.
    ExerciseStyle {
        /// On any day up to expiration.
        American => "american",
        /// At expiration only.
        European => "european",
    }
}

wire_enum! {
    /// What an option deliverable is.
    DeliverableType {
        /// Cash.
        Cash => "cash",
        /// An equity.
        Equity => "equity",
    }
}

wire_enum! {
    /// How many days after exercise a deliverable settles.
    SettlementType {
        /// The same day.
        TPlus0 => "T+0",
        /// One day after.
        TPlus1 => "T+1",
        /// Two days after.
        TPlus2 => "T+2",
        /// Three days after.
        TPlus3 => "T+3",
        /// Four days after.
        TPlus4 => "T+4",
        /// Five days after.
        TPlus5 => "T+5",
    }
}

wire_enum! {
    /// How a deliverable settles.
    SettlementMethod {
        /// Broker to broker.
        Btob => "BTOB",
        /// Cash difference.
        Cadf => "CADF",
        /// Cash fixed.
        Cafx => "CAFX",
        /// Correspondent clearing corporation.
        Ccc => "CCC",
    }
}

/// An option contract.
///
/// Prices and sizes are exact [`Decimal`]s. Fields Alpaca adds later are
/// ignored until suri models them.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OptionContract {
    /// The contract ID.
    pub id: String,
    /// The contract's symbol, such as `AAPL231103C00170000`.
    pub symbol: String,
    /// The contract's name.
    pub name: String,
    /// Whether the contract is active.
    pub status: Inbound<AssetStatus>,
    /// Whether it can be traded on Alpaca.
    pub tradable: bool,
    /// The expiration date.
    pub expiration_date: Date,
    /// The underlying asset's symbol.
    pub underlying_symbol: String,
    /// The underlying asset's ID.
    pub underlying_asset_id: String,
    /// Call or put.
    pub r#type: Inbound<ContractType>,
    /// American or European.
    pub style: Inbound<ExerciseStyle>,
    /// The strike price.
    pub strike_price: Decimal,
    /// The number of underlying shares per contract.
    pub size: Decimal,
    /// Whether the contract is in the Penny Program.
    pub ppind: Option<bool>,
    /// The multiplier applied to the price.
    pub multiplier: Option<Decimal>,
    /// The root symbol.
    pub root_symbol: Option<String>,
    /// The last close price.
    pub close_price: Option<Decimal>,
    /// The date of the last close price.
    pub close_price_date: Option<Date>,
    /// The open interest.
    pub open_interest: Option<Decimal>,
    /// The date of the open interest.
    pub open_interest_date: Option<Date>,
    /// What exercising the contract delivers, when asked for.
    pub deliverables: Option<Vec<Deliverable>>,
}

/// One thing an option contract delivers on exercise.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Deliverable {
    /// The deliverable's symbol.
    pub symbol: String,
    /// Cash or equity.
    pub r#type: Inbound<DeliverableType>,
    /// The amount delivered (for cash, the cash amount); `None` while a
    /// delayed settlement leaves it undetermined.
    pub amount: Option<Decimal>,
    /// The deliverable's share of the cost basis, in percent.
    pub allocation_percentage: Decimal,
    /// When it settles if the contract is exercised or assigned.
    pub settlement_type: Inbound<SettlementType>,
    /// How it settles.
    pub settlement_method: Inbound<SettlementMethod>,
    /// Whether the settlement is delayed.
    pub delayed_settlement: bool,
    /// The deliverable's asset ID; absent for cash.
    pub asset_id: Option<String>,
}

/// One page of option contracts.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OptionContracts {
    /// The contracts.
    pub option_contracts: Vec<OptionContract>,
    /// The token for the next page; `None` on the last page.
    pub next_page_token: Option<String>,
}

/// Which option contracts to list. By default Alpaca lists active contracts
/// expiring before the next weekend. One page per call: to read the next
/// page, set `page_token` to the previous page's `next_page_token`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OptionContractsQuery {
    /// Only contracts on these underlying symbols.
    pub underlying_symbols: Option<Vec<String>>,
    /// Whether to include each contract's deliverables.
    pub show_deliverables: Option<bool>,
    /// Only contracts with this status; active unless set.
    pub status: Option<AssetStatus>,
    /// Only contracts expiring on this date.
    pub expiration_date: Option<Date>,
    /// Only contracts expiring on or after this date.
    pub expiration_date_gte: Option<Date>,
    /// Only contracts expiring on or before this date.
    pub expiration_date_lte: Option<Date>,
    /// Only contracts with this root symbol.
    pub root_symbol: Option<String>,
    /// Only calls or only puts.
    pub r#type: Option<ContractType>,
    /// Only contracts of this exercise style.
    pub style: Option<ExerciseStyle>,
    /// Only contracts with a strike at or above this price, sent as its
    /// exact text.
    pub strike_price_gte: Option<Decimal>,
    /// Only contracts with a strike at or below this price, sent as its
    /// exact text.
    pub strike_price_lte: Option<Decimal>,
    /// The previous page's `next_page_token`.
    pub page_token: Option<String>,
    /// The most contracts on a page (Alpaca's default is 100, its maximum
    /// 10000).
    pub limit: Option<u32>,
    /// Only contracts in, or only those outside, the Penny Program.
    pub ppind: Option<bool>,
}

impl OptionContractsQuery {
    /// A query with Alpaca's defaults.
    pub fn new() -> OptionContractsQuery {
        OptionContractsQuery::default()
    }
}

const ASSETS: Endpoint = Endpoint {
    host: Host::Trading,
    method: Method::GET,
    template: "/v2/assets",
};

const ASSET: Endpoint = Endpoint {
    host: Host::Trading,
    method: Method::GET,
    template: "/v2/assets/{symbol_or_asset_id}",
};

const OPTION_CONTRACTS: Endpoint = Endpoint {
    host: Host::Trading,
    method: Method::GET,
    template: "/v2/options/contracts",
};

const OPTION_CONTRACT: Endpoint = Endpoint {
    host: Host::Trading,
    method: Method::GET,
    template: "/v2/options/contracts/{symbol_or_id}",
};

impl Client {
    /// Lists assets (`GET /v2/assets`).
    pub async fn assets(&self, query: &AssetsQuery) -> Result<Response<Vec<Asset>>, Error> {
        let mut request = Request::new(&ASSETS, &["v2", "assets"]);
        if let Some(status) = &query.status {
            request.pair("status", status.as_str());
        }
        if let Some(asset_class) = &query.asset_class {
            request.pair("asset_class", asset_class.as_str());
        }
        if let Some(exchange) = &query.exchange {
            request.pair("exchange", exchange.as_str());
        }
        if let Some(attributes) = &query.attributes {
            let names: Vec<&str> = attributes.iter().map(AssetAttribute::as_str).collect();
            request.pair("attributes", names.join(","));
        }
        self.send_request(request).await
    }

    /// Reads one asset by symbol or asset ID (`GET
    /// /v2/assets/{symbol_or_asset_id}`). A crypto pair such as `BTC/USD` is
    /// sent encoded, as `BTC%2FUSD`.
    pub async fn asset(&self, symbol: &Symbol) -> Result<Response<Asset>, Error> {
        self.send(&ASSET, &["v2", "assets", symbol.as_str()]).await
    }

    /// Lists one page of option contracts (`GET /v2/options/contracts`).
    pub async fn option_contracts(
        &self,
        query: &OptionContractsQuery,
    ) -> Result<Response<OptionContracts>, Error> {
        let mut request = Request::new(&OPTION_CONTRACTS, &["v2", "options", "contracts"]);
        if let Some(symbols) = &query.underlying_symbols {
            request.pair("underlying_symbols", symbols.join(","));
        }
        if let Some(show_deliverables) = query.show_deliverables {
            request.pair("show_deliverables", show_deliverables.to_string());
        }
        if let Some(status) = &query.status {
            request.pair("status", status.as_str());
        }
        if let Some(date) = &query.expiration_date {
            request.pair("expiration_date", date.to_string());
        }
        if let Some(date) = &query.expiration_date_gte {
            request.pair("expiration_date_gte", date.to_string());
        }
        if let Some(date) = &query.expiration_date_lte {
            request.pair("expiration_date_lte", date.to_string());
        }
        if let Some(root_symbol) = &query.root_symbol {
            request.pair("root_symbol", root_symbol.as_str());
        }
        if let Some(contract_type) = &query.r#type {
            request.pair("type", contract_type.as_str());
        }
        if let Some(style) = &query.style {
            request.pair("style", style.as_str());
        }
        if let Some(strike) = &query.strike_price_gte {
            request.pair("strike_price_gte", strike.as_str());
        }
        if let Some(strike) = &query.strike_price_lte {
            request.pair("strike_price_lte", strike.as_str());
        }
        if let Some(page_token) = &query.page_token {
            request.pair("page_token", page_token.as_str());
        }
        if let Some(limit) = query.limit {
            request.pair("limit", limit.to_string());
        }
        if let Some(ppind) = query.ppind {
            request.pair("ppind", ppind.to_string());
        }
        self.send_request(request).await
    }

    /// Reads one option contract by symbol or ID (`GET
    /// /v2/options/contracts/{symbol_or_id}`).
    pub async fn option_contract(
        &self,
        symbol: &Symbol,
    ) -> Result<Response<OptionContract>, Error> {
        self.send(
            &OPTION_CONTRACT,
            &["v2", "options", "contracts", symbol.as_str()],
        )
        .await
    }
}

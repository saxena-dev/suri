//! Assets and option contracts: the upstream fixtures field by field, symbols
//! in paths, every query parameter on the wire, and the decimal rules.

#![cfg(feature = "rest")]

#[path = "support/fixture.rs"]
mod fixture;
#[path = "support/server.rs"]
mod server;

use server::{Request, Server, Step, response};
use suri::{
    Asset, AssetAttribute, AssetClass, AssetStatus, AssetsQuery, BorrowStatus, Client, Config,
    ContractType, Credentials, Date, Decimal, Deliverable, DeliverableType, Environment, ErrorKind,
    Exchange, ExerciseStyle, Id, Inbound, OptionContract, OptionContracts, OptionContractsQuery,
    Region, SettlementMethod, SettlementType, Symbol,
};

const ALL_ASSETS: &str =
    "trading/trading_client/test_asset_routes::test_get_all_assets::GET /v2/assets#0";
const ALL_ASSETS_PARAMS: &str =
    "trading/trading_client/test_asset_routes::test_get_all_assets_params::GET /v2/assets#0";
const ASSET: &str =
    "trading/trading_client/test_asset_routes::test_get_asset::GET /v2/assets/{symbol}#0";
const CONTRACTS: &str = "trading/trading_client/test_option_routes::test_get_option_contracts::GET /v2/options/contracts#0";
const CONTRACTS_SYMBOLS: &str = "trading/trading_client/test_option_routes::test_get_option_contracts_with_multiple_symbols::GET /v2/options/contracts#0";
const CONTRACTS_DELIVERABLES: &str = "trading/trading_client/test_option_routes::test_get_option_contracts_with_deliverables_and_ppind::GET /v2/options/contracts#0";
const CONTRACT: &str = "trading/trading_client/test_option_routes::test_get_option_contract::GET /v2/options/contracts/{symbol}#0";
const CONTRACT_CASH: &str = "trading/trading_client/test_option_routes::test_get_option_contract_cash_deliverable_omits_asset_id::GET /v2/options/contracts/{symbol}#0";
const CONTRACT_DELAYED: &str = "trading/trading_client/test_option_routes::test_get_option_contract_delayed_settlement_null_amount::GET /v2/options/contracts/{symbol}#0";

// Built from the asset schema's example and per-property examples, with the
// properties that have none set to distinct values.
const FULL_ASSET: &str = r#"{
  "attributes":["ptp_no_exception","has_options"],
  "borrow_status":"easy_to_borrow",
  "class":"us_equity",
  "cusip":"987654321",
  "easy_to_borrow":true,
  "exchange":"NASDAQ",
  "fractionable":true,
  "id":"b0b6dd9d-8b9b-48a9-ba46-b9d54906e415",
  "maintenance_margin_requirement":30,
  "margin_requirement_long":"100",
  "margin_requirement_short":"30",
  "marginable":true,
  "min_order_size":"0.0001",
  "min_trade_increment":"0.000001",
  "name":"Apple Inc. Common Stock",
  "price_increment":"0.01",
  "shortable":false,
  "status":"active",
  "symbol":"AAPL",
  "tradable":true
}"#;

fn client(server: &Server) -> Client {
    let config = Config::new(
        Region::US,
        Environment::Paper,
        Credentials::api_key("PKSENTINELKEY0001", "SECRETSENTINELVALUE0001"),
    )
    .with_base_url(&server.url)
    .with_max_read_attempts(1);
    Client::new(config).unwrap()
}

fn dec(text: &str) -> Decimal {
    text.parse().unwrap()
}

fn date(text: &str) -> Date {
    text.parse().unwrap()
}

fn serve(body: &[u8]) -> Server {
    Server::start(vec![Step::Respond(response("200 OK", &[], body))])
}

/// The query's pairs, decoded, in order.
fn pairs(request: &Request) -> Vec<(String, String)> {
    let url = url::Url::parse(&format!("http://h{}", request.path)).unwrap();
    url.query_pairs()
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect()
}

fn owned(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

fn assert_plain_get(request: &Request, path: &str) {
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, path);
    assert_eq!(request.header("Content-Type"), None);
    assert!(request.body.is_empty());
}

async fn assets(body: &[u8], query: &AssetsQuery) -> (Vec<Asset>, Request) {
    let server = serve(body);
    let list = client(&server).assets(query).await.unwrap().into_parts().0;
    (list, server.requests().remove(0))
}

async fn read_asset(body: &[u8], symbol: &Symbol) -> (Result<Asset, suri::Error>, Request) {
    let server = serve(body);
    let result = client(&server)
        .asset(symbol)
        .await
        .map(|response| response.into_parts().0);
    (result, server.requests().remove(0))
}

async fn contracts(body: &[u8], query: &OptionContractsQuery) -> (OptionContracts, Request) {
    let server = serve(body);
    let page = client(&server)
        .option_contracts(query)
        .await
        .unwrap()
        .into_parts()
        .0;
    (page, server.requests().remove(0))
}

async fn read_contract(
    body: &[u8],
    symbol: &str,
) -> (Result<OptionContract, suri::Error>, Request) {
    let server = serve(body);
    let result = client(&server)
        .option_contract(&symbol.parse().unwrap())
        .await
        .map(|response| response.into_parts().0);
    (result, server.requests().remove(0))
}

/// The one asset every upstream asset fixture carries.
#[allow(deprecated)]
fn assert_fixture_asset(asset: &Asset) {
    assert_eq!(asset.id, "904837e3-3b76-47ec-b432-046db621571b");
    assert_eq!(asset.class, Inbound::Known(AssetClass::UsEquity));
    assert_eq!(asset.exchange, Inbound::Known(Exchange::Nasdaq));
    assert_eq!(asset.symbol, "AAPL");
    assert_eq!(asset.name, "Apple Inc. Common Stock");
    assert_eq!(asset.status, Inbound::Known(AssetStatus::Active));
    assert!(asset.tradable);
    assert!(asset.marginable);
    assert!(asset.shortable);
    assert!(asset.fractionable);
    // easy_to_borrow, last_close_pct_change and last_price are not modelled
    // and are ignored; the two attributes are not documented values.
    assert_eq!(
        asset.attributes,
        Some(vec![
            Inbound::Unknown("attribute1".to_owned()),
            Inbound::Unknown("attribute2".to_owned()),
        ])
    );
    assert_eq!(asset.borrow_status, None);
    assert_eq!(asset.cusip, None);
    assert_eq!(asset.margin_requirement_long, None);
    assert_eq!(asset.margin_requirement_short, None);
    assert_eq!(asset.min_order_size, None);
    assert_eq!(asset.min_trade_increment, None);
    assert_eq!(asset.price_increment, None);
    assert_eq!(asset.maintenance_margin_requirement, None);
}

#[tokio::test]
async fn the_asset_list_fixture_decodes_field_by_field() {
    let body = fixture::upstream(ALL_ASSETS).body;
    let (list, request) = assets(&body, &AssetsQuery::new()).await;
    assert_plain_get(&request, "/v2/assets");
    assert_eq!(list.len(), 1);
    assert_fixture_asset(&list[0]);
}

#[tokio::test]
async fn every_asset_query_parameter_goes_on_the_wire_in_order() {
    let body = fixture::upstream(ALL_ASSETS_PARAMS).body;
    let mut query = AssetsQuery::new();
    query.status = Some(AssetStatus::Active);
    query.asset_class = Some(AssetClass::UsEquity);
    query.exchange = Some(Exchange::Nasdaq);
    query.attributes = Some(vec![AssetAttribute::PtpNoException, AssetAttribute::Ipo]);
    let (list, request) = assets(&body, &query).await;
    assert_eq!(
        pairs(&request),
        owned(&[
            ("status", "active"),
            ("asset_class", "us_equity"),
            ("exchange", "NASDAQ"),
            ("attributes", "ptp_no_exception,ipo"),
        ])
    );
    assert_eq!(list.len(), 1);
    assert_fixture_asset(&list[0]);
}

#[tokio::test]
async fn the_asset_fixture_decodes_field_by_field() {
    let body = fixture::upstream(ASSET).body;
    let (asset, request) = read_asset(&body, &"AAPL".parse().unwrap()).await;
    assert_plain_get(&request, "/v2/assets/AAPL");
    assert_fixture_asset(&asset.unwrap());
}

#[tokio::test]
#[allow(deprecated)]
async fn every_asset_field_decodes() {
    let (asset, _) = read_asset(FULL_ASSET.as_bytes(), &"AAPL".parse().unwrap()).await;
    let asset = asset.unwrap();
    assert_eq!(asset.id, "b0b6dd9d-8b9b-48a9-ba46-b9d54906e415");
    assert_eq!(asset.symbol, "AAPL");
    assert_eq!(asset.name, "Apple Inc. Common Stock");
    assert_eq!(asset.class, Inbound::Known(AssetClass::UsEquity));
    assert_eq!(asset.exchange, Inbound::Known(Exchange::Nasdaq));
    assert_eq!(asset.status, Inbound::Known(AssetStatus::Active));
    assert!(asset.tradable);
    assert!(asset.marginable);
    assert!(!asset.shortable);
    assert!(asset.fractionable);
    assert_eq!(
        asset.attributes,
        Some(vec![
            Inbound::Known(AssetAttribute::PtpNoException),
            Inbound::Known(AssetAttribute::HasOptions),
        ])
    );
    assert_eq!(
        asset.borrow_status,
        Some(Inbound::Known(BorrowStatus::EasyToBorrow))
    );
    assert_eq!(asset.cusip.as_deref(), Some("987654321"));
    assert_eq!(asset.margin_requirement_long, Some(dec("100")));
    assert_eq!(asset.margin_requirement_short, Some(dec("30")));
    assert_eq!(asset.min_order_size, Some(dec("0.0001")));
    assert_eq!(asset.min_trade_increment, Some(dec("0.000001")));
    assert_eq!(asset.price_increment, Some(dec("0.01")));
    assert_eq!(asset.maintenance_margin_requirement, Some(30.0));
}

#[tokio::test]
async fn unknown_values_and_an_empty_exchange_are_kept() {
    let body = br#"{"id":"i","symbol":"S","name":"N","class":"us_future","exchange":"","status":"delisted","tradable":false,"marginable":false,"shortable":false,"fractionable":false,"borrow_status":"locate_required"}"#;
    let (asset, _) = read_asset(body, &"S".parse().unwrap()).await;
    let asset = asset.unwrap();
    assert_eq!(asset.class, Inbound::Unknown("us_future".to_owned()));
    assert_eq!(asset.exchange, Inbound::Known(Exchange::Empty));
    assert_eq!(asset.status, Inbound::Unknown("delisted".to_owned()));
    assert_eq!(
        asset.borrow_status,
        Some(Inbound::Unknown("locate_required".to_owned()))
    );
    assert_eq!(asset.attributes, None);
}

#[tokio::test]
#[allow(deprecated)]
async fn the_deprecated_margin_requirement_is_read_to_the_nearest_f64() {
    // Each sample decodes to exactly the f64 Rust's own parser gives,
    // compared bit for bit.
    let samples = [
        "8425.21",
        "0.30000000000000004",
        "1.1368683772161603e-13",
        "9007199254740993",
        "null",
    ];
    for sample in samples {
        let body = FULL_ASSET.replace(
            r#""maintenance_margin_requirement":30"#,
            &format!(r#""maintenance_margin_requirement":{sample}"#),
        );
        let (asset, _) = read_asset(body.as_bytes(), &"AAPL".parse().unwrap()).await;
        let value = asset.unwrap().maintenance_margin_requirement;
        let expected = sample.parse::<f64>().ok();
        assert_eq!(
            value.map(f64::to_bits),
            expected.map(f64::to_bits),
            "{sample}"
        );
    }
}

#[tokio::test]
async fn asset_decimals_follow_the_exact_rules() {
    let long = "1234567890.1234567890123456789";
    let body = FULL_ASSET.replace(r#""0.0001""#, &format!(r#""{long}""#));
    let (asset, _) = read_asset(body.as_bytes(), &"AAPL".parse().unwrap()).await;
    assert_eq!(asset.unwrap().min_order_size.unwrap().as_str(), long);
    for bad in ["0.5", "\"1e2\""] {
        let body = FULL_ASSET.replace(r#""0.01""#, bad);
        let (result, _) = read_asset(body.as_bytes(), &"AAPL".parse().unwrap()).await;
        assert_eq!(result.unwrap_err().kind(), ErrorKind::Decode, "{bad}");
    }
}

#[tokio::test]
async fn an_asset_without_a_required_field_fails() {
    let body = FULL_ASSET.replace(r#""fractionable":true,"#, "");
    let (result, _) = read_asset(body.as_bytes(), &"AAPL".parse().unwrap()).await;
    assert_eq!(result.unwrap_err().kind(), ErrorKind::Decode);
}

#[tokio::test]
async fn a_crypto_pair_is_sent_encoded() {
    let body = fixture::upstream(ASSET).body;
    let (_, request) = read_asset(&body, &"BTC/USD".parse().unwrap()).await;
    assert_plain_get(&request, "/v2/assets/BTC%2FUSD");
}

#[tokio::test]
async fn an_id_passes_as_a_symbol() {
    let body = fixture::upstream(ASSET).body;
    // Mixed case, so a conversion that changed the case would show.
    let text = "904837E3-3b76-47ec-B432-046db621571b";
    let id: Id = text.parse().unwrap();
    let (_, request) = read_asset(&body, &Symbol::from(&id)).await;
    assert_plain_get(&request, &format!("/v2/assets/{text}"));
    assert_eq!(Symbol::from(id).as_str(), text);
}

#[tokio::test]
async fn an_unknown_asset_is_an_api_error() {
    let server = Server::start(vec![Step::Respond(response(
        "404 Not Found",
        &[],
        br#"{"code":40410000,"message":"asset not found for NOPE"}"#,
    ))]);
    let error = client(&server)
        .asset(&"NOPE".parse().unwrap())
        .await
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Api);
    assert_eq!(error.status(), Some(404));
}

/// The contract every upstream option fixture carries, apart from the
/// fields that vary between them.
fn assert_fixture_contract(contract: &OptionContract) {
    assert_eq!(contract.id, "00000000-0000-0000-0000-000000000000");
    assert_eq!(contract.symbol, "AAPL231103C00170000");
    assert_eq!(contract.name, "AAPL Nov 03 2023 170 Call");
    assert_eq!(contract.status, Inbound::Known(AssetStatus::Active));
    assert!(contract.tradable);
    assert_eq!(contract.expiration_date, date("2023-11-03"));
    assert_eq!(contract.root_symbol.as_deref(), Some("AAPL"));
    assert_eq!(contract.underlying_symbol, "AAPL");
    assert_eq!(
        contract.underlying_asset_id,
        "00000000-0000-0000-0000-000000000000"
    );
    assert_eq!(contract.r#type, Inbound::Known(ContractType::Call));
    assert_eq!(contract.style, Inbound::Known(ExerciseStyle::American));
    assert_eq!(contract.strike_price, dec("170"));
    assert_eq!(contract.size, dec("100"));
    assert_eq!(contract.open_interest, Some(dec("0")));
    assert_eq!(contract.open_interest_date, Some(date("2023-11-03")));
    assert_eq!(contract.close_price, None);
    assert_eq!(contract.close_price_date, None);
}

/// The contract without `ppind`, `multiplier` or deliverables.
fn assert_bare_contract(contract: &OptionContract) {
    assert_fixture_contract(contract);
    assert_eq!(contract.ppind, None);
    assert_eq!(contract.multiplier, None);
    assert_eq!(contract.deliverables, None);
}

/// The contract with `ppind`, `multiplier` and one deliverable.
fn assert_full_contract(contract: &OptionContract) -> &Deliverable {
    assert_fixture_contract(contract);
    assert_eq!(contract.ppind, Some(true));
    assert_eq!(contract.multiplier, Some(dec("100")));
    let deliverables = contract.deliverables.as_ref().unwrap();
    assert_eq!(deliverables.len(), 1);
    &deliverables[0]
}

fn assert_standard_deliverable(deliverable: &Deliverable) {
    assert_eq!(deliverable.r#type, Inbound::Known(DeliverableType::Equity));
    assert_eq!(deliverable.symbol, "AAPL");
    assert_eq!(deliverable.amount, Some(dec("100")));
    assert_eq!(deliverable.allocation_percentage, dec("100"));
    assert_eq!(
        deliverable.settlement_type,
        Inbound::Known(SettlementType::TPlus1)
    );
    assert_eq!(
        deliverable.settlement_method,
        Inbound::Known(SettlementMethod::Ccc)
    );
    assert!(!deliverable.delayed_settlement);
    assert_eq!(
        deliverable.asset_id.as_deref(),
        Some("b0b6dd9d-8b9b-48a9-ba46-b9d54906e415")
    );
}

#[tokio::test]
async fn the_contract_list_fixtures_decode_field_by_field() {
    let (page, request) = contracts(
        &fixture::upstream(CONTRACTS).body,
        &OptionContractsQuery::new(),
    )
    .await;
    assert_plain_get(&request, "/v2/options/contracts");
    assert_eq!(page.option_contracts.len(), 1);
    assert_bare_contract(&page.option_contracts[0]);
    assert_eq!(page.next_page_token, None);

    let mut query = OptionContractsQuery::new();
    query.underlying_symbols = Some(vec!["AAPL".to_owned(), "SPY".to_owned()]);
    let (page, request) = contracts(&fixture::upstream(CONTRACTS_SYMBOLS).body, &query).await;
    assert_eq!(
        pairs(&request),
        owned(&[("underlying_symbols", "AAPL,SPY")])
    );
    assert_eq!(page.option_contracts.len(), 1);
    assert_bare_contract(&page.option_contracts[0]);
    assert_eq!(page.next_page_token.as_deref(), Some("MA=="));

    let mut query = OptionContractsQuery::new();
    query.show_deliverables = Some(true);
    let (page, request) = contracts(&fixture::upstream(CONTRACTS_DELIVERABLES).body, &query).await;
    assert_eq!(pairs(&request), owned(&[("show_deliverables", "true")]));
    assert_eq!(page.option_contracts.len(), 1);
    assert_standard_deliverable(assert_full_contract(&page.option_contracts[0]));
    assert_eq!(page.next_page_token, None);
}

#[tokio::test]
async fn the_contract_fixtures_decode_field_by_field() {
    let (result, request) =
        read_contract(&fixture::upstream(CONTRACT).body, "AAPL231103C00170000").await;
    assert_plain_get(&request, "/v2/options/contracts/AAPL231103C00170000");
    assert_standard_deliverable(assert_full_contract(&result.unwrap()));

    let (result, _) = read_contract(
        &fixture::upstream(CONTRACT_CASH).body,
        "AAPL231103C00170000",
    )
    .await;
    let contract_value = result.unwrap();
    let cash = assert_full_contract(&contract_value);
    assert_eq!(cash.r#type, Inbound::Known(DeliverableType::Cash));
    assert_eq!(cash.symbol, "USD");
    assert_eq!(cash.amount, Some(dec("2500")));
    assert_eq!(cash.allocation_percentage, dec("100"));
    assert_eq!(cash.settlement_type, Inbound::Known(SettlementType::TPlus1));
    assert_eq!(
        cash.settlement_method,
        Inbound::Known(SettlementMethod::Cafx)
    );
    assert!(!cash.delayed_settlement);
    assert_eq!(cash.asset_id, None);

    let (result, _) = read_contract(
        &fixture::upstream(CONTRACT_DELAYED).body,
        "AAPL231103C00170000",
    )
    .await;
    let contract_value = result.unwrap();
    let delayed = assert_full_contract(&contract_value);
    assert_eq!(delayed.r#type, Inbound::Known(DeliverableType::Equity));
    assert_eq!(delayed.symbol, "AAPL");
    assert_eq!(delayed.amount, None);
    assert_eq!(delayed.allocation_percentage, dec("40"));
    assert_eq!(
        delayed.settlement_type,
        Inbound::Known(SettlementType::TPlus2)
    );
    assert_eq!(
        delayed.settlement_method,
        Inbound::Known(SettlementMethod::Btob)
    );
    assert!(delayed.delayed_settlement);
    assert_eq!(
        delayed.asset_id.as_deref(),
        Some("b0b6dd9d-8b9b-48a9-ba46-b9d54906e415")
    );
}

#[tokio::test]
async fn unknown_contract_values_are_kept_and_unknown_fields_ignored() {
    let body = br#"{"id":"i","symbol":"S","name":"N","status":"halted","tradable":true,"expiration_date":"2023-11-03","underlying_symbol":"U","underlying_asset_id":"u","type":"binary","style":"bermudan","strike_price":"1","size":"1","extra":[1],"deliverables":[{"type":"bond","symbol":"X","amount":"1","allocation_percentage":"1","settlement_type":"T+6","settlement_method":"DVP","delayed_settlement":false,"extra":{}}]}"#;
    let (result, _) = read_contract(body, "S").await;
    let contract = result.unwrap();
    assert_eq!(contract.status, Inbound::Unknown("halted".to_owned()));
    assert_eq!(contract.r#type, Inbound::Unknown("binary".to_owned()));
    assert_eq!(contract.style, Inbound::Unknown("bermudan".to_owned()));
    let deliverable = &contract.deliverables.unwrap()[0];
    assert_eq!(deliverable.r#type, Inbound::Unknown("bond".to_owned()));
    assert_eq!(
        deliverable.settlement_type,
        Inbound::Unknown("T+6".to_owned())
    );
    assert_eq!(
        deliverable.settlement_method,
        Inbound::Unknown("DVP".to_owned())
    );
}

#[tokio::test]
async fn contract_decimals_follow_the_exact_rules() {
    let body = String::from_utf8(fixture::upstream(CONTRACT).body).unwrap();
    let long = "170.12345678901234567890";
    let (result, _) = read_contract(
        body.replace(
            r#""strike_price": "170""#,
            &format!(r#""strike_price": "{long}""#),
        )
        .as_bytes(),
        "S",
    )
    .await;
    assert_eq!(result.unwrap().strike_price.as_str(), long);
    for bad in ["170.5", "\"1.7e2\""] {
        let body = body.replace(
            r#""strike_price": "170""#,
            &format!(r#""strike_price": {bad}"#),
        );
        let (result, _) = read_contract(body.as_bytes(), "S").await;
        assert_eq!(result.unwrap_err().kind(), ErrorKind::Decode, "{bad}");
    }
}

#[tokio::test]
async fn deliverable_decimals_follow_the_exact_rules() {
    let body = String::from_utf8(fixture::upstream(CONTRACT).body).unwrap();
    let amount = r#""amount": "100""#;
    let share = r#""allocation_percentage": "100""#;
    let long = "12345678901234567890.123456789";
    let body_long = body
        .replace(amount, &format!(r#""amount": "{long}""#))
        .replace(share, &format!(r#""allocation_percentage": "{long}""#));
    let (result, _) = read_contract(body_long.as_bytes(), "S").await;
    let contract = result.unwrap();
    let deliverable = &contract.deliverables.as_ref().unwrap()[0];
    assert_eq!(deliverable.amount.as_ref().unwrap().as_str(), long);
    assert_eq!(deliverable.allocation_percentage.as_str(), long);
    for field in [amount, share] {
        assert_eq!(body.matches(field).count(), 1, "{field}");
        let name = field.split(':').next().unwrap();
        for bad in ["100.5", "\"1e2\""] {
            let body = body.replace(field, &format!("{name}: {bad}"));
            let (result, _) = read_contract(body.as_bytes(), "S").await;
            assert_eq!(
                result.unwrap_err().kind(),
                ErrorKind::Decode,
                "{name}: {bad}"
            );
        }
    }
}

#[tokio::test]
async fn an_unknown_field_in_a_contract_page_is_ignored() {
    let body = String::from_utf8(fixture::upstream(CONTRACTS_SYMBOLS).body).unwrap();
    let body = body.replace(
        r#""next_page_token": "MA==""#,
        r#""next_page_token": "MA==", "unknown_member": {"a": [1]}"#,
    );
    assert!(body.contains("unknown_member"));
    let (page, _) = contracts(body.as_bytes(), &OptionContractsQuery::new()).await;
    assert_eq!(page.option_contracts.len(), 1);
    assert_eq!(page.next_page_token.as_deref(), Some("MA=="));
}

#[tokio::test]
async fn every_contract_query_parameter_goes_on_the_wire_in_order() {
    let gte = "100.123456789012345678";
    let lte = "200.987654321098765432";
    let mut query = OptionContractsQuery::new();
    query.underlying_symbols = Some(vec!["AAPL".to_owned(), "SPY".to_owned()]);
    query.show_deliverables = Some(false);
    query.status = Some(AssetStatus::Inactive);
    query.expiration_date = Some(date("2025-06-20"));
    query.expiration_date_gte = Some(date("2025-06-01"));
    query.expiration_date_lte = Some(date("2025-07-31"));
    query.root_symbol = Some("AAPL".to_owned());
    query.r#type = Some(ContractType::Put);
    query.style = Some(ExerciseStyle::European);
    query.strike_price_gte = Some(dec(gte));
    query.strike_price_lte = Some(dec(lte));
    query.page_token = Some("MA==".to_owned());
    query.limit = Some(10000);
    query.ppind = Some(true);
    let (_, request) = contracts(&fixture::upstream(CONTRACTS).body, &query).await;
    assert_eq!(
        pairs(&request),
        owned(&[
            ("underlying_symbols", "AAPL,SPY"),
            ("show_deliverables", "false"),
            ("status", "inactive"),
            ("expiration_date", "2025-06-20"),
            ("expiration_date_gte", "2025-06-01"),
            ("expiration_date_lte", "2025-07-31"),
            ("root_symbol", "AAPL"),
            ("type", "put"),
            ("style", "european"),
            ("strike_price_gte", gte),
            ("strike_price_lte", lte),
            ("page_token", "MA=="),
            ("limit", "10000"),
            ("ppind", "true"),
        ])
    );
}

/// Asserts that each (text, value) pair parses and writes back exactly.
fn round_trip<T>(pairs: &[(&str, T)])
where
    T: std::str::FromStr + PartialEq + std::fmt::Debug + std::fmt::Display,
    T::Err: std::fmt::Debug,
{
    for (text, value) in pairs {
        assert_eq!(&text.parse::<T>().unwrap(), value, "{text}");
        assert_eq!(value.to_string(), *text);
    }
}

#[test]
fn every_documented_value_has_its_wire_text() {
    use AssetClass as C;
    round_trip(&[
        ("us_equity", C::UsEquity),
        ("us_option", C::UsOption),
        ("crypto", C::Crypto),
        ("crypto_perp", C::CryptoPerp),
        ("treasury", C::Treasury),
        ("corporate", C::Corporate),
        ("global_equity", C::GlobalEquity),
        ("us_index", C::UsIndex),
        ("us_equity_chain", C::UsEquityChain),
        ("ipo", C::Ipo),
    ]);
    use Exchange as E;
    round_trip(&[
        ("AMEX", E::Amex),
        ("ARCA", E::Arca),
        ("BATS", E::Bats),
        ("NYSE", E::Nyse),
        ("NASDAQ", E::Nasdaq),
        ("NYSEARCA", E::Nysearca),
        ("OTC", E::Otc),
        ("CRYPTO", E::Crypto),
        ("", E::Empty),
    ]);
    round_trip(&[
        ("active", AssetStatus::Active),
        ("inactive", AssetStatus::Inactive),
    ]);
    use AssetAttribute as A;
    round_trip(&[
        ("ptp_no_exception", A::PtpNoException),
        ("ptp_with_exception", A::PtpWithException),
        ("ipo", A::Ipo),
        ("has_options", A::HasOptions),
        ("options_late_close", A::OptionsLateClose),
        ("fractional_eh_enabled", A::FractionalEhEnabled),
        ("overnight_tradable", A::OvernightTradable),
        ("overnight_halted", A::OvernightHalted),
    ]);
    round_trip(&[
        ("easy_to_borrow", BorrowStatus::EasyToBorrow),
        ("hard_to_borrow", BorrowStatus::HardToBorrow),
    ]);
    round_trip(&[("call", ContractType::Call), ("put", ContractType::Put)]);
    round_trip(&[
        ("american", ExerciseStyle::American),
        ("european", ExerciseStyle::European),
    ]);
    round_trip(&[
        ("cash", DeliverableType::Cash),
        ("equity", DeliverableType::Equity),
    ]);
    use SettlementType as T;
    round_trip(&[
        ("T+0", T::TPlus0),
        ("T+1", T::TPlus1),
        ("T+2", T::TPlus2),
        ("T+3", T::TPlus3),
        ("T+4", T::TPlus4),
        ("T+5", T::TPlus5),
    ]);
    use SettlementMethod as M;
    round_trip(&[
        ("BTOB", M::Btob),
        ("CADF", M::Cadf),
        ("CAFX", M::Cafx),
        ("CCC", M::Ccc),
    ]);
}

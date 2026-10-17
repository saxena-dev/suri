//! The trading account: every field of the upstream fixtures, unknown values
//! and the decimal rules.

#![cfg(feature = "rest")]

#[path = "support/fixture.rs"]
mod fixture;
#[path = "support/server.rs"]
mod server;

use server::{Server, Step, response};
use suri::{
    Account, AccountStatus, Client, Config, Credentials, Decimal, Environment, Error, ErrorKind,
    Inbound, Region, Stage, Timestamp,
};

const FULL: &str =
    "trading/trading_client/test_account_routes::test_get_account::GET /v2/account#0";
const MINIMAL: &str = "trading/trading_client/test_account_routes::test_get_account_parses_buying_power_fields::GET /v2/account#0";

fn client(server: &Server) -> Client {
    let config = Config::new(
        Region::US,
        Environment::Paper,
        Credentials::api_key("PKSENTINELKEY0001", "SECRETSENTINELVALUE0001"),
    )
    .with_base_url(&server.url)
    // One attempt: these tests name the outcome of a single request.
    .with_max_read_attempts(1);
    Client::new(config).unwrap()
}

fn dec(text: &str) -> Option<Decimal> {
    Some(text.parse().unwrap())
}

async fn account_from(body: &[u8]) -> Result<Account, Error> {
    let server = Server::start(vec![Step::Respond(response("200 OK", &[], body))]);
    client(&server)
        .account()
        .await
        .map(|response| response.into_parts().0)
}

#[tokio::test]
#[allow(deprecated)]
async fn the_upstream_account_decodes_field_by_field() {
    let body = fixture::upstream(FULL).body;
    let account = account_from(&body).await.unwrap();
    assert_eq!(account.id, "e6fe16f3-64a4-4921-8928-cadf02f92f98");
    assert_eq!(account.status, Inbound::Known(AccountStatus::Active));
    assert_eq!(account.account_number.as_deref(), Some("010203ABCD"));
    assert_eq!(account.crypto_status, None);
    assert_eq!(account.currency.as_deref(), Some("USD"));
    assert_eq!(
        account.created_at,
        Some("2019-06-12T22:47:07.99658Z".parse::<Timestamp>().unwrap())
    );
    assert_eq!(account.balance_asof, None);
    assert_eq!(account.account_blocked, Some(false));
    assert_eq!(account.shorting_enabled, Some(true));
    assert_eq!(account.trade_suspended_by_user, Some(false));
    assert_eq!(account.trading_blocked, Some(false));
    assert_eq!(account.transfers_blocked, Some(false));
    assert_eq!(account.accrued_fees, dec("0"));
    assert_eq!(account.buying_power, dec("262113.632"));
    assert_eq!(account.cash, dec("-23140.2"));
    assert_eq!(account.equity, dec("103820.56"));
    assert_eq!(account.initial_margin, dec("63480.38"));
    assert_eq!(account.intraday_adjustments, None);
    assert_eq!(account.last_equity, dec("103529.24"));
    assert_eq!(account.last_maintenance_margin, dec("38000.832"));
    assert_eq!(account.long_market_value, dec("126960.76"));
    assert_eq!(account.maintenance_margin, dec("38088.228"));
    assert_eq!(account.multiplier, dec("4"));
    assert_eq!(account.non_marginable_buying_power, dec("98945.02"));
    assert_eq!(account.options_buying_power, dec("262113.632"));
    assert_eq!(account.pending_reg_taf_fees, None);
    assert_eq!(account.pending_transfer_in, None);
    assert_eq!(account.pending_transfer_out, None);
    assert_eq!(account.portfolio_value, dec("103820.56"));
    assert_eq!(account.regt_buying_power, dec("80680.36"));
    assert_eq!(account.short_market_value, dec("0"));
    assert_eq!(account.sma, dec("0"));
    // Sent as the strings "1".
    assert_eq!(account.options_approved_level, Some(1));
    assert_eq!(account.options_trading_level, Some(1));
}

#[tokio::test]
#[allow(deprecated)]
async fn a_minimal_account_leaves_every_optional_field_empty() {
    let body = fixture::upstream(MINIMAL).body;
    let account = account_from(&body).await.unwrap();
    assert_eq!(account.id, "e6fe16f3-64a4-4921-8928-cadf02f92f98");
    assert_eq!(account.status, Inbound::Known(AccountStatus::Active));
    assert_eq!(account.account_number.as_deref(), Some("010203ABCD"));
    // crypto_tier, effective_buying_power and position_market_value are not
    // modelled and are ignored.
    assert_eq!(account.crypto_status, None);
    assert_eq!(account.currency, None);
    assert_eq!(account.created_at, None);
    assert_eq!(account.balance_asof, None);
    assert_eq!(account.account_blocked, None);
    assert_eq!(account.shorting_enabled, None);
    assert_eq!(account.trade_suspended_by_user, None);
    assert_eq!(account.trading_blocked, None);
    assert_eq!(account.transfers_blocked, None);
    for value in [
        &account.accrued_fees,
        &account.buying_power,
        &account.cash,
        &account.equity,
        &account.initial_margin,
        &account.intraday_adjustments,
        &account.last_equity,
        &account.last_maintenance_margin,
        &account.long_market_value,
        &account.maintenance_margin,
        &account.multiplier,
        &account.non_marginable_buying_power,
        &account.options_buying_power,
        &account.pending_reg_taf_fees,
        &account.pending_transfer_in,
        &account.pending_transfer_out,
        &account.portfolio_value,
        &account.regt_buying_power,
        &account.short_market_value,
        &account.sma,
    ] {
        assert_eq!(value, &None);
    }
    assert_eq!(account.options_approved_level, None);
    assert_eq!(account.options_trading_level, None);
}

#[test]
fn every_documented_status_reads_and_writes_its_wire_text() {
    for (text, status) in [
        ("INACTIVE", AccountStatus::Inactive),
        ("PAPER_ONLY", AccountStatus::PaperOnly),
        ("ONBOARDING", AccountStatus::Onboarding),
        ("SUBMISSION_FAILED", AccountStatus::SubmissionFailed),
        ("SUBMITTED", AccountStatus::Submitted),
        ("ACCOUNT_UPDATED", AccountStatus::AccountUpdated),
        ("APPROVAL_PENDING", AccountStatus::ApprovalPending),
        ("ACTIVE", AccountStatus::Active),
        ("REJECTED", AccountStatus::Rejected),
        ("ACCOUNT_CLOSED", AccountStatus::AccountClosed),
        ("APPROVED", AccountStatus::Approved),
        (
            "ACCOUNT_CLOSED_PENDING",
            AccountStatus::AccountClosedPending,
        ),
        ("ACTION_REQUIRED", AccountStatus::ActionRequired),
        ("LIMITED", AccountStatus::Limited),
    ] {
        assert_eq!(text.parse::<AccountStatus>(), Ok(status), "{text}");
        assert_eq!(status.as_str(), text);
    }
}

#[tokio::test]
async fn a_missing_always_present_field_fails_the_call() {
    let error = account_from(br#"{"status":"ACTIVE"}"#).await.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Decode);
}

#[tokio::test]
async fn an_unknown_status_is_kept() {
    let account =
        account_from(br#"{"id":"a","status":"UNDER_REVIEW","crypto_status":"PAPER_ONLY"}"#)
            .await
            .unwrap();
    assert_eq!(account.status, Inbound::Unknown("UNDER_REVIEW".to_owned()));
    assert_eq!(
        account.crypto_status,
        Some(Inbound::Known(AccountStatus::PaperOnly))
    );
}

#[tokio::test]
async fn options_levels_arrive_as_integers_strings_or_nothing() {
    let integers = account_from(
        br#"{"id":"a","status":"ACTIVE","options_approved_level":3,"options_trading_level":2}"#,
    )
    .await
    .unwrap();
    assert_eq!(integers.options_approved_level, Some(3));
    assert_eq!(integers.options_trading_level, Some(2));
    let nulls = account_from(
        br#"{"id":"a","status":"ACTIVE","options_approved_level":null,"options_trading_level":"12"}"#,
    )
    .await
    .unwrap();
    assert_eq!(nulls.options_approved_level, None);
    // A value beyond today's documented range is kept.
    assert_eq!(nulls.options_trading_level, Some(12));
    for body in [
        br#"{"id":"a","status":"ACTIVE","options_trading_level":"x"}"#.as_slice(),
        br#"{"id":"a","status":"ACTIVE","options_trading_level":""}"#,
        br#"{"id":"a","status":"ACTIVE","options_trading_level":"256"}"#,
        // Rust's own integer parse accepts a sign; a digit string does not.
        br#"{"id":"a","status":"ACTIVE","options_trading_level":"+1"}"#,
        br#"{"id":"a","status":"ACTIVE","options_trading_level":-1}"#,
    ] {
        let error = account_from(body).await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Decode);
        assert_eq!(error.stage(), Stage::ResponseReceived);
    }
}

#[tokio::test]
async fn decimals_are_exact_text_and_never_floats() {
    let long = "123456789012345678901234567890.123";
    let body = format!(r#"{{"id":"a","status":"ACTIVE","cash":"{long}"}}"#);
    let account = account_from(body.as_bytes()).await.unwrap();
    assert_eq!(account.cash.unwrap().as_str(), long);
    for body in [
        br#"{"id":"a","status":"ACTIVE","cash":1.5}"#.as_slice(),
        br#"{"id":"a","status":"ACTIVE","cash":"1e5"}"#,
    ] {
        let error = account_from(body).await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Decode);
    }
}

#[tokio::test]
async fn the_request_is_a_plain_get() {
    let body = fixture::upstream(MINIMAL).body;
    let server = Server::start(vec![Step::Respond(response("200 OK", &[], &body))]);
    client(&server).account().await.unwrap();
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path, "/v2/account");
    assert_eq!(requests[0].header("Content-Type"), None);
    assert!(requests[0].body.is_empty());
}

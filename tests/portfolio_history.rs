//! Portfolio history: JSON numbers read as the nearest `f64`, timestamps from
//! Unix seconds, and the window on the wire.

#![cfg(feature = "rest")]

#[path = "support/server.rs"]
mod server;

use server::{Request, Server, Step, response};
use suri::{
    Client, Config, Credentials, Date, Environment, ErrorKind, HistoryRange, IntradayReporting,
    PnlReset, PortfolioHistory, PortfolioHistoryQuery, Region, Timestamp,
};

// The documented examples `example-cashflows` and `example-query-1d-7d`.
const CASHFLOWS: &str = r#"{"unknown_member":true,"base_value":8413.04,"base_value_asof":"2026-04-06","cashflow":{"CSD":[9.93,10.13,10.16,0],"DIV":[0,0,0.63,0]},"equity":[8425.21,8639.77,8766.1,8835.31],"profit_loss":[12.17,214.56,126.33,69.21],"profit_loss_pct":[0.0014,0.0255,0.0146,0.0079],"timeframe":"1D","timestamp":[1775606400,1775692800,1775779200,1775865600]}"#;
const WEEK: &str = r#"{"base_value":2784.79,"cashflow":{"CSD":[0,0,100,0,0]},"equity":[2784.79,2794.79,2805.46,2774.16,2748.73],"profit_loss":[0,10.0022,10.6692,-31.2996,-25.4232],"profit_loss_pct":[0,0.0035,0.0074,-0.0038,-0.0129],"timeframe":"1D","timestamp":[1697241600,1697500800,1697587200,1697673600,1697760000]}"#;

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

async fn history(
    body: &str,
    query: &PortfolioHistoryQuery,
) -> (Result<PortfolioHistory, suri::Error>, Request) {
    let server = Server::start(vec![Step::Respond(response(
        "200 OK",
        &[],
        body.as_bytes(),
    ))]);
    let history = client(&server)
        .portfolio_history(query)
        .await
        .map(|response| response.into_parts().0);
    (history, server.requests().remove(0))
}

fn times(texts: &[&str]) -> Vec<Timestamp> {
    texts.iter().map(|text| text.parse().unwrap()).collect()
}

fn values(numbers: &[f64]) -> Vec<Option<f64>> {
    numbers.iter().copied().map(Some).collect()
}

#[tokio::test]
async fn the_cashflow_example_decodes_value_by_value() {
    let (history, request) = history(CASHFLOWS, &PortfolioHistoryQuery::new()).await;
    let history = history.unwrap();
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, "/v2/account/portfolio/history");
    assert_eq!(
        history.timestamp,
        times(&[
            "2026-04-08T00:00:00Z",
            "2026-04-09T00:00:00Z",
            "2026-04-10T00:00:00Z",
            "2026-04-11T00:00:00Z",
        ])
    );
    assert_eq!(history.equity, values(&[8425.21, 8639.77, 8766.1, 8835.31]));
    assert_eq!(history.profit_loss, values(&[12.17, 214.56, 126.33, 69.21]));
    assert_eq!(
        history.profit_loss_pct,
        values(&[0.0014, 0.0255, 0.0146, 0.0079])
    );
    assert_eq!(history.base_value, Some(8413.04));
    assert_eq!(
        history.base_value_asof,
        Some("2026-04-06".parse::<Date>().unwrap())
    );
    assert_eq!(history.timeframe, "1D");
    assert_eq!(history.cashflow.len(), 2);
    assert_eq!(history.cashflow["CSD"], vec![9.93, 10.13, 10.16, 0.0]);
    assert_eq!(history.cashflow["DIV"], vec![0.0, 0.0, 0.63, 0.0]);
}

#[tokio::test]
async fn the_week_example_decodes_value_by_value() {
    let history = history(WEEK, &PortfolioHistoryQuery::new())
        .await
        .0
        .unwrap();
    assert_eq!(
        history.timestamp,
        times(&[
            "2023-10-14T00:00:00Z",
            "2023-10-17T00:00:00Z",
            "2023-10-18T00:00:00Z",
            "2023-10-19T00:00:00Z",
            "2023-10-20T00:00:00Z",
        ])
    );
    assert_eq!(
        history.equity,
        values(&[2784.79, 2794.79, 2805.46, 2774.16, 2748.73])
    );
    // The integer 0 reads as 0.0.
    assert_eq!(
        history.profit_loss,
        values(&[0.0, 10.0022, 10.6692, -31.2996, -25.4232])
    );
    assert_eq!(
        history.profit_loss_pct,
        values(&[0.0, 0.0035, 0.0074, -0.0038, -0.0129])
    );
    assert_eq!(history.base_value, Some(2784.79));
    assert_eq!(history.base_value_asof, None);
    assert_eq!(history.cashflow["CSD"], vec![0.0, 0.0, 100.0, 0.0, 0.0]);
}

#[tokio::test]
async fn nulls_are_none_and_cashflow_may_be_absent() {
    let body = r#"{"base_value":null,"equity":[null,1.5],"profit_loss":[null,0.25],"profit_loss_pct":[null,null],"timeframe":"15Min","timestamp":[1580826600,1580827500]}"#;
    let history = history(body, &PortfolioHistoryQuery::new())
        .await
        .0
        .unwrap();
    assert_eq!(history.base_value, None);
    assert_eq!(history.equity, vec![None, Some(1.5)]);
    assert_eq!(history.profit_loss, vec![None, Some(0.25)]);
    assert_eq!(history.profit_loss_pct, vec![None, None]);
    assert!(history.cashflow.is_empty());
    assert_eq!(
        history.timestamp,
        times(&["2020-02-04T14:30:00Z", "2020-02-04T14:45:00Z"])
    );
}

#[tokio::test]
async fn numbers_are_read_to_the_correctly_rounded_nearest_f64() {
    // Each sample decodes to exactly the f64 Rust's own parser gives,
    // compared bit for bit; the third is misread without serde_json's
    // float_roundtrip feature.
    let samples = [
        "8425.21",
        "0.30000000000000004",
        "1.1368683772161603e-13",
        "9007199254740993",
    ];
    let body = format!(
        r#"{{"base_value":0,"equity":[{}],"profit_loss":[],"profit_loss_pct":[],"timeframe":"1D","timestamp":[]}}"#,
        samples.join(",")
    );
    let history = history(&body, &PortfolioHistoryQuery::new())
        .await
        .0
        .unwrap();
    for (sample, value) in samples.iter().zip(&history.equity) {
        let expected: f64 = sample.parse().unwrap();
        assert_eq!(value.unwrap().to_bits(), expected.to_bits(), "{sample}");
    }
    // 9007199254740993 is not a double; it rounds to the nearest one.
    assert_eq!(history.equity[3], Some(9007199254740992.0));
}

#[tokio::test]
async fn millisecond_timestamps_fail_the_decode() {
    let body = r#"{"base_value":0,"equity":[1],"profit_loss":[0],"profit_loss_pct":[0],"timeframe":"15Min","timestamp":[1580826600000]}"#;
    let error = history(body, &PortfolioHistoryQuery::new())
        .await
        .0
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Decode);
}

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

#[tokio::test]
#[allow(deprecated)]
async fn every_query_parameter_goes_on_the_wire_in_the_documented_order() {
    let mut query = PortfolioHistoryQuery::new();
    query.range = Some(HistoryRange::PeriodEnding {
        period: "1M".to_owned(),
        end: "2024-02-01T21:00:00Z".parse().unwrap(),
    });
    query.timeframe = Some("1D".to_owned());
    query.intraday_reporting = Some(IntradayReporting::ExtendedHours);
    query.pnl_reset = Some(PnlReset::NoReset);
    query.extended_hours = Some(true);
    query.cashflow_types = Some("CSD,DIV".to_owned());
    let (_, request) = history(WEEK, &query).await;
    assert_eq!(
        pairs(&request),
        owned(&[
            ("period", "1M"),
            ("timeframe", "1D"),
            ("intraday_reporting", "extended_hours"),
            ("pnl_reset", "no_reset"),
            ("end", "2024-02-01T21:00:00Z"),
            ("extended_hours", "true"),
            ("cashflow_types", "CSD,DIV"),
        ])
    );
}

#[tokio::test]
#[allow(deprecated)]
async fn a_start_takes_its_documented_place_among_the_other_parameters() {
    let mut query = PortfolioHistoryQuery::new();
    query.range = Some(HistoryRange::Between {
        start: "2024-01-02T14:30:00Z".parse().unwrap(),
        end: "2024-01-05T21:00:00Z".parse().unwrap(),
    });
    query.timeframe = Some("15Min".to_owned());
    query.intraday_reporting = Some(IntradayReporting::Continuous);
    query.pnl_reset = Some(PnlReset::PerDay);
    query.extended_hours = Some(false);
    query.cashflow_types = Some("ALL".to_owned());
    let (_, request) = history(WEEK, &query).await;
    assert_eq!(
        pairs(&request),
        owned(&[
            ("timeframe", "15Min"),
            ("intraday_reporting", "continuous"),
            ("start", "2024-01-02T14:30:00Z"),
            ("pnl_reset", "per_day"),
            ("end", "2024-01-05T21:00:00Z"),
            ("extended_hours", "false"),
            ("cashflow_types", "ALL"),
        ])
    );
}

#[tokio::test]
async fn the_intraday_example_cashflow_integers_read_as_numbers() {
    // Shaped like the documented intraday example (15-minute windows from
    // 09:30 New York time, integer cashflow values), shortened to four.
    let body = r#"{"base_value":1000,"cashflow":{"CSD":[0,0,100,0]},"equity":[1000,1000,1100,1100],"profit_loss":[0,0,0,0],"profit_loss_pct":[0,0,0,0],"timeframe":"15Min","timestamp":[1697722200,1697723100,1697724000,1697724900]}"#;
    let history = history(body, &PortfolioHistoryQuery::new())
        .await
        .0
        .unwrap();
    assert_eq!(history.cashflow["CSD"], vec![0.0, 0.0, 100.0, 0.0]);
    assert_eq!(history.base_value, Some(1000.0));
    assert_eq!(
        history.timestamp,
        times(&[
            "2023-10-19T13:30:00Z",
            "2023-10-19T13:45:00Z",
            "2023-10-19T14:00:00Z",
            "2023-10-19T14:15:00Z",
        ])
    );
}

#[tokio::test]
async fn each_range_sends_its_own_parameters() {
    let start: Timestamp = "2024-01-02T14:30:00Z".parse().unwrap();
    let end: Timestamp = "2024-01-05T21:00:00Z".parse().unwrap();
    for (range, expected) in [
        (
            HistoryRange::Period("7D".to_owned()),
            vec![("period", "7D")],
        ),
        (
            HistoryRange::Start(start),
            vec![("start", "2024-01-02T14:30:00Z")],
        ),
        (
            HistoryRange::End(end),
            vec![("end", "2024-01-05T21:00:00Z")],
        ),
        (
            HistoryRange::PeriodEnding {
                period: "1W".to_owned(),
                end,
            },
            vec![("period", "1W"), ("end", "2024-01-05T21:00:00Z")],
        ),
        (
            HistoryRange::PeriodStarting {
                start,
                period: "1W".to_owned(),
            },
            vec![("period", "1W"), ("start", "2024-01-02T14:30:00Z")],
        ),
        (
            HistoryRange::Between { start, end },
            vec![
                ("start", "2024-01-02T14:30:00Z"),
                ("end", "2024-01-05T21:00:00Z"),
            ],
        ),
    ] {
        let mut query = PortfolioHistoryQuery::new();
        query.range = Some(range);
        let (_, request) = history(WEEK, &query).await;
        assert_eq!(pairs(&request), owned(&expected));
    }
}

#[test]
fn every_documented_value_has_its_wire_text() {
    for (text, value) in [
        ("market_hours", IntradayReporting::MarketHours),
        ("extended_hours", IntradayReporting::ExtendedHours),
        ("continuous", IntradayReporting::Continuous),
    ] {
        assert_eq!(text.parse::<IntradayReporting>(), Ok(value));
        assert_eq!(value.as_str(), text);
    }
    for (text, value) in [
        ("no_reset", PnlReset::NoReset),
        ("per_day", PnlReset::PerDay),
    ] {
        assert_eq!(text.parse::<PnlReset>(), Ok(value));
        assert_eq!(value.as_str(), text);
    }
}

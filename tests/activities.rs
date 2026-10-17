//! Account activities: fills and other activities in one list, the query
//! on the wire, and the decimal rules.

#![cfg(feature = "rest")]

#[path = "support/server.rs"]
mod server;

use server::{Request, Server, Step, response};
use suri::{
    ActivitiesQuery, Activity, ActivityCategory, ActivityFilter, ActivityStatus, ActivityType,
    Client, Config, Credentials, Date, Decimal, Environment, ErrorKind, FillType, Id, Inbound,
    OrderSide, OrderStatus, Region, Sort, Timestamp,
};

// Built from the two schema examples of the activities reference page.
const MIXED: &str = r#"[
  {"activity_type":"FILL","cum_qty":"7","id":"20220202135509981::2d7be4ff-d1f3-43e9-856a-0f5cf5c5088e","leaves_qty":"3","unknown_member":1,"order_id":"b5abe576-6a8a-49f3-a353-46b72c1ccae9","order_status":"filled","price":"174.78","qty":"2","side":"buy","symbol":"AAPL","transaction_time":"2022-02-02T18:55:09.981482Z","type":"fill"},
  {"activity_sub_type":"SDIV","activity_type":"DIV","created_at":"2021-05-10T14:01:04.650275Z","currency":"USD","cusip":"00206R102","date":"2019-08-01","group_id":"g-1","id":"20190801011955195::5f596936-6f23-4cef-bdf1-3806aae57dbf","net_amount":"1.02","per_share_amount":"0.51","qty":"2","status":"executed","symbol":"T","another_unknown":"x"}
]"#;

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

fn dec(text: &str) -> Option<Decimal> {
    Some(text.parse().unwrap())
}

fn at(text: &str) -> Option<Timestamp> {
    Some(text.parse().unwrap())
}

async fn activities(body: &str, query: &ActivitiesQuery) -> (Vec<Activity>, Request) {
    let server = Server::start(vec![Step::Respond(response(
        "200 OK",
        &[],
        body.as_bytes(),
    ))]);
    let list = client(&server)
        .account_activities(query)
        .await
        .unwrap()
        .into_parts()
        .0;
    let request = server.requests().remove(0);
    (list, request)
}

/// The query's pairs, decoded, in order.
fn pairs(request: &Request) -> Vec<(String, String)> {
    let url = url::Url::parse(&format!("http://h{}", request.path)).unwrap();
    url.query_pairs()
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect()
}

#[tokio::test]
async fn fills_and_other_activities_decode_field_by_field() {
    let (list, request) = activities(MIXED, &ActivitiesQuery::new()).await;
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, "/v2/account/activities");
    assert_eq!(list.len(), 2);

    let Activity::Trade(fill) = &list[0] else {
        panic!("not a fill: {:?}", list[0]);
    };
    assert_eq!(
        fill.id,
        "20220202135509981::2d7be4ff-d1f3-43e9-856a-0f5cf5c5088e"
    );
    assert_eq!(fill.activity_type, Inbound::Known(ActivityType::Fill));
    assert_eq!(fill.cum_qty, dec("7"));
    assert_eq!(fill.leaves_qty, dec("3"));
    assert_eq!(fill.price, dec("174.78"));
    assert_eq!(fill.qty, dec("2"));
    assert_eq!(
        fill.order_id.as_deref(),
        Some("b5abe576-6a8a-49f3-a353-46b72c1ccae9")
    );
    assert_eq!(fill.symbol.as_deref(), Some("AAPL"));
    assert_eq!(fill.order_status, Some(Inbound::Known(OrderStatus::Filled)));
    assert_eq!(fill.side, Some(Inbound::Known(OrderSide::Buy)));
    assert_eq!(fill.transaction_time, at("2022-02-02T18:55:09.981482Z"));
    assert_eq!(fill.r#type, Some(Inbound::Known(FillType::Fill)));

    let Activity::NonTrade(dividend) = &list[1] else {
        panic!("not a non-trade activity: {:?}", list[1]);
    };
    assert_eq!(
        dividend.id,
        "20190801011955195::5f596936-6f23-4cef-bdf1-3806aae57dbf"
    );
    assert_eq!(dividend.activity_type, Inbound::Known(ActivityType::Div));
    assert_eq!(dividend.activity_sub_type.as_deref(), Some("SDIV"));
    assert_eq!(dividend.created_at, at("2021-05-10T14:01:04.650275Z"));
    assert_eq!(dividend.currency.as_deref(), Some("USD"));
    assert_eq!(dividend.cusip.as_deref(), Some("00206R102"));
    assert_eq!(dividend.group_id.as_deref(), Some("g-1"));
    assert_eq!(dividend.symbol.as_deref(), Some("T"));
    assert_eq!(dividend.date, Some("2019-08-01".parse::<Date>().unwrap()));
    assert_eq!(dividend.net_amount, dec("1.02"));
    assert_eq!(dividend.per_share_amount, dec("0.51"));
    assert_eq!(dividend.qty, dec("2"));
    assert_eq!(
        dividend.status,
        Some(Inbound::Known(ActivityStatus::Executed))
    );
}

#[tokio::test]
async fn an_unknown_type_is_a_non_trade_activity_that_keeps_its_text() {
    let (list, _) = activities(
        r#"[{"id":"a::b","activity_type":"XYZ","net_amount":"-1.5"}]"#,
        &ActivitiesQuery::new(),
    )
    .await;
    let Activity::NonTrade(activity) = &list[0] else {
        panic!("not a non-trade activity");
    };
    assert_eq!(activity.activity_type, Inbound::Unknown("XYZ".to_owned()));
    assert_eq!(activity.net_amount, dec("-1.5"));
    assert_eq!(activity.date, None);
}

#[tokio::test]
async fn an_item_without_its_type_or_with_a_bad_number_fails_the_list() {
    for body in [
        r#"[{"id":"a::b"}]"#,
        r#"[{"id":"a::b","activity_type":"DIV","qty":-0}]"#,
        r#"[{"id":"a::b","activity_type":"DIV","qty":1.5}]"#,
        r#"[{"id":"a::b","activity_type":"DIV","qty":"2e3"}]"#,
    ] {
        let server = Server::start(vec![Step::Respond(response(
            "200 OK",
            &[],
            body.as_bytes(),
        ))]);
        let error = client(&server)
            .account_activities(&ActivitiesQuery::new())
            .await
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Decode, "{body}");
    }
}

#[tokio::test]
async fn a_long_amount_is_kept_exactly() {
    let long = "1234567890.1234567890123456789";
    let body = format!(r#"[{{"id":"a::b","activity_type":"CSD","net_amount":"{long}"}}]"#);
    let (list, _) = activities(&body, &ActivitiesQuery::new()).await;
    let Activity::NonTrade(activity) = &list[0] else {
        panic!("not a non-trade activity");
    };
    assert_eq!(activity.net_amount.as_ref().unwrap().as_str(), long);
}

#[tokio::test]
async fn every_query_parameter_goes_on_the_wire_in_order() {
    let mut query = ActivitiesQuery::new();
    query.filter = Some(ActivityFilter::Types(vec![
        ActivityType::Fill,
        ActivityType::Div,
    ]));
    query.order_id = Some(
        "b5abe576-6a8a-49f3-a353-46b72c1ccae9"
            .parse::<Id>()
            .unwrap(),
    );
    query.date = Some("2024-01-02".parse().unwrap());
    query.until = at("2024-01-03T00:00:00Z");
    query.after = at("2024-01-01T12:30:00.5Z");
    query.direction = Some(Sort::Asc);
    query.page_size = Some(50);
    query.page_token = Some("20220202135509981::2d7be4ff".to_owned());
    let (_, request) = activities("[]", &query).await;
    let expected: Vec<(String, String)> = [
        ("activity_types", "FILL,DIV"),
        ("order_id", "b5abe576-6a8a-49f3-a353-46b72c1ccae9"),
        ("date", "2024-01-02"),
        ("until", "2024-01-03T00:00:00Z"),
        ("after", "2024-01-01T12:30:00.5Z"),
        ("direction", "asc"),
        ("page_size", "50"),
        ("page_token", "20220202135509981::2d7be4ff"),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value.to_owned()))
    .collect();
    assert_eq!(pairs(&request), expected);
}

#[tokio::test]
async fn a_category_is_sent_in_place_of_types() {
    let mut query = ActivitiesQuery::new();
    query.filter = Some(ActivityFilter::Category(ActivityCategory::NonTradeActivity));
    query.order_id = Some(
        "b5abe576-6a8a-49f3-a353-46b72c1ccae9"
            .parse::<Id>()
            .unwrap(),
    );
    query.page_size = Some(10);
    let (_, request) = activities("[]", &query).await;
    let expected: Vec<(String, String)> = [
        ("category", "non_trade_activity"),
        ("order_id", "b5abe576-6a8a-49f3-a353-46b72c1ccae9"),
        ("page_size", "10"),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value.to_owned()))
    .collect();
    assert_eq!(pairs(&request), expected);
}

#[tokio::test]
async fn no_query_leaves_no_question_mark() {
    let (_, request) = activities("[]", &ActivitiesQuery::new()).await;
    assert_eq!(request.path, "/v2/account/activities");
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
    use ActivityType as A;
    round_trip(&[
        ("FILL", A::Fill),
        ("TRANS", A::Trans),
        ("MISC", A::Misc),
        ("ACATC", A::Acatc),
        ("ACATS", A::Acats),
        ("CFEE", A::Cfee),
        ("CGD", A::Cgd),
        ("CSD", A::Csd),
        ("CSW", A::Csw),
        ("DIV", A::Div),
        ("DIVCGL", A::Divcgl),
        ("DIVCGS", A::Divcgs),
        ("DIVFEE", A::Divfee),
        ("DIVFT", A::Divft),
        ("DIVNRA", A::Divnra),
        ("DIVROC", A::Divroc),
        ("DIVTW", A::Divtw),
        ("DIVTXEX", A::Divtxex),
        ("FEE", A::Fee),
        ("INT", A::Int),
        ("INTNRA", A::Intnra),
        ("INTTW", A::Inttw),
        ("JNL", A::Jnl),
        ("JNLC", A::Jnlc),
        ("JNLS", A::Jnls),
        ("MA", A::Ma),
        ("NC", A::Nc),
        ("OPASN", A::Opasn),
        ("OPCA", A::Opca),
        ("OPCSH", A::Opcsh),
        ("OPEXC", A::Opexc),
        ("OPEXP", A::Opexp),
        ("OPTRD", A::Optrd),
        ("PTC", A::Ptc),
        ("PTR", A::Ptr),
        ("REO", A::Reo),
        ("REORG", A::Reorg),
        ("SPIN", A::Spin),
        ("SPLIT", A::Split),
        ("FOPT", A::Fopt),
        ("OCT", A::Oct),
    ]);
    use OrderStatus as S;
    round_trip(&[
        ("new", S::New),
        ("partially_filled", S::PartiallyFilled),
        ("filled", S::Filled),
        ("done_for_day", S::DoneForDay),
        ("canceled", S::Canceled),
        ("expired", S::Expired),
        ("replaced", S::Replaced),
        ("pending_cancel", S::PendingCancel),
        ("pending_replace", S::PendingReplace),
        ("accepted", S::Accepted),
        ("pending_new", S::PendingNew),
        ("accepted_for_bidding", S::AcceptedForBidding),
        ("stopped", S::Stopped),
        ("rejected", S::Rejected),
        ("suspended", S::Suspended),
        ("calculated", S::Calculated),
        ("held", S::Held),
    ]);
    round_trip(&[("buy", OrderSide::Buy), ("sell", OrderSide::Sell)]);
    round_trip(&[
        ("fill", FillType::Fill),
        ("partial_fill", FillType::PartialFill),
    ]);
    round_trip(&[
        ("executed", ActivityStatus::Executed),
        ("correct", ActivityStatus::Correct),
        ("canceled", ActivityStatus::Canceled),
    ]);
    round_trip(&[
        ("trade_activity", ActivityCategory::TradeActivity),
        ("non_trade_activity", ActivityCategory::NonTradeActivity),
    ]);
    round_trip(&[("asc", Sort::Asc), ("desc", Sort::Desc)]);
}

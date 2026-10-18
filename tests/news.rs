//! News: the upstream fixtures field by field, the documented example,
//! optional and unknown members, and every query parameter on the wire.

#![cfg(feature = "rest")]

#[path = "support/fixture.rs"]
mod fixture;
#[path = "support/requests.rs"]
mod requests;
#[path = "support/server.rs"]
mod server;
#[path = "support/stock.rs"]
mod stock;

use requests::{client, only_request, owned, pairs, path, serve};
use server::Server;
use stock::at;
use suri::data::news::{Article, ImageSize, Page, Query};
use suri::{ErrorKind, Inbound, Sort};

fn serve_fixture(index: u32) -> Server {
    let id = format!("data/test_historical_news_data::test_get_news::GET /v1beta1/news#{index}");
    let fixture = fixture::upstream(&id);
    assert_eq!(fixture.status, 200);
    serve("200 OK", &fixture.body)
}

async fn read(server: &Server) -> Page {
    client(server)
        .news(&Query::new())
        .await
        .unwrap()
        .into_parts()
        .0
}

fn images(article: &Article) -> Vec<(Inbound<ImageSize>, &str)> {
    article
        .images
        .iter()
        .map(|image| (image.size.clone(), image.url.as_str()))
        .collect()
}

#[tokio::test]
async fn the_fixtures_are_read_field_by_field() {
    let server = serve_fixture(0);
    let page = read(&server).await;
    let request = only_request(&server);
    assert_eq!(request.method, "GET");
    assert_eq!(path(&request), "/v1beta1/news");
    assert!(pairs(&request).is_empty());
    assert_eq!(page.news.len(), 1);
    let article = &page.news[0];
    assert_eq!(article.id, 40432158);
    assert_eq!(article.headline, "headline");
    assert_eq!(article.author, "John");
    assert_eq!(article.created_at, at("2024-08-18T20:15:44Z"));
    assert_eq!(article.updated_at, at("2024-08-18T20:15:44Z"));
    assert_eq!(article.summary, "summary");
    // Empty content, although the schema forbids it.
    assert_eq!(article.content, "");
    assert_eq!(article.url.as_deref(), Some("https://"));
    assert_eq!(article.symbols, ["AAPL", "QCOM"]);
    assert_eq!(article.source, "source");
    assert_eq!(
        images(article),
        [
            (Inbound::Known(ImageSize::Large), "https://"),
            (Inbound::Known(ImageSize::Small), "https://"),
            (Inbound::Known(ImageSize::Thumb), "https://"),
        ]
    );
    assert_eq!(
        page.next_page_token.as_deref(),
        Some("MTczMDk3MTEwMTAwMDAwMDAwMHw0MTc5OTExNQ==")
    );

    let page = read(&serve_fixture(1)).await;
    assert_eq!(page.news.len(), 1);
    let article = &page.news[0];
    assert_eq!(article.id, 41799100);
    assert_eq!(
        article.headline,
        "Piper Sandler Maintains Neutral on Clearwater Analytics Hldg, Raises Price Target to $28"
    );
    assert_eq!(article.author, "Benzinga Newsdesk");
    assert_eq!(article.created_at, at("2024-11-07T09:16:41Z"));
    assert_eq!(article.updated_at, at("2024-11-07T09:16:41Z"));
    assert_eq!(article.summary, "");
    assert_eq!(article.content, "");
    assert_eq!(
        article.url.as_deref(),
        Some(
            "https://www.benzinga.com/news/24/11/41799100/piper-sandler-maintains-neutral-on-clearwater-analytics-hldg-raises-price-target-to-28"
        )
    );
    assert_eq!(article.symbols, ["CWAN"]);
    assert_eq!(article.source, "benzinga");
    assert!(article.images.is_empty());
    assert_eq!(page.next_page_token, None);
}

// The documented example, as sent.
const EXAMPLE: &str = r#"{"news":[{"author":"Charles Gross","content":"<p>This headline-only article is meant to show you why a stock is moving, the most difficult aspect of stock trading....</p>","created_at":"2021-12-31T11:08:42Z","headline":"Apple Leader in Phone Sales in China for Second Straight Month in November With 23.6% Share, According to Market Research Data","id":24843171,"images":[],"source":"benzinga","summary":"This headline-only article is meant to show you why a stock is moving, the most difficult aspect of stock trading","symbols":["AAPL"],"updated_at":"2021-12-31T11:08:43Z","url":"https://www.benzinga.com/news/21/12/24843171/apple-leader-in-phone-sales-in-china-for-second-straight-month-in-november-with-23-6-share-according"}],"next_page_token":"MTY0MDk0ODkyMzAwMDAwMDAwMHwyNDg0MzE3MQ=="}"#;

#[tokio::test]
async fn the_documented_example_is_read_as_sent() {
    let page = read(&serve("200 OK", EXAMPLE.as_bytes())).await;
    let article = &page.news[0];
    assert_eq!(article.id, 24843171);
    assert_eq!(article.author, "Charles Gross");
    // HTML content is kept as sent.
    assert_eq!(
        article.content,
        "<p>This headline-only article is meant to show you why a stock is moving, the most difficult aspect of stock trading....</p>"
    );
    assert_eq!(article.created_at, at("2021-12-31T11:08:42Z"));
    assert_eq!(article.updated_at, at("2021-12-31T11:08:43Z"));
    assert!(article.headline.starts_with("Apple Leader in Phone Sales"));
    assert_eq!(
        article.summary,
        "This headline-only article is meant to show you why a stock is moving, the most difficult aspect of stock trading"
    );
    assert!(article.images.is_empty());
    assert_eq!(article.source, "benzinga");
    assert_eq!(article.symbols, ["AAPL"]);
    assert!(
        article
            .url
            .as_deref()
            .unwrap()
            .starts_with("https://www.benzinga.com/news/21/12/24843171/")
    );
    assert_eq!(
        page.next_page_token.as_deref(),
        Some("MTY0MDk0ODkyMzAwMDAwMDAwMHwyNDg0MzE3MQ==")
    );
}

const ARTICLE: &str = r#""id":1,"headline":"h","author":"a","created_at":"2024-01-03T00:00:00Z","updated_at":"2024-01-03T00:00:01Z","summary":"s","content":"c","symbols":[],"source":"benzinga""#;

#[tokio::test]
async fn optional_and_unknown_members_are_read() {
    let body = format!(
        r#"{{"news":[{{{ARTICLE},"images":[]}},{{{ARTICLE},"url":null,"images":[]}},{{{ARTICLE},"url":"https://x"}},{{{ARTICLE},"url":"https://y","images":[{{"size":"huge","url":"https://i","new":1}}],"new":{{}}}}],"next_page_token":null,"new":[]}}"#
    );
    let page = read(&serve("200 OK", body.as_bytes())).await;
    assert_eq!(page.news.len(), 4);
    // Without a URL, or with a null one.
    assert_eq!(page.news[0].url, None);
    assert_eq!(page.news[1].url, None);
    // Without images, as the stream sends articles.
    assert_eq!(page.news[2].url.as_deref(), Some("https://x"));
    assert!(page.news[2].images.is_empty());
    // An image size Alpaca has not documented is kept.
    assert_eq!(
        images(&page.news[3]),
        [(Inbound::Unknown("huge".to_owned()), "https://i")]
    );
    assert_eq!(page.news[3].created_at, at("2024-01-03T00:00:00Z"));
    assert_eq!(page.news[3].updated_at, at("2024-01-03T00:00:01Z"));
}

#[tokio::test]
async fn a_page_or_article_that_breaks_the_schema_fails() {
    for body in [
        r#"{"next_page_token":null}"#.to_owned(),
        r#"{"news":null,"next_page_token":null}"#.to_owned(),
        format!(
            r#"{{"news":[{{{}}}]}}"#,
            ARTICLE.replace(r#""id":1"#, r#""id":1.5"#)
        ),
        format!(
            r#"{{"news":[{{{}}}]}}"#,
            ARTICLE.replace(r#","source":"benzinga""#, "")
        ),
    ] {
        let server = serve("200 OK", body.as_bytes());
        let error = client(&server).news(&Query::new()).await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Decode, "{body}");
    }
}

#[tokio::test]
async fn every_parameter_goes_out_as_documented() {
    let mut query = Query::new();
    query.start = Some(at("2024-01-03T01:02:03.123456789Z"));
    query.end = Some(at("2024-01-04T00:00:00Z"));
    query.sort = Some(Sort::Asc);
    query.symbols = Some(vec![
        "AAPL".to_owned(),
        "TSLA".to_owned(),
        "BTCUSD".to_owned(),
    ]);
    query.limit = Some(50);
    query.include_content = Some(true);
    query.exclude_contentless = Some(false);
    // A token passed back byte for byte, its `=` and `+` included.
    query.page_token = Some("MTY0MDk0ODkyMzAwMDAwMDAwMHwyNDg+NDMxNzE=".to_owned());
    let server = serve("200 OK", br#"{"news":[],"next_page_token":null}"#);
    client(&server).news(&query).await.unwrap();
    let request = only_request(&server);
    assert_eq!(request.method, "GET");
    assert_eq!(path(&request), "/v1beta1/news");
    assert_eq!(
        pairs(&request),
        owned(&[
            ("start", "2024-01-03T01:02:03.123456789Z"),
            ("end", "2024-01-04T00:00:00Z"),
            ("sort", "asc"),
            ("symbols", "AAPL,TSLA,BTCUSD"),
            ("limit", "50"),
            ("include_content", "true"),
            ("exclude_contentless", "false"),
            ("page_token", "MTY0MDk0ODkyMzAwMDAwMDAwMHwyNDg+NDMxNzE="),
        ])
    );

    // The other spelling of each boolean.
    let mut query = Query::new();
    query.include_content = Some(false);
    query.exclude_contentless = Some(true);
    let server = serve("200 OK", br#"{"news":[],"next_page_token":null}"#);
    client(&server).news(&query).await.unwrap();
    assert_eq!(
        pairs(&only_request(&server)),
        owned(&[
            ("include_content", "false"),
            ("exclude_contentless", "true")
        ])
    );
}

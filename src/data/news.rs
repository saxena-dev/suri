//! News: articles across stocks and crypto.
//!
//! `Client::news` returns these types.

use reqwest::Method;
use serde::Deserialize;

use crate::rest::client::{Endpoint, Host, Request};
use crate::value::wire::wire_enum;
use crate::{Client, Error, Inbound, Response, Sort, Timestamp};

wire_enum! {
    /// The display size an image suits.
    ImageSize {
        /// A thumbnail.
        Thumb => "thumb",
        /// Small.
        Small => "small",
        /// Large.
        Large => "large",
    }
}

/// Which articles to read. One call returns one page; with nothing set,
/// Alpaca returns the latest 10 articles.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Query {
    /// The start of the interval, inclusive.
    pub start: Option<Timestamp>,
    /// The end of the interval, inclusive.
    pub end: Option<Timestamp>,
    /// The order by update time (Alpaca's default: descending).
    pub sort: Option<Sort>,
    /// Only articles about these symbols, sent comma-joined.
    pub symbols: Option<Vec<String>>,
    /// The most articles in the page (Alpaca: 1 to 50).
    pub limit: Option<u32>,
    /// Whether to include each article's content, when it has one.
    pub include_content: Option<bool>,
    /// Whether to leave out articles without content.
    pub exclude_contentless: Option<bool>,
    /// The page to read, from a previous page's `next_page_token`.
    pub page_token: Option<String>,
}

impl Query {
    /// The latest articles, every parameter Alpaca's default.
    pub fn new() -> Query {
        Query::default()
    }
}

/// One page of articles.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Page {
    /// The articles.
    pub news: Vec<Article>,
    /// The token for the next page; `None` on the last page.
    pub next_page_token: Option<String>,
}

/// A news article. Alpaca's schema forbids empty text, but its responses
/// carry empty summaries and contents, so any text is read as sent.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Article {
    /// The article ID.
    pub id: i64,
    /// The headline.
    pub headline: String,
    /// The original author.
    pub author: String,
    /// When the article was created.
    pub created_at: Timestamp,
    /// When the article was updated.
    pub updated_at: Timestamp,
    /// The summary, which may be the content's first sentence.
    pub summary: String,
    /// The content, which may contain HTML.
    pub content: String,
    /// The article's URL, if it has one.
    pub url: Option<String>,
    /// The symbols the article is about or mentions.
    pub symbols: Vec<String>,
    /// Where the article comes from, such as `benzinga`.
    pub source: String,
    /// Images for the article, possibly none; empty when Alpaca leaves the
    /// list out.
    #[serde(default)]
    pub images: Vec<Image>,
}

/// An image for an article.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Image {
    /// The display size it suits.
    pub size: Inbound<ImageSize>,
    /// Its URL.
    pub url: String,
}

const NEWS: Endpoint = Endpoint {
    host: Host::Data,
    method: Method::GET,
    template: "/v1beta1/news",
};

impl Client {
    /// Reads one page of news articles (`GET /v1beta1/news`).
    pub async fn news(&self, query: &Query) -> Result<Response<Page>, Error> {
        let mut request = Request::new(&NEWS, &["v1beta1", "news"]);
        if let Some(start) = &query.start {
            request.pair("start", start.to_string());
        }
        if let Some(end) = &query.end {
            request.pair("end", end.to_string());
        }
        if let Some(sort) = &query.sort {
            request.pair("sort", sort.as_str());
        }
        if let Some(symbols) = &query.symbols {
            request.pair("symbols", symbols.join(","));
        }
        if let Some(limit) = query.limit {
            request.pair("limit", limit.to_string());
        }
        if let Some(include_content) = query.include_content {
            request.pair("include_content", include_content.to_string());
        }
        if let Some(exclude_contentless) = query.exclude_contentless {
            request.pair("exclude_contentless", exclude_contentless.to_string());
        }
        if let Some(page_token) = &query.page_token {
            request.pair("page_token", page_token.as_str());
        }
        self.send_request(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_news_endpoint_is_on_the_data_host() {
        assert!(matches!(NEWS.host, Host::Data));
    }
}

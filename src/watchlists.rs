//! Watchlists: named, ordered lists of assets.

use reqwest::Method;
use serde::{Deserialize, Serialize, Serializer};

use crate::client::{Endpoint, Request};
use crate::{Asset, Client, Error, Id, Response, Symbol, Timestamp};

/// A watchlist.
///
/// Fields Alpaca adds later are ignored until suri models them.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Watchlist {
    /// The watchlist ID.
    pub id: String,
    /// The account ID.
    pub account_id: String,
    /// The watchlist's name.
    pub name: String,
    /// When the watchlist was created.
    pub created_at: Timestamp,
    /// When the watchlist was last updated.
    pub updated_at: Timestamp,
    /// The assets, in the order they were added; absent from a list of
    /// watchlists.
    pub assets: Option<Vec<Asset>>,
}

/// A watchlist to create.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CreateWatchlist {
    /// The name.
    pub name: String,
    /// The symbols to start it with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbols: Option<Vec<String>>,
}

impl CreateWatchlist {
    /// An empty watchlist with this name.
    pub fn new(name: impl Into<String>) -> CreateWatchlist {
        CreateWatchlist {
            name: name.into(),
            symbols: None,
        }
    }
}

/// A change to a watchlist: a new name, a new list of symbols replacing the
/// old one, or both.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum WatchlistUpdate {
    /// A new name.
    Name(String),
    /// A new list of symbols; an empty list empties the watchlist.
    Symbols(Vec<String>),
    /// A new name and a new list of symbols.
    NameAndSymbols {
        /// The new name.
        name: String,
        /// The new list of symbols.
        symbols: Vec<String>,
    },
}

/// The body of a watchlist update.
#[derive(Serialize)]
struct WireUpdate<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    symbols: Option<&'a [String]>,
}

impl Serialize for WatchlistUpdate {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let (name, symbols) = match self {
            WatchlistUpdate::Name(name) => (Some(name.as_str()), None),
            WatchlistUpdate::Symbols(symbols) => (None, Some(symbols.as_slice())),
            WatchlistUpdate::NameAndSymbols { name, symbols } => {
                (Some(name.as_str()), Some(symbols.as_slice()))
            }
        };
        WireUpdate { name, symbols }.serialize(serializer)
    }
}

/// The body that adds one asset to a watchlist.
#[derive(Serialize)]
struct WireAdd<'a> {
    symbol: &'a str,
}

const WATCHLISTS: Endpoint = Endpoint {
    method: Method::GET,
    template: "/v2/watchlists",
};

const CREATE_WATCHLIST: Endpoint = Endpoint {
    method: Method::POST,
    template: "/v2/watchlists",
};

const WATCHLIST: Endpoint = Endpoint {
    method: Method::GET,
    template: "/v2/watchlists/{watchlist_id}",
};

const UPDATE_WATCHLIST: Endpoint = Endpoint {
    method: Method::PUT,
    template: "/v2/watchlists/{watchlist_id}",
};

const ADD_TO_WATCHLIST: Endpoint = Endpoint {
    method: Method::POST,
    template: "/v2/watchlists/{watchlist_id}",
};

const REMOVE_FROM_WATCHLIST: Endpoint = Endpoint {
    method: Method::DELETE,
    template: "/v2/watchlists/{watchlist_id}/{symbol}",
};

const DELETE_WATCHLIST: Endpoint = Endpoint {
    method: Method::DELETE,
    template: "/v2/watchlists/{watchlist_id}",
};

impl Client {
    /// Lists the account's watchlists, without their assets (`GET
    /// /v2/watchlists`).
    pub async fn watchlists(&self) -> Result<Response<Vec<Watchlist>>, Error> {
        self.send(&WATCHLISTS, &["v2", "watchlists"]).await
    }

    /// Creates a watchlist (`POST /v2/watchlists`). It is made once and
    /// never retried.
    pub async fn create_watchlist(
        &self,
        create: &CreateWatchlist,
    ) -> Result<Response<Watchlist>, Error> {
        let request = Request::new(&CREATE_WATCHLIST, &["v2", "watchlists"]).json(create);
        self.send_request(request).await
    }

    /// Reads a watchlist with its assets (`GET
    /// /v2/watchlists/{watchlist_id}`).
    pub async fn watchlist(&self, id: &Id) -> Result<Response<Watchlist>, Error> {
        self.send(&WATCHLIST, &["v2", "watchlists", id.as_str()])
            .await
    }

    /// Renames a watchlist, replaces its symbols, or both (`PUT
    /// /v2/watchlists/{watchlist_id}`). It is made once and never retried.
    pub async fn update_watchlist(
        &self,
        id: &Id,
        update: &WatchlistUpdate,
    ) -> Result<Response<Watchlist>, Error> {
        let segments = ["v2", "watchlists", id.as_str()];
        let request = Request::new(&UPDATE_WATCHLIST, &segments).json(update);
        self.send_request(request).await
    }

    /// Adds an asset to the end of a watchlist (`POST
    /// /v2/watchlists/{watchlist_id}`). It is made once and never retried.
    pub async fn add_to_watchlist(
        &self,
        id: &Id,
        symbol: &str,
    ) -> Result<Response<Watchlist>, Error> {
        let segments = ["v2", "watchlists", id.as_str()];
        let request = Request::new(&ADD_TO_WATCHLIST, &segments).json(&WireAdd { symbol });
        self.send_request(request).await
    }

    /// Removes an asset from a watchlist (`DELETE
    /// /v2/watchlists/{watchlist_id}/{symbol}`). It is made once and never
    /// retried.
    pub async fn remove_from_watchlist(
        &self,
        id: &Id,
        symbol: &Symbol,
    ) -> Result<Response<Watchlist>, Error> {
        let segments = ["v2", "watchlists", id.as_str(), symbol.as_str()];
        self.send(&REMOVE_FROM_WATCHLIST, &segments).await
    }

    /// Deletes a watchlist (`DELETE /v2/watchlists/{watchlist_id}`). It is
    /// made once and never retried.
    pub async fn delete_watchlist(&self, id: &Id) -> Result<Response<()>, Error> {
        let segments = ["v2", "watchlists", id.as_str()];
        let request = Request::new(&DELETE_WATCHLIST, &segments).no_content();
        self.send_request(request).await
    }
}

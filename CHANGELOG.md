# Changelog

All notable changes to suri are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## 0.1.0 - 2026-10-19

### Added

- The value types, in every build: `Decimal`, an exact decimal kept as
  text; `Timestamp` and `Date`, read from RFC 3339 text to the nanosecond
  (with `stream`, `Timestamp` also reads MessagePack's timestamp
  extension); `Inbound`, which keeps enum values suri does not know yet;
  `Symbol` and `Id`, checked values for URL paths (an `Id` converts into a
  `Symbol`); and their parse errors.
- The `rest` feature, on by default: `Client`, built from a `Config`
  (`Credentials`, `Region`, `Environment` and the time and size limits)
  that is checked when the client is built, with `ConfigError` naming the
  first problem. Requests ask for uncompressed responses
  (`Accept-Encoding: identity`), and a `POST` without a body sends
  `Content-Length: 0`.
- `Response` and `ResponseMeta`, with the request ID and the rate-limit
  headers (`RateLimit`).
- `Error`, with its `ErrorKind`, its `Stage`, whether the request may have
  reached Alpaca, Alpaca's code and message (credentials redacted, the
  message bounded to one line of 1,024 bytes), and, on every error of an
  order placement or replacement, the client order ID.
- Reads are retried on throttling, server errors and failed connections,
  with backoff and jitter, never before a rate-limit reset, within
  `read_deadline` and `max_read_attempts`. Requests that change state are
  made once and never retried.
- A `tracing` span, `suri.request`, for every call, with `retry` and
  `error` events; suri installs no subscriber.
- Documented enums, in `suri::trading` and `suri::data`, reject other
  texts with `ParseEnumError`, and `Sort` orders a paged query; both are at
  the crate root, with `rest`.
- The Trading API, in `suri::trading`:
  - `Client::clock` and `Client::calendar`, with `Clock`, `CalendarDay`
    and `ClockTime` for the day's local market times.
  - `Client::account`, with `Account` and `AccountStatus`;
    `account_configurations` and `update_account_configurations`, which
    sends only the fields set; `account_activities`, fills and other
    activities, one page per call; and `portfolio_history`, its values
    read as the nearest `f64`.
  - `Client::assets` and `asset`, and `option_contracts`, one page per call
    with strike filters sent as exact decimal text, and `option_contract`.
    A symbol in a path is percent-encoded, so `BTC/USD` goes as
    `BTC%2FUSD`.
  - `Client::place_order`, with `OrderRequest` and `OrderShape` for every
    order class; `OrderRequest::new` generates a `ClientOrderId` the caller
    can read before sending, and `order_by_client_order_id` finds out
    whether an order whose placement failed was placed. `orders`, one
    page per call, with `OrdersQuery` and `OrderWindow`, and `order`, one
    order by ID;
    `replace_order`, with a generated client order ID for the replacing
    order; `cancel_order`, which returns `Response<()>`, and
    `cancel_all_orders`, with one `CancelResult` per order.
  - `Client::positions` and `position`, with `Position` and `PositionSide`
    (a position's market-data fields are optional, so an allocated IPO not
    yet trading is read too); `close_position`, with `ClosePosition`, and
    `close_all_positions`, with one `ClosedPosition` per position; and
    `exercise_option` and `decline_exercise`.
  - `Client::watchlists`, `watchlist`, `create_watchlist`,
    `update_watchlist`, `add_to_watchlist`, `remove_from_watchlist` and
    `delete_watchlist`, with `Watchlist`, `CreateWatchlist` and
    `WatchlistUpdate`.
- Market Data, in `suri::data`, its history one page per call, its doubles
  read as the nearest `f64` and its integers at their documented width,
  with the shared `Series`, `Latest`, `Timeframe` and `TickType`:
  - Stocks, in `suri::data::stocks`: `Client::stock_bars`, `stock_trades`,
    `stock_quotes`, `stock_auctions`, `stock_latest_bars`,
    `stock_latest_trades`, `stock_latest_quotes`, `stock_snapshots`,
    `stock_conditions` and `stock_exchanges`, with their own `BarsQuery`
    and `HistoryQuery`, the shared `Bar`, and `LatestQuery`,
    `ConditionsQuery`, `Snapshot`, `Snapshots`, `Feed` and `Adjustment`.
  - Crypto, in `suri::data::crypto`, at a `Location`:
    `Client::crypto_bars`, `crypto_trades`, `crypto_quotes`,
    `crypto_latest_bars`, `crypto_latest_trades`, `crypto_latest_quotes`,
    `crypto_latest_orderbooks` and `crypto_snapshots`, with their own
    `Bar`, the shared `BarsQuery` and `HistoryQuery`, and `SymbolsQuery`,
    `Orderbook` and `TakerSide`.
  - Options, in `suri::data::options`: `Client::option_bars`,
    `option_trades`, `option_latest_trades`, `option_latest_quotes`,
    `option_snapshots`, `option_chain`, with strike filters sent as exact
    decimal text, `option_conditions` and `option_exchanges`, with the
    shared `Bar`, `BarsQuery` and `HistoryQuery`, and `Greeks`,
    `Snapshots` and the chain's `ChainQuery`.
  - News, in `suri::data::news`: `Client::news`, with `Query`, `Page`,
    `Article`, `Image` and `ImageSize`.
- The `stream` feature (off by default, requires `rest`): WebSocket
  streams whose value owns its connection and reads it only while the
  caller polls (`next`, `close`), with a liveness Ping and a `tracing`
  span, `suri.stream`, per stream; every end other than `close` is a
  `StreamError`, with its `StreamErrorKind`, that says later messages may
  be lost, and `close` returns the messages that arrive while closing.
  - `Client::trade_updates`: `TradeUpdates`, whose messages are typed as
    `TradeStreamMessage` with `TradeUpdate`, `TradeEvent` and `LegFill`
    (orders carry `Order::cancel_requested_at`); a message suri cannot
    read is delivered as sent in a `StreamMessage`.
  - The Market Data streams: `Client::stock_stream`, by `StreamFeed`,
    `crypto_stream`, `option_stream`, over MessagePack, and `news_stream`,
    each a `DataStream` that can `subscribe` and `unsubscribe` `Channels`
    and delivers each frame's messages (`stocks::Message`,
    `crypto::Message`, `options::Message`, `news::Message`, each a
    `DataMessage`, a trait only suri implements), with the stock stream's
    `Correction`, `Cancel`, `CancelAction`, `Luld`, `Status` and
    `Imbalance`; `StreamError::code` carries Alpaca's code when a Market
    Data stream's opening fails.
- Dependencies: `serde` in every build; with `rest`, `reqwest` (rustls),
  `url`, `secrecy`, `serde_json`, `tokio` and `tracing`; with `stream`,
  `tokio-tungstenite`, `futures-util`, `rustls`, `rustls-platform-verifier`,
  `rmp-serde` and `rmp`.

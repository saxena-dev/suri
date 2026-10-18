# Changelog

All notable changes to suri are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- `Decimal`, an exact decimal value kept as text.
- `Timestamp` and `Date`, read from RFC 3339 text to the nanosecond.
- `Inbound`, which keeps enum values suri does not know yet.
- `Symbol` and `Id`, checked values for URL paths.
- The `rest` feature, on by default, and `Client` with `Client::clock`, which
  reads the market clock.
- `Config`, `Credentials`, `Region` and `Environment`, checked when the client
  is built, with `ConfigError` naming the first problem.
- `Response` and `ResponseMeta`, with the request ID and rate-limit headers.
- `Error`, with its kind, its stage and whether the request may have reached
  Alpaca.
- `Error::code` and `Error::message`: Alpaca's error code and message, with
  credentials redacted and the message bounded to one line of 1,024 bytes.
- Reads are retried on throttling, server errors and failed connections,
  with backoff and jitter, never before a rate-limit reset, within
  `read_deadline` and `max_read_attempts`.
- A `tracing` span, `suri.request`, for every call, with `retry` and `error`
  events; suri installs no subscriber.
- Requests ask for uncompressed responses (`Accept-Encoding: identity`).
- `Client::account` and `Account`, with `AccountStatus`; documented enums
  reject other texts with `ParseEnumError`.
- `Client::account_configurations` and `Client::update_account_configurations`,
  which sends only the fields set.
- `Client::account_activities`: fills and other activities, one page per
  call.
- `Client::portfolio_history`, with values read as the nearest `f64`.
- Requests that change state are made once and never retried.
- `Client::assets` and `Client::asset`; a symbol in a path is sent
  percent-encoded, so `BTC/USD` goes as `BTC%2FUSD`.
- `Client::option_contracts` and `Client::option_contract`, one page per
  call, with strike filters sent as exact decimal text.
- `Client::calendar`, with `ClockTime` for the day's local market times.
- An `Id` converts into a `Symbol`.
- `Client::place_order`, made once and never retried, with `OrderRequest`
  and `OrderShape` for every order class; `OrderRequest::new` generates a
  `ClientOrderId` the caller can read before sending.
- `Error::client_order_id`, set on every error of an order placement.
- `Client::order_by_client_order_id`, to find out whether an order whose
  placement failed was placed.
- `Client::orders` and `Client::order`, one page per call, with
  `OrdersQuery` and `OrderWindow`.
- `Client::replace_order`, made once, with a generated client order ID for
  the replacing order that every error carries.
- `Client::cancel_order`, which returns `Response<()>`, and
  `Client::cancel_all_orders`, with one `CancelResult` per order; both are
  made once.
- `Client::positions` and `Client::position`, with `Position` and
  `PositionSide`; a position's market-data fields are optional, so an
  allocated IPO not yet trading is read too.
- `Client::close_position`, with `ClosePosition`, and
  `Client::close_all_positions`, with one `ClosedPosition` per position;
  both are made once.
- `Client::exercise_option` and `Client::decline_exercise`, made once;
  a `POST` without a body sends `Content-Length: 0`.
- `Client::watchlists` and `Client::watchlist`, and
  `Client::create_watchlist`, `Client::update_watchlist`,
  `Client::add_to_watchlist`, `Client::remove_from_watchlist` and
  `Client::delete_watchlist`, each made once, with `Watchlist`,
  `CreateWatchlist` and `WatchlistUpdate`.
- The `stream` feature (off by default, requires `rest`) and
  `Client::trade_updates`: the Trading stream over WebSocket, read only
  while the caller polls, a liveness Ping, every end a `StreamError` that
  says later messages may be lost, and `TradeUpdates::close`, which returns
  the messages that arrive while closing.
- Typed trade updates: `TradeStreamMessage` with `TradeUpdate`,
  `TradeEvent` and `LegFill`; a `trade_updates` message suri cannot read,
  and any other stream's message, delivered as sent in a `StreamMessage`.
- `Order::cancel_requested_at`, sent on the trade updates stream.
- Market Data: `Client::stock_bars`, `stock_trades`, `stock_quotes` and
  `stock_auctions`, one page per call, in `suri::data::stocks`, with the
  shared `Series`, `Bar` and `Timeframe` in `suri::data`.
- `Client::stock_latest_bars`, `stock_latest_trades`, `stock_latest_quotes`
  and `stock_snapshots`, with `Latest`, `LatestQuery`, `Snapshot` and
  `Snapshots`.
- `Client::stock_conditions` and `stock_exchanges`, the stock condition and
  exchange code tables, with `TickType` and `ConditionsQuery`.
- Crypto market data in `suri::data::crypto`: `Client::crypto_bars`,
  `crypto_trades`, `crypto_quotes`, `crypto_latest_bars`,
  `crypto_latest_trades`, `crypto_latest_quotes`,
  `crypto_latest_orderbooks` and `crypto_snapshots`, at a `Location`, with
  the shared `BarsQuery`, `HistoryQuery` and `SymbolsQuery` in
  `suri::data`.
- Options market data in `suri::data::options`: `Client::option_bars`,
  `option_trades`, `option_latest_trades`, `option_latest_quotes`,
  `option_snapshots`, `option_chain`, `option_conditions` and
  `option_exchanges`, with `Greeks`, `Snapshots` and the chain's
  `ChainQuery`.
- News in `suri::data::news`: `Client::news`, one page of articles per
  call, with `Query`, `Page`, `Article`, `Image` and `ImageSize`.
- With `stream`, the Market Data streams: `Client::stock_stream`,
  `crypto_stream` and `news_stream` return a `DataStream` that subscribes
  and unsubscribes `Channels` and delivers each frame's messages
  (`stocks::Message`, `crypto::Message`, `news::Message`), with the stock
  stream's `StreamFeed`, `Correction`, `Cancel`, `CancelAction`, `Luld`,
  `Status` and `Imbalance`, and `StreamError::code`.
- With `stream`, the dependencies `tokio-tungstenite`, `futures-util`,
  `rustls` and `rustls-platform-verifier`.

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

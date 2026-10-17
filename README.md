# suri

A Rust client for the [Alpaca](https://alpaca.markets) Trading and Market
Data APIs. Support for the Broker API is planned after the first release.

**Status: pre-release.** The client reads the market clock and nothing else
yet; the crate is being built in the open, one tested piece at a time.

## Scope of 0.1.0

The first release covers the Trading API's REST endpoints (account, orders,
positions, assets, option contracts, the clock and calendar, watchlists, and
account activities and history), the `trade_updates` stream, and the Market
Data API's REST endpoints and streams for stocks, crypto, options and news, on
Alpaca's US hosts.

## Building

suri needs Rust 1.88 or later. With its default `rest` feature, building it
also needs a C compiler, for the TLS library's cryptography, and on 32-bit
x86 Windows the NASM assembler as well.

## What suri promises

- A failed call is never reported as success.
- An order is never sent twice behind your back; when its outcome is unknown,
  you are told so.
- A stream message is never dropped without an error.
- Your credentials never appear in suri's logs, spans or errors.

The [contract](docs/contract.md) states every behaviour suri promises and
changes together with the code.

## Licence

MIT. See [LICENSE](LICENSE).

<div align="center">

<img src=".github/assets/banner.png" width="100%" alt="suri, a Rust client for the Alpaca Trading and Market Data APIs">

<br>
<br>

[![CI](https://github.com/saxena-dev/suri/actions/workflows/ci.yml/badge.svg)](https://github.com/saxena-dev/suri/actions/workflows/ci.yml)
[![Coverage](https://codecov.io/gh/saxena-dev/suri/graph/badge.svg)](https://codecov.io/gh/saxena-dev/suri)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org/)
[![Docs.rs](https://docs.rs/suri/badge.svg)](https://docs.rs/suri)
[![Crates.io](https://img.shields.io/crates/v/suri.svg)](https://crates.io/crates/suri)
[![Downloads](https://img.shields.io/crates/d/suri.svg)](https://crates.io/crates/suri)

**[Contract](https://github.com/saxena-dev/suri/blob/v0.1.0/docs/contract.md)** &nbsp;·&nbsp; **[Migration guide](https://github.com/saxena-dev/suri/blob/v0.1.0/docs/migration.md)** &nbsp;·&nbsp; **[Changelog](CHANGELOG.md)** &nbsp;·&nbsp; **[Alpaca docs](https://docs.alpaca.markets)**

</div>

<br>

**suri** is an asynchronous Rust SDK for [Alpaca](https://alpaca.markets)'s
Trading and Market Data APIs: the REST API for your account, orders,
positions, assets and watchlists, the market data for stocks, crypto,
options and news, and the WebSocket streams for trade updates and live
market data.

When software talks to a broker, the expensive bugs are rarely crashes.
They are the quiet ones: an error read as an empty list, an order placed
twice because a timeout was retried, a stream message that vanished, an
API secret printed into a log. suri is built so that these cannot happen
quietly. When something goes wrong, you are told, with enough detail to
decide what to do next.

**Status: 0.1.0, the first release.** The
[contract](https://github.com/saxena-dev/suri/blob/v0.1.0/docs/contract.md)
states every behaviour suri promises, and the
[migration guide](https://github.com/saxena-dev/suri/blob/v0.1.0/docs/migration.md)
lists every break between versions.

## Why suri

<table>
<tr>
<td width="50%" valign="top">

<sub>01</sub><br>
**Failures are never disguised as success.**<br>
A call succeeds only when Alpaca answers with a success status and, for
a call that returns data, a body of the documented shape. Anything else
is an `Error` that says what went wrong and how far the request got.

</td>
<td width="50%" valign="top">

<sub>02</sub><br>
**Orders are never sent twice behind your back.**<br>
Every request that changes your account is sent at most once: no status,
failed connection or timeout makes suri send it again. When a response
is lost, the error says the order *may* have reached Alpaca, so you can
look before you send again.

</td>
</tr>
<tr>
<td width="50%" valign="top">

<sub>03</sub><br>
**Every order you place can be looked up.**<br>
`OrderRequest::new` gives each order a client order ID before it is
sent, every error of a placement carries it, and
`Client::order_by_client_order_id` looks the order up by it.

</td>
<td width="50%" valign="top">

<sub>04</sub><br>
**Reads retry on their own, within limits you can see.**<br>
A read that meets a 429, a 500, 502, 503 or 504, or a failed connection
is tried again with jittered backoff, within its attempt limit and never
past its deadline. After a 429 that carries a reset time, it never
retries before that time by your machine's clock.

</td>
</tr>
<tr>
<td width="50%" valign="top">

<sub>05</sub><br>
**A stream message is never dropped without an error.**<br>
Every message a stream receives reaches you or is reported as an
error, the messages that arrive while you close it are returned by
`close`, and a stream that ends without your asking ends with an error
saying later messages may be lost.

</td>
<td width="50%" valign="top">

<sub>06</sub><br>
**Money sent as text stays exact.**<br>
Prices, quantities and amounts that Alpaca sends as decimal strings
arrive as `Decimal`, which keeps every digit as sent and never passes
through a binary float. Values sent as JSON numbers, such as market-data
prices and portfolio history, arrive as `f64`, and documented integers
as integers.

</td>
</tr>
<tr>
<td width="50%" valign="top">

<sub>07</sub><br>
**Unknown values are kept, not guessed.**<br>
When Alpaca adds a new order status or event, you receive it as
`Inbound::Unknown` with the exact text, instead of a failed call or a
wrong guess.

</td>
<td width="50%" valign="top">

<sub>08</sub><br>
**Your credentials stay yours.**<br>
The key ID and secret never appear in suri's logs, spans or errors. One
library suri uses can log them at its most detailed levels; the contract
names it and the filter that removes them.

</td>
</tr>
<tr>
<td width="50%" valign="top">

<sub>09</sub><br>
**You own your telemetry.**<br>
suri reports through `tracing` and installs nothing: no subscriber, no
logger, no global state.

</td>
<td width="50%" valign="top">

<sub>10</sub><br>
**Take only what you need.**<br>
The REST client and the streams are separate features. With no features
at all, you get the value types alone, with no network stack.

</td>
</tr>
</table>

## Installation

```toml
[dependencies]
suri = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The default feature, `rest`, gives you the REST client. Add `stream` for
the WebSocket streams:

```toml
suri = { version = "0.1", features = ["stream"] }
```

suri needs **Rust 1.88** or newer. With `rest`, building it also needs a C
compiler, for the TLS library's cryptography, and on 32-bit x86 Windows
the NASM assembler as well.

## Getting started

### Before you start

You need an Alpaca account and an API key pair: a key ID and a secret.

> [!TIP]
> Start on paper: with `Environment::Paper`, every Trading request goes to
> Alpaca's paper-trading host. Market data comes from the same host for
> paper and live accounts.

### 1. Build a client and read the market clock

suri reads no `APCA_API_*` variable; your program reads the key pair and
passes it in. The configuration is checked when the client is built, so a
bad value fails there, never halfway through a request.

```rust,no_run
use suri::{Client, Config, Credentials, Environment, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key_id = std::env::var("APCA_API_KEY_ID")?;
    let secret = std::env::var("APCA_API_SECRET_KEY")?;
    let credentials = Credentials::api_key(key_id, secret);
    let client = Client::new(Config::new(Region::US, Environment::Paper, credentials))?;
    let clock = client.clock().await?;
    println!("market open: {}", clock.data().is_open);
    Ok(())
}
```

Every REST call returns a `Response`: its `data()` is the endpoint's
value itself, and its `meta()` carries the HTTP status, and Alpaca's
request ID and rate-limit headers when it sent them.

### 2. Read your account

```rust,no_run
use suri::{Client, Config, Credentials, Environment, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let credentials = Credentials::api_key(
        std::env::var("APCA_API_KEY_ID")?,
        std::env::var("APCA_API_SECRET_KEY")?,
    );
    let client = Client::new(Config::new(Region::US, Environment::Paper, credentials))?;

    let response = client.account().await?;
    let account = response.data();
    if let Some(buying_power) = &account.buying_power {
        println!("buying power: {buying_power}");
    }
    Ok(())
}
```

Amounts like `buying_power` are `Decimal`s: they print exactly as Alpaca
sent them. A field the documentation does not promise is an `Option`.

### 3. Place an order, carefully

An order request holds its client order ID from the moment you build it.
Store it before you send, and you can look the order up by it later.

```rust,no_run
use suri::trading::{OrderKind, OrderRequest, OrderShape, OrderSide, Size, TimeInForce};
use suri::{Client, Config, Credentials, Environment, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let credentials = Credentials::api_key(
        std::env::var("APCA_API_KEY_ID")?,
        std::env::var("APCA_API_SECRET_KEY")?,
    );
    let client = Client::new(Config::new(Region::US, Environment::Paper, credentials))?;

    let shape = OrderShape::Simple {
        symbol: "AAPL".to_owned(),
        side: OrderSide::Buy,
        size: Size::Qty("1".parse()?),
        kind: OrderKind::Market,
    };
    let request = OrderRequest::new(shape, TimeInForce::Day);
    println!("placing {}", request.client_order_id);
    match client.place_order(&request).await {
        Ok(response) => println!("order {} placed", response.data().id),
        Err(error) if error.may_have_reached_broker() => {
            println!("placing failed: {error}");
            // Do not send it again: find out whether it was placed.
            match client
                .order_by_client_order_id(&request.client_order_id)
                .await
            {
                Ok(found) => println!("order {} was placed", found.data().id),
                Err(lookup) if lookup.status() == Some(404) => {
                    println!("Alpaca had no order {}", request.client_order_id);
                }
                // Still unknown: the lookup failed too.
                Err(lookup) => return Err(lookup.into()),
            }
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}
```

> [!IMPORTANT]
> `place_order` sends an order once and never retries it, and every error
> it returns carries the order's client order ID. After one that
> `may_have_reached_broker`, look the order up by that ID before doing
> anything else. A placed order is not a filled order: read its progress
> with `Client::order`, or from the `trade_updates` stream.

Every order class has its own `OrderShape`, so a bracket order without its
stop-loss does not compile.

### 4. Read market data, a page at a time

Each call reads one page. A page may hold fewer bars than the limit even
when more exist: only a missing `next_page_token` means the end.

```rust,no_run
use suri::data::Timeframe;
use suri::data::stocks::BarsQuery;
use suri::{Client, Config, Credentials, Environment, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let credentials = Credentials::api_key(
        std::env::var("APCA_API_KEY_ID")?,
        std::env::var("APCA_API_SECRET_KEY")?,
    );
    let client = Client::new(Config::new(Region::US, Environment::Paper, credentials))?;

    let mut query = BarsQuery::new(vec!["AAPL".to_owned()], Timeframe::Day);
    query.start = Some("2024-01-02T00:00:00Z".parse()?);
    query.limit = Some(100);
    loop {
        let (page, _) = client.stock_bars(&query).await?.into_parts();
        for bar in page.items.get("AAPL").into_iter().flatten() {
            println!("{}: close {}", bar.timestamp, bar.close);
        }
        match page.next_page_token {
            Some(token) => query.page_token = Some(token),
            None => break,
        }
    }
    Ok(())
}
```

Crypto, options and news work the same way, each in its own module:
`suri::data::crypto`, `suri::data::options` and `suri::data::news`.

### 5. Stream trade updates and live market data

With the `stream` feature, each stream is a value that owns its WebSocket
connection and reads it only while you call `next` or `close`: there is no
background task and no queue, so nothing piles up inside suri or is
dropped behind your back.

```rust,no_run
use suri::data::Channels;
use suri::data::stocks::{Message, StreamFeed};
use suri::trading::TradeStreamMessage;
use suri::{Client, Config, Credentials, Environment, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let credentials = Credentials::api_key(
        std::env::var("APCA_API_KEY_ID")?,
        std::env::var("APCA_API_SECRET_KEY")?,
    );
    let client = Client::new(Config::new(Region::US, Environment::Paper, credentials))?;

    let mut updates = client.trade_updates().await?;
    // Waits until three messages arrive: place an order meanwhile.
    for _ in 0..3 {
        match updates.next().await? {
            TradeStreamMessage::Update(update) => {
                println!("{:?}: order {}", update.event, update.order.id);
            }
            other => println!("{other:?}"),
        }
    }
    let (more, result) = updates.close().await;
    // Handle what arrived while closing before looking at the result.
    for message in more {
        println!("{message:?}");
    }
    result?;

    let mut stocks = client.stock_stream(StreamFeed::Iex).await?;
    let mut channels = Channels::new();
    channels.trades = vec!["AAPL".to_owned()];
    stocks.subscribe(&channels).await?;
    for message in stocks.next().await? {
        match message {
            Message::Trade { symbol, trade } => {
                println!("{symbol}: {} at {}", trade.size, trade.price);
            }
            Message::Subscription(current) => println!("trades: {:?}", current.trades),
            other => println!("{other:?}"),
        }
    }
    let (more, result) = stocks.close().await;
    for message in more {
        println!("{message:?}");
    }
    result?;
    Ok(())
}
```

These rules hold for every stream:

- **Keep calling `next`.** While you wait in `next`, suri sends a Ping
  after `request_timeout` of silence, and ends the stream with `Timeout`
  if nothing answers. A program that stops calling `next` also stops
  answering Alpaca's Pings.
- **`next` is cancel-safe; the others are not.** Dropping `next`'s
  future, in a `select!` or a timeout, loses no message. A dropped
  `subscribe` or `unsubscribe` may or may not have sent its message, and
  a dropped `close` drops the connection and the messages it read.
- **Every end is loud.** A stream that ends without your asking returns
  one error saying later messages may be lost, and every later call
  returns `Ended`. There is no reconnection: open a new stream.
- **Unreadable messages are kept.** A message of a known type that suri
  cannot read is delivered as sent (`Undecoded`), one of a type it does
  not know as `Other`, and the stream goes on. A frame it cannot read at
  all, or a message larger than `max_body_bytes`, ends the stream with an
  error.

## Handling errors

Every failed call returns an `Error`. Its **kind** tells you what
happened, and its **stage** tells you how far the request got:

```rust,no_run
use suri::{Client, ErrorKind};

async fn show_account(client: &Client) {
    match client.account().await {
        Ok(response) => println!("account {}", response.data().id),
        Err(error) if error.kind() == ErrorKind::Api => {
            // Match on the status and Alpaca's code: its message text may change.
            println!("Alpaca answered {:?}, code {:?}", error.status(), error.code());
        }
        Err(error) => {
            println!("{error} (may have reached Alpaca: {})", error.may_have_reached_broker());
        }
    }
}
```

| `ErrorKind` | What it means |
| :-- | :-- |
| `Api` | Alpaca answered with an error status; `status()` gives it, and `code()` and `message()` Alpaca's code and message when its body carried them |
| `Redirect` | Alpaca answered with a redirect, which suri never follows |
| `Transport` | the connection failed |
| `Timeout` | `connect_timeout`, `request_timeout` or the read's deadline expired |
| `BodyTooLarge` | the body was larger than `max_body_bytes` |
| `Decode` | the body did not have the documented shape |

The stage is `NotStarted` only when no byte of the request left your
machine. Anything later means Alpaca may have acted on it, and
`may_have_reached_broker()` says so in one call. Alpaca's message is kept
with your key ID and secret replaced by `[REDACTED]`, on one line, and at
most 1,024 bytes long. The contract's Errors section gives the exact
limits of that scrubbing.

## Limits, retries and time

Every limit has a documented default, so `Config::new` alone is enough.
`Client::new` checks each limit you change.

| Setting | Default | What it bounds |
| :-- | :-: | :-- |
| `connect_timeout` | 10 s | connecting, TLS included; for a stream, the upgrade too |
| `request_timeout` | 30 s | one request, from connecting to the whole body; for a stream, its opening, a subscription, a liveness wait and its close |
| `read_deadline` | 60 s | a read's attempts and waits together |
| `max_read_attempts` | 4 | a read's attempts, the first included |
| `retry_base` | 250 ms | the backoff after the first attempt, doubled after each later one |
| `retry_cap` | 8 s | the longest backoff |
| `max_body_bytes` | 32 MiB | the largest body, or stream message, read |

```rust,no_run
use std::time::Duration;
use suri::{Client, Config, ConfigError, Credentials, Environment, Region};

fn client(credentials: Credentials) -> Result<Client, ConfigError> {
    let config = Config::new(Region::US, Environment::Paper, credentials)
        .with_request_timeout(Duration::from_secs(10))
        .with_max_read_attempts(2);
    Client::new(config)
}
```

suri retries reads, and only reads: after a 429, a 500, 502, 503 or
504, or a connection that fails or times out, up to `max_read_attempts`
attempts within `read_deadline`. After a 429 that carries Alpaca's
rate-limit reset time, it waits at least until then, by your machine's
clock. Up to half of each backoff is added at random, so that many
clients do not retry in step.

## Observability

suri reports through the `tracing` crate and installs nothing. With no
subscriber, nothing is recorded. Each call is one `suri.request` span with
its method, endpoint template, attempt and status, and `retry` and `error`
events; each stream is one `suri.stream` span with events such as
`connected`, `ping` and `ended`. None of them records a URL, a body,
Alpaca's message or a credential.

> [!WARNING]
> The WebSocket library, `tungstenite`, logs requests and frames at TRACE
> and DEBUG level, the key ID and secret included. Filter it at INFO or
> above, for example `tungstenite=info`, and none of those records remains.

## What suri does not do

- It does not retry a request that changes your account, ever.
- It does not loop through pages for you: each call reads one page, and
  you decide how many to read.
- It does not reconnect a stream. A stream has no resume key, so a silent
  reconnect would hide a gap; you open a new one.
- It does not limit your request rate on its own. A read slows down only
  when it retries: after a 429, a 500, 502, 503 or 504, or a failed
  connection.
- It does not read environment variables, except that the TLS
  certificate verifier may read `SSL_CERT_FILE` and `SSL_CERT_DIR` on
  some systems. Where your key pair comes from is up to you.
- It writes no files.
- It does not cover the Broker API yet, regions other than the US, or the
  single-symbol stock endpoints (pass one symbol to the multi-symbol
  call; the data is the same).
- It does not compress stream traffic: every frame arrives as sent.

## How suri is tested

- **Upstream responses.** Response fixtures from the test suite of
  alpaca-py, Alpaca's Python SDK, pinned to a commit, are served byte for
  byte as extracted, and the decoded values are checked against values
  written out by hand.
- **Nothing leaves your machine.** Every test that makes a request runs
  against loopback HTTP and WebSocket servers. No test needs an Alpaca
  account or the network.
- **Hostile inputs.** Tests feed truncated, oversized and wrongly shaped
  bodies and frames, and MessagePack frames nested past 128 levels, and
  check that each one is reported, never misread or silently lost.
- **Every configuration.** CI builds and tests every feature row (none,
  `rest`, and every feature) on stable, checks them all on Rust 1.88, runs
  the tests with serde_json's `arbitrary_precision` on, and compiles the
  examples in this README.
- **No test can hang.** The stream tests run under a time limit, and in
  CI every test binary runs under a watchdog, so a regression fails
  instead of stalling.

## Supported Alpaca APIs

All on Alpaca's US paper and live hosts:

| | REST | Streams <sub>`stream`</sub> |
| :-- | :-- | :-- |
| **Trading** | account · configuration · activities · portfolio history · orders · positions · assets · option contracts · clock · calendar · watchlists | `trade_updates` |
| **Stocks** | bars · trades · quotes · auctions · latest values · snapshots · code tables | ✓ |
| **Crypto** | bars · trades · quotes · latest values and orderbooks · snapshots | ✓ |
| **Options** | bars · trades · latest values · snapshots · chain · code tables | ✓ <sub>MessagePack</sub> |
| **News** | articles | ✓ |
| **Broker&nbsp;API** | planned after 0.1.0 | |

## Feature flags

| Feature | Default | Adds |
| :-- | :-: | :-- |
| `rest` | ✓ | `Client` and the REST endpoints suri covers, Trading and Market Data. |
| `stream` | | The `trade_updates` stream and the Market Data streams over WebSocket. Requires `rest`. |

With no features, suri builds its value types only (`suri::Decimal`,
`Timestamp`, `Date`, `Inbound`, `Symbol`, `Id`).

## Documentation

- [API reference on docs.rs](https://docs.rs/suri)
- [The contract](https://github.com/saxena-dev/suri/blob/v0.1.0/docs/contract.md):
  every behaviour suri promises, in detail.
- [The migration guide](https://github.com/saxena-dev/suri/blob/v0.1.0/docs/migration.md):
  every break between versions and what to do.
- [The changelog](CHANGELOG.md).

## Status and disclaimer

suri is pre-1.0. Breaking changes ship only in minor versions, each listed
in the migration guide with what to do about it.

suri is an independent open-source project. It is not affiliated with
Alpaca in any way, and Alpaca neither makes, endorses nor supports it. For
questions and bug reports, please
[open an issue](https://github.com/saxena-dev/suri/issues).

The software is provided "as is", without warranty of any kind. The author
and contributors take no responsibility for any financial losses, damages
or other issues arising from its use. Test against a paper account before
trading real money.

## Licence

MIT. See [LICENSE](LICENSE).

<br>

<div align="center">
  <img src=".github/assets/icon.png" width="56" alt="">
  <br>
  <sub>Named after the Suri, the rarer of the two alpaca breeds.</sub>
</div>

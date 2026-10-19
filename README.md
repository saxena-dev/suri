# suri

A Rust client for the [Alpaca](https://alpaca.markets) Trading and Market
Data APIs. Support for the Broker API is planned after the first release.

**Status: 0.1.0, the first release.** It covers the Trading API's REST
endpoints in the scope below, each change sent at most once and never
retried, and its `trade_updates` stream behind the `stream` feature.
Market Data covers stocks (bars, trades, quotes, auctions, latest values,
snapshots and code tables), crypto, options and news over REST, and the
stock, crypto, options and news streams behind `stream`. The
[contract](https://github.com/saxena-dev/suri/blob/v0.1.0/docs/contract.md) states every behaviour suri
promises, and the [migration guide](https://github.com/saxena-dev/suri/blob/v0.1.0/docs/migration.md) lists
every break between versions.

## Quickstart

Each example is a whole program. They use `tokio` with its `macros` and
`rt-multi-thread` features, which your program adds:

```toml
[dependencies]
suri = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Build a client and read the market clock. suri reads no `APCA_API_*`
variable; the program reads the key pair and passes it in.

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

Read the account:

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

Place an order. Its client order ID exists before it is sent, so it can be
stored first. An order is sent once and never retried: when an error says
the order may have reached Alpaca, look it up before sending it again.

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
                Err(lookup) => println!("lookup failed: {lookup}"),
            }
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}
```

Read daily bars, one page per call. A page may hold fewer bars than the
limit even when more exist: only a missing `next_page_token` means the end.

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

With the `stream` feature (`suri = { version = "0.1", features =
["stream"] }`), read trade updates and a market-data stream.
Each stream value owns its connection and reads it only while `next` or
`close` runs; every end other than `close` is an error.

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
    result?;
    println!("{} more while closing", more.len());

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
    stocks.close().await.1?;
    Ok(())
}
```

## Features

| Feature | Default | Adds |
|---|---|---|
| `rest` | on | The HTTP client and the Trading and Market Data REST endpoints |
| `stream` | off | The `trade_updates` stream and the Market Data streams for stocks, crypto, options (MessagePack) and news over WebSocket; requires `rest` |

With no features, suri builds its value types only (`suri::Decimal`,
`Timestamp`, `Date`, `Inbound`, `Symbol`, `Id`).

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
- Your credentials never appear in suri's logs, spans or errors. Where a
  library suri depends on can log them at its most detailed levels, the
  contract names that library and the filter that removes them.

The [contract](docs/contract.md) states every behaviour suri promises and
changes together with the code. The [migration guide](docs/migration.md)
lists every break between versions and what to do.

## Licence

MIT. See [LICENSE](LICENSE).

# The suri contract

This document states every behaviour suri promises to its callers. It changes
together with the code: a change in behaviour is a change to this document,
and a behaviour that is not written here is not promised.

## Promises

1. **A failed call is never reported as success.** If Alpaca rejects a
   request, or suri cannot tell whether it succeeded, the call returns an
   error.
2. **An order is never sent twice behind your back.** suri never retries an
   order or any other request that changes state. When it cannot tell whether
   such a request reached Alpaca, the error says so, and gives you what you
   need to find out.
3. **A stream message is never dropped without an error.** Every message
   suri receives reaches you or is reported as an error, messages that
   arrive while you close a stream are returned by `close`, and a stream
   that ends without your asking ends with an error saying later messages
   may be lost.
4. **Your credentials never appear in suri's logs, spans or errors.** Where a
   library suri depends on can log them at its most detailed levels, this
   document will name that library and the filter that removes them.

## How suri chooses

When the documentation leaves a choice open, suri chooses correctness first,
then performance, then simplicity, and leans towards the simpler option when
the last two pull against each other.

## Decimal values

Prices, quantities and amounts that Alpaca sends as decimal strings arrive as
`Decimal`, which keeps the text exactly as sent: every digit, however many,
and no conversion through a binary float on the way in or out. A `Decimal` is
written back as a JSON string of the same text.

- **Accepted:** a JSON string of an optional `-`, one or more digits, and
  optionally a `.` followed by one or more digits (`"0"`, `"-0"`, `"007"`,
  `"1.00"`), and a JSON integer within 64 bits, kept as its digits.
- **Rejected:** any other string (`"1e5"`, `".5"`, `"+1"`, `""`), a number
  with a fraction or an exponent, the number `-0`, an integer beyond 64 bits,
  `true`, `null`, arrays and objects, when the value is decoded directly. A
  nullable field is an `Option<Decimal>`.
- **Equality is equality of the text.** `"5"` and `"5.0"` are different
  values, as they are on the wire; Alpaca's own position examples mix
  `"5"` and `"100.0"`. There is no ordering and no arithmetic: read
  `as_str()` into the decimal type of your choice to compare or compute.

## Time

Instants arrive as `Timestamp`, a UTC instant to the nanosecond between
0000-01-01T00:00:00Z and 9999-12-31T23:59:59.999999999Z; dates without a time
arrive as `Date` (`YYYY-MM-DD`).

- **Read:** RFC 3339 text: `YYYY-MM-DD`, `T` or `t`, `HH:MM:SS`, an optional
  fraction of one to nine digits, then `Z`, `z` or an offset `±HH:MM`. The
  offset is applied and not kept: `13:45:04-04:00` and `17:45:04Z` are the
  same instant.
- **Rejected rather than altered:** ten or more fractional digits (never
  trimmed), a leap second `:60` (never folded into the next minute), any
  other deviation from that form, and instants outside the range.
- **Written:** `YYYY-MM-DDTHH:MM:SS[.fraction]Z` in UTC, with trailing zeros
  removed from the fraction and no fraction when it is zero. Every value
  written reads back as the same instant.
- **To other time types:** `unix_seconds()` (the floor) and `subsec_nanos()`
  build any time crate's value. There is no conversion to
  `std::time::SystemTime`, which cannot hold every instant on every platform.

## Unknown values

Alpaca adds values to its enumerations, such as a new order status or a new
activity type, without changing the API version, and treats that as a
compatible change. Such fields arrive as `Inbound<T>`: a value suri knows is
`Known`, and any other value is `Unknown` with the exact text the server sent.
A new value is never an error and never read as a different known value. An
`Inbound` cannot be sent back to the server: requests take the plain enum.

## Path values

A value that becomes part of a request's URL path, such as a symbol or an
order ID, is a checked type, so a value that would turn one request into
another cannot be built.

- **`Symbol`:** printable ASCII other than space (bytes `!` to `~`), not empty,
  and not `.` or `..`, which would remove or keep a path segment and send the
  request elsewhere. A `/`, as in the crypto pair `BTC/USD`, is allowed.
  Symbols compare as text.
- **`Id`:** a UUID of 8-4-4-4-12 hexadecimal digits in either case, written
  back exactly as given and compared ignoring case, so one ID in two cases is
  one value. An `Id` converts into a `Symbol`, so an ID can be passed
  wherever a symbol or an ID is accepted.

A path value is sent percent-encoded as one segment: `BTC/USD` goes as
`BTC%2FUSD`, the form Alpaca asks for.

## Configuration

A `Client` is built from a `Config`: a region, an environment and an API key
pair, with every limit at a documented default. `Client::new` checks the
whole configuration and reports the first problem as a `ConfigError`, so a
bad value fails when the client is built, never at request time.

- **Hosts:** region `US` with environment `Paper` uses
  `https://paper-api.alpaca.markets` for the Trading API, and `Live` uses
  `https://api.alpaca.markets`; both use `https://data.alpaca.markets` for
  Market Data. Each endpoint carries its own version prefix, such as `/v2`.
  A base-URL override replaces every host, streams included.
- **Credentials:** the key ID and secret must be non-empty printable ASCII
  (bytes `!` to `~`), so a trailing newline read from a file is caught here.
  Neither can be read back from suri, and `Debug` prints `Credentials { .. }`.
- **Limits:**

  | Value | Default | Allowed |
  |---|---|---|
  | `connect_timeout`: connecting, TLS included; for a stream, the WebSocket upgrade as well | 10 s | 1 ms to 60 s, and no longer than `request_timeout` |
  | `request_timeout`: one request, from connecting until the whole body has arrived; for a stream, its opening, a subscription's write, the silence before a liveness Ping and the wait after it, and its close | 30 s | 1 ms to 10 min |
  | `max_body_bytes`: the largest response body, or stream message, read | 32 MiB | 1 KiB to 1 GiB |
  | `read_deadline`: a read's attempts and waits together, from the start of the first | 60 s | No shorter than `request_timeout`, at most 1 h |
  | `retry_base`: the backoff after a read's first attempt, doubled after each later one | 250 ms | 1 ms to 10 s |
  | `retry_cap`: the longest backoff | 8 s | 1 ms to 60 s, and no smaller than `retry_base` |
  | `max_read_attempts`: a read's attempts, the first included | 4 | 1 to 10 |

  Idle connections are closed after 30 s.
- **Base URL override:** `with_base_url` points the client at another server,
  for tests. It must use https unless its host is loopback (`localhost`,
  `127.0.0.0/8` or `::1`), and must have no path, query, fragment or
  credentials. `Debug` of a `Config` prints it as `Some(..)`.
- **Environment:** suri reads no environment variable: not Alpaca's
  `APCA_API_*` variables, and not the proxy variables. The one exception is
  the TLS library's certificate verifier, which on some systems reads
  `SSL_CERT_FILE` and `SSL_CERT_DIR` in place of the system's certificate
  store.

## Requests

- Each request carries the key pair in the `APCA-API-KEY-ID` and
  `APCA-API-SECRET-KEY` headers, marked sensitive, and
  `User-Agent: suri/<version>`.
- suri speaks HTTP/1.1 directly to the host: no proxy, and no compression
  (it sends `Accept-Encoding: identity` and decompresses nothing).
- A redirect is never followed; it is an error (below).
- A value placed in a URL path, such as a symbol, is sent as one encoded
  segment: `BTC/USD` is sent as `BTC%2FUSD`.
- A request with a body sends it as JSON with `Content-Type:
  application/json`. A `POST` without a body, such as exercising an
  option, sends `Content-Length: 0`; a `GET` or `DELETE` without one sends
  neither header.

## Responses

A successful call returns a `Response`: the decoded data and its
`ResponseMeta`, which holds the status, the `X-Request-ID` header when sent
(Alpaca's support asks for it) and the rate-limit headers when all three
(`X-RateLimit-Limit`, `X-RateLimit-Remaining` and `X-RateLimit-Reset`, read
as Unix seconds) were sent as whole numbers. Fields a response has beyond the
documented ones are ignored, so a field Alpaca adds never fails a call.
A call Alpaca documents with no response body, such as a cancellation,
returns `Response<()>`: whatever body arrives is read within
`max_body_bytes` and dropped, never decoded.

## Errors

Every failed call returns an `Error`. Its `kind()` says what went wrong, its
`stage()` how far the request got, and it names the method, the endpoint's
path template (never the URL) and the attempt. When a response arrived, the
error carries its status and `ResponseMeta`.

| Kind | Meaning |
|---|---|
| `Api` | Alpaca answered with an error status |
| `Redirect` | Alpaca answered with a redirect, which was not followed; `location()` gives its scheme, host and port when it was an absolute http or https URL, never its credentials, path or query |
| `Transport` | The connection failed |
| `Timeout` | `connect_timeout`, `request_timeout` or the `read_deadline` expired |
| `BodyTooLarge` | The body was larger than `max_body_bytes` |
| `Decode` | The body did not have the documented shape |

| Stage | Meaning |
|---|---|
| `NotStarted` | No byte of the request was sent: connecting, TLS included, failed or timed out |
| `ResponseReceived` | The response's status line arrived |
| `Unknown` | The request may or may not have reached Alpaca |

`may_have_reached_broker()` is false only at `NotStarted`. When it is true
for a request that changes state, such as an order, the request may have
taken effect: do not send it again until you know. For a timed-out order,
Alpaca advises against resending it or treating it as canceled until that
is confirmed, and advises checking the dashboard and contacting its
support.

For an `Api` error, suri reads the response body, up to `max_body_bytes`,
for Alpaca's error code and message:

- `code()` is the body's `code` when it is a non-negative integer written
  without a fraction or exponent, at most 18446744073709551615, and
  `message()` its `message` when it is a string; each is read on its own.
  A body that is a JSON string gives that string as the message, and any
  other non-empty body with neither field gives its whole text (bytes that
  are not UTF-8 become U+FFFD), so nothing Alpaca said is lost. A body that
  is empty, larger than `max_body_bytes` or cut short gives neither; the
  error is still `Api`, decided by the status alone.
- The status and `code()` are what to match on. Alpaca states that message
  texts can change without notice.
- Before it is kept, the message has the request's key ID and secret
  replaced with `[REDACTED]`, as well as a prefix of 8 or more characters of
  either that ends the text; control characters become spaces, so the
  message stays on one line; and it is cut to 1,024 bytes at a character
  boundary. The scrubbing cannot recognise a credential that a cut leaves
  shorter than 8 characters, or a partial credential followed by other text.
- A request with missing or wrong credentials may be answered 401 or 403;
  Alpaca's documentation shows both.

## Retries

suri retries reads (`GET` requests), and only reads. A request that
changes state, such as an order placement or a configuration update, is
sent at most once: no
status, failed connection or timeout leads suri to send it again, and its
timeout is `request_timeout` alone.

- **When:** a response of 429, 500, 502, 503 or 504, and a connection that
  fails or times out at any stage. Never a redirect, a body over the limit,
  a body that does not decode, or any other status.
- **How often and how long:** up to `max_read_attempts` attempts within
  `read_deadline` from the start of the first. Each attempt's timeout is
  `request_timeout` or the time left before the deadline, whichever is
  shorter. A wait that would end at or after the deadline is not taken: the
  call returns the last failure, and `attempt()` says which attempt it was.
- **Waits:** after attempt `n`, the backoff is `retry_base` × 2^(n−1), at
  most `retry_cap`. After a 429 with an `X-RateLimit-Reset`, the wait is the
  later of the backoff and the reset. Up to half the backoff is then added
  at random, so that many clients do not retry in step. A retry never comes
  before the backoff, or before the reset as this machine's clock reads it.

## Observability

suri reports through the `tracing` crate and installs nothing: no
subscriber, logger or global state. With no subscriber installed, nothing
is recorded.

- **One span per call:** `suri.request`, level INFO, target `suri`, covering
  every attempt and wait. Its fields: `method` (such as `GET`), `endpoint`
  (the path template, such as `/v2/clock`), `attempt` (recorded as each
  attempt starts), `status` (recorded when a response arrives) and `stage`
  (recorded only when the call returns an error: `not_started`,
  `response_received` or `unknown`).
- **Events,** level DEBUG, target `suri`, inside that span: `retry`
  (message `retrying`; fields `attempt`, `reason` — `status_429`,
  `status_5xx`, `transport` or `timeout` — and `wait_ms`) before each wait,
  and `error` (message `request failed`; field `kind`: `api`, `redirect`,
  `transport`, `timeout`, `body_too_large` or `decode`) once when the call
  returns an error.
- **Never recorded:** the URL or its query, a request or response body,
  Alpaca's message, the request ID, any header value and any credential.
- **Libraries suri uses** also log:
  - `reqwest`, through the `log` crate, at DEBUG and TRACE (targets
    `reqwest::connect` and `reqwest::retry`);
  - `hyper_util`, through `tracing`, mostly at DEBUG and TRACE and at WARN
    when a socket option cannot be set (targets under
    `hyper_util::client::legacy`);
  - the certificate verifier, through `log` (targets under
    `rustls_platform_verifier`): certificate errors and, on some systems,
    the loading of the system's root certificates when `Client::new` runs.

  These records name hosts, ports, certificate errors and counts, not
  credentials: suri's tests capture every record from suri and its HTTP
  libraries at TRACE, with requests that succeed, fail, retry and have
  Alpaca echo the credentials back, and find none. No filter is needed.
- **Streams** (the `stream` feature): one span per stream, `suri.stream`,
  level INFO, target `suri`, with the field `stream` (`trade_updates`,
  `stocks`, `crypto` or `news`) and, on the stock and crypto streams,
  `feed` (the stock feed's name, such as `iex` or `delayed_sip`, or the
  crypto location, such as `us-1`), covering the opening and every
  `subscribe`, `unsubscribe`, `next` and `close` call. Its events, level
  DEBUG: `connected`, `authenticated`, `listening` (`trade_updates`
  only), `ping` (suri sent a liveness Ping), and `ended`, once, when the stream ends for any reason
  other than being dropped, with the field `kind` (the error kind in snake
  case, such as `closed` or `transport`, or `closed_by_caller`). Never
  recorded: the URL, a frame or message, a close reason, Alpaca's message,
  a header value or a credential.
- **The WebSocket library, `tungstenite`, does log credentials.** The
  Trading stream authenticates with a message carrying the key ID and
  secret, a Market Data stream with headers carrying them, and
  `tungstenite` logs through `log`:

  | Target | Level | Record | What it holds |
  |---|---|---|---|
  | `tungstenite::handshake::client` | TRACE | the upgrade request | every request header, the key ID and secret of a Market Data stream included |
  | `tungstenite::protocol` | TRACE | `Sending frame` | every frame suri writes, as text: the authentication message with the key ID and secret, and the reply to a Close, which repeats its reason |
  | `tungstenite::protocol::frame` | TRACE | `writing frame`, `received frame` | every frame written or read, its payload in hex: the same content |
  | `tungstenite::protocol` | TRACE | `Received message` | every message read, as text |
  | `tungstenite::protocol` | DEBUG | `Received close frame`, `Replying to close` | a Close frame's reason, which a server may fill with what it was sent |

  Filter `tungstenite` at INFO or above (for example `tungstenite=info`) to
  drop every one of them. `tokio_tungstenite` logs at TRACE and DEBUG
  without frame contents. suri's tests capture every record at TRACE during
  a stream whose server echoes the credentials in its close reason and
  during a Market Data stream's opening, and find them only in these
  records, and in none once the filter applies.

## Response fields

These rules apply to every response type:

- **Money, prices and quantities** that Alpaca sends as strings are
  `Decimal`s holding the exact text (see "Decimal values"); a JSON number
  in such a field is accepted only if it is an integer, and is otherwise a
  `Decode` error rather than a rounded value.
- **Text enumerations** are `Inbound` values: a value suri does not know
  yet is kept as `Unknown` with its text (see "Unknown values").
- **Optional fields.** A field is always present (not an `Option`) only
  when Alpaca's schema requires it, or every documented example and
  recorded response carries it, and none of them leaves it out or sends
  `null`. Any other field is an `Option`, `None` when the response omits it
  or sends `null`, so an optional field missing from a response never fails
  the call; a response missing an always-present field fails with a
  `Decode` error.
- **Unknown fields** are ignored, so a field Alpaca adds never fails a call;
  suri models it in a later version. A field Alpaca deprecates is kept and
  marked `#[deprecated]`.

## Account

`Client::account` reads `GET /v2/account`: the account's ID and status
(always present) and its balances, buying power, margin figures and
restrictions. The options levels arrive as integers or, in some responses,
as strings of digits; both read as the same number.

- **Configuration.** `Client::account_configurations` reads `GET
  /v2/account/configurations`. `Client::update_account_configurations`
  sends `PATCH /v2/account/configurations` with only the fields set in the
  `ConfigurationsUpdate`, never a `null`, so a field left unset keeps its
  value; it is made once and never retried.
- **Activities.** `Client::account_activities` reads one page of `GET
  /v2/account/activities`. A fill (`activity_type` `FILL`) is an
  `Activity::Trade`; every other activity, including one of a type suri
  does not know, is an `Activity::NonTrade`. To read the next page, pass
  the last activity's `id` as `page_token`. Listing by types and by
  category cannot both be asked for: `ActivityFilter` holds one or the
  other.
- **Portfolio history.** `Client::portfolio_history` reads `GET
  /v2/account/portfolio/history`. Alpaca sends its values as JSON numbers;
  suri reads each to the correctly rounded nearest `f64`, which keeps every
  value seen so far exactly (a value with more than 15 significant digits
  that is not the shortest form of some `f64` would be rounded). The
  timestamps are Unix seconds, read as `Timestamp`s; a value outside years
  0000 to 9999 (such as milliseconds) fails the decode. A window is a
  `HistoryRange`: a period, a start, an end, or two of them.

## Assets

`Client::assets` lists `GET /v2/assets`, narrowed by an `AssetsQuery`;
`Client::asset` reads one asset by symbol or ID (`GET
/v2/assets/{symbol_or_asset_id}`), a crypto pair such as `BTC/USD` included
(see "Path values"). Margin requirements, order sizes and increments are
exact decimals. The deprecated `maintenance_margin_requirement`, a JSON
number, is read as the nearest `f64`. An exchange sent as `""` reads as
`Exchange::Empty`.

## Option contracts

`Client::option_contracts` reads one page of `GET /v2/options/contracts`,
narrowed by an `OptionContractsQuery`; to read the next page, pass the
page's `next_page_token` as `page_token`. `Client::option_contract` reads
one contract by symbol or ID (`GET /v2/options/contracts/{symbol_or_id}`).
The strike filters are sent as their exact decimal text. A deliverable's
`amount` is `None` while a delayed settlement leaves it undetermined, and
its `asset_id` is `None` for cash.

## Orders

`Client::place_order` sends `POST /v2/orders` with an `OrderRequest` and
returns the `Order` Alpaca reports.

- **Client order IDs.** `OrderRequest::new` fills `client_order_id` with a
  new `ClientOrderId`, a random version 4 UUID, so the ID can be read,
  logged or stored before the order is sent. A caller may replace it with
  its own, of 1 to 128 characters, but cannot leave it out.
- **Made once.** A placement, a replacement and a cancellation are never
  retried (see "Retries"). Every error of a placement carries the order's
  ID in `client_order_id()`, and every error of a replacement the
  replacing order's ID, whatever went wrong.
- **After an error.** When `may_have_reached_broker()` is true, the order
  may have been placed. Before deciding anything, look it up with
  `Client::order_by_client_order_id` (`GET
  /v2/orders:by_client_order_id`): an order means it was placed; a 404
  `Api` error means Alpaca had no order with that ID when it answered
  (Alpaca does not document this response). Alpaca advises against resending a
  timed-out order.
- **Shapes.** An `OrderShape` is one of Alpaca's order classes with every
  part that class requires: `Simple`, `Bracket` (an entry with a
  take-profit and a stop-loss), `Oto` (an entry with one exit), `Oco` (two
  exits, sent as a limit order) and `MultiLeg` (up to four option legs, with
  no symbol or side of its own). An order missing a required part, or with
  both a quantity and a notional, cannot be built. Which combinations of
  type, time in force and asset Alpaca accepts is Alpaca's to check.
- **Wire form.** An `OrderRequest`'s JSON (`Serialize`) is exactly the body
  sent: prices and quantities as their exact decimal text, and a field left
  unset is left out, never sent as `null`.
- **The order.** A multi-leg order's parent may come with an empty symbol
  and side (the documented REST response example sends both as `""`; the
  documented trade-updates example carries a side), and its legs with an
  empty type; an empty value is kept as `""`, or as `Unknown` where the
  enumeration does not document one.
- **Finding orders.** `Client::orders` reads one page of `GET /v2/orders`,
  narrowed by an `OrdersQuery`; its `OrderWindow` is a time window or the
  orders before or after one order, which Alpaca does not combine.
  `Client::order` reads one order by ID (`GET /v2/orders/{order_id}`), with
  its legs nested when asked.
- **Replacing.** `Client::replace_order` sends `PATCH
  /v2/orders/{order_id}` with only the fields set in a
  `ReplaceOrderRequest`, and returns the replacing order: a new order, with
  its own client order ID, which `ReplaceOrderRequest::new` generates.
  Success does not guarantee the replacement: if the order fills before the
  replacing order reaches the market, the replacing order is rejected.
- **Cancelling.** `Client::cancel_order` sends `DELETE
  /v2/orders/{order_id}` and returns `Response<()>`: success means Alpaca
  accepted the cancellation, and a 422 `Api` error that the order can no
  longer be cancelled. `Client::cancel_all_orders` (`DELETE /v2/orders`)
  returns one `CancelResult` per order with its own status, Alpaca's code
  and message for a failed one, and any body a successful one has, as text;
  every kept text is scrubbed and bounded as an error's message is. Nothing
  in one result's body fails the others; a result without its `id` or
  `status` fails the call with a `Decode` error.

## Positions

`Client::positions` lists the open positions (`GET /v2/positions`) and
`Client::position` reads one by symbol or asset ID; amounts are exact
decimals, and an option position's exchange is `Exchange::Empty`. The
market value, current and last day's prices, change and unrealized profit
and loss are `None` while Alpaca has no market price for the asset, as for
an allocated IPO before it starts trading.

- **Closing.** `Client::close_position` (`DELETE
  /v2/positions/{symbol_or_asset_id}`) closes a position by placing an
  order, and returns it. A `ClosePosition` closes all of it, a quantity, or
  a percentage on a 0 to 100 scale (`0.5` is half a percent), sent as its
  exact decimal text. Alpaca takes no client order ID for it, so after an
  error that may have reached Alpaca, read the orders and positions to find
  out what happened.
- **Closing all.** `Client::close_all_positions` (`DELETE /v2/positions`),
  optionally cancelling open orders first, returns one `ClosedPosition` per
  position: its status, the closing order for a successful one (or the
  body as text, if it is not an order), and Alpaca's code and message for a
  failed one. Every kept text is scrubbed and bounded as an error's message
  is. Nothing in one result's body fails the others; a result without its
  `symbol` or `status` fails the call with a `Decode` error.
- **Options.** `Client::exercise_option` (`POST
  /v2/positions/{symbol_or_contract_id}/exercise`) and
  `Client::decline_exercise` (`POST
  /v2/positions/{symbol_or_contract_id}/do-not-exercise`) return
  `Response<()>`; success means Alpaca accepted the instruction. Alpaca's
  options guide asks users to contact its support for a do-not-exercise
  instruction instead.
- **Made once.** Closing, closing all, exercising and declining are never
  retried (see "Retries").

## Watchlists

`Client::watchlists` lists the account's watchlists, without their assets,
and `Client::watchlist` reads one by ID with its assets, in the order they
were added.
Watchlists are addressed by ID; Alpaca's by-name forms are not offered,
since the ID forms cover every operation.

- **Changing them.** `Client::create_watchlist` sends a `CreateWatchlist`
  (a name, and symbols if set). `Client::update_watchlist` sends a
  `WatchlistUpdate`: a new name, a new list of symbols that replaces the
  old one (an empty list empties the watchlist), or both.
  `Client::add_to_watchlist` adds one symbol at the end,
  `Client::remove_from_watchlist` removes one, and
  `Client::delete_watchlist` deletes the watchlist and returns
  `Response<()>`.
- **Made once.** Each of these five changes is never retried (see
  "Retries").

## Streams

The `stream` feature (off by default; it requires `rest`) adds the Trading
API's `trade_updates` stream and the Market Data streams (see "Market
data" below).

- **Opening.** `Client::trade_updates` connects to the client's host with
  the scheme `wss` (or `ws` for a loopback `http` base URL) and the path
  `/stream`: `wss://paper-api.alpaca.markets/stream` on paper,
  `wss://api.alpaca.markets/stream` live. It sends the documented
  authentication message, `{"action":"auth","key":…,"secret":…}`, then
  `{"action":"listen","data":{"streams":["trade_updates"]}}`, and returns
  once Alpaca confirms both. The upgrade is bounded by `connect_timeout`,
  the whole opening by `request_timeout`; either bound passing is
  `Timeout`. A TLS configuration that cannot be built is `Tls`, an HTTP
  status other than 101 `Handshake`, rejected credentials `Unauthorized`
  (never retried), a confirmation without `trade_updates` `NotListening`.
  suri offers no WebSocket extension and refuses an upgrade that claims
  one with `Handshake`.
- **Messages.** `TradeUpdates::next` returns the next message as a
  `TradeStreamMessage` (see "Trade updates" below); a message suri does not
  type keeps its `stream` and its `data`, the raw JSON text as sent, in a
  `StreamMessage`. Text and binary frames (paper sends binary) are read
  alike; Ping and Pong frames are answered and never delivered. A message
  larger than `max_body_bytes` ends the stream with `TooLarge`.
- **Polling.** The `TradeUpdates` value owns its connection and reads it
  only while `next` or `close` runs: there is no background task and no
  queue, so suri drops nothing. While `next` waits, after
  `request_timeout` without any frame suri sends a Ping, and if nothing
  arrives within another `request_timeout` the stream ends with `Timeout`.
  A caller that stops calling `next` stops answering Alpaca's Pings; what
  Alpaca then does is reported by the next call. `next` is cancel-safe:
  dropping its future loses no message.
- **Ends.** Every end other than `close` is one error saying later messages
  may be lost, and every later call returns `Ended`. Alpaca's Close frame is
  `Closed` with its code and reason (a code a client may not receive
  arrives as 1002); Alpaca's `error` message is `Server` with its text; a
  frame that is not a stream message is `Decode`; a failed connection is
  `Transport`. suri answers Alpaca's Close before dropping the connection.
  There is no reconnection: call `Client::trade_updates` again, and expect
  that messages sent meanwhile may be lost (the stream has no resume key).
- **Closing.** `TradeUpdates::close` sends a Close frame and waits up to
  `request_timeout` for Alpaca's; it returns the messages that arrived
  meanwhile together with the result, whatever the result. If Alpaca's
  Close does not arrive in time the result is `Timeout`, whose message says
  only that the server did not answer the close: the caller asked for the
  end, so no later message is owed. If a cancelled `next` had already
  read Alpaca's Close, `close` returns that `Closed` error. It is not
  cancel-safe: dropping its future drops the connection and those
  messages. Dropping `TradeUpdates` without `close` drops the connection
  without a Close frame.
- **Scrubbing.** A close reason and Alpaca's error message are scrubbed of
  the key ID and secret and bounded as an error's message is (see
  "Errors"); a close reason holds at most 123 bytes, so a cut credential of
  8 or more characters at its end is replaced too.
- **Trade updates.** A `trade_updates` message whose `data` reads as a
  `TradeUpdate` is `TradeStreamMessage::Update`: its `event`, a
  `TradeEvent` (the 17 documented values; another text is kept in
  `Inbound::Unknown`), the `order` as the REST API's `Order`, and the
  fields only some events carry, each optional: `timestamp`,
  `execution_id`, `price`, `qty` and `position_qty` (exact decimals), and
  on multi-leg fills `at`, `event_id`, `position_qtys` and the legs'
  fills, `LegFill`. The stream's orders carry `cancel_requested_at`, which
  the Trading REST order schema does not list (`None` when absent). A
  `trade_updates` message that does not read as a `TradeUpdate` (no
  `event` or `order`, a decimal that `Decimal` rejects, see "Decimal
  values", a malformed time) is `TradeStreamMessage::Undecoded` with the
  message as sent, and the stream goes on. A message of any other stream
  is `TradeStreamMessage::Other`, as sent. `close` returns its messages
  typed the same way.

### Market data

`Client::stock_stream`, `Client::crypto_stream` and `Client::news_stream`
open the Market Data streams on `wss://stream.data.alpaca.markets`, paper
and live alike (a base-URL override replaces the host, with the scheme
`ws` or `wss`): a stock stream at the path its `StreamFeed` names
(`/v2/sip`, `/v2/iex`, `/v2/delayed_sip`, `/v1beta1/boats`,
`/v1beta1/overnight`, or the test stream `/v2/test`, open at all hours
with the symbol `FAKEPACA`), the crypto stream of a `Location` at
`/v1beta3/crypto/{loc}` (Alpaca's stream describes `us`, `us-1` and
`eu-1`), and news at `/v1beta1/news`. Each returns a `DataStream` of its
family's `Message` (`stocks::Message`, `crypto::Message`,
`news::Message`), subscribed to nothing.

- **Opening.** The key ID and secret go in the upgrade's
  `APCA-API-KEY-ID` and `APCA-API-SECRET-KEY` headers, so no frame
  carries them, and suri writes nothing before the first `subscribe`. The
  stream opens once Alpaca's `connected` and `authenticated` messages
  arrive, in that order, each alone in its frame; another message in
  their place is `Decode`. An `error` message during the opening ends it:
  code 402 is `Unauthorized`, any other code `Server`, each with
  `StreamError::code` and Alpaca's message, scrubbed. The time bounds,
  TLS, `Handshake` and the refusal of an extension are as for
  `trade_updates`.
- **Subscriptions.** `subscribe` and `unsubscribe` send a `Channels`: its
  non-empty channels, in the documented order, each a list of symbols
  (`"*"` for every symbol). Each returns once the message is written,
  within `request_timeout`; Alpaca's reply, the whole current
  subscription, arrives through `next` as a `Subscription` message, or an
  `Error` message. A failed write ends the stream, and a write not done in
  time ends it with `Timeout`. Neither call is cancel-safe: a dropped
  future may or may not have sent the message. A channel a stream does
  not offer is Alpaca's to refuse.
- **One frame per call.** `next` returns the messages of one frame, in the
  order sent, never an empty list (an empty frame is skipped); suri holds
  nothing between calls. Text and binary frames are both read as one
  JSON array; a frame that is anything else ends the stream with
  `Decode`. `close` returns the messages of every frame read while
  closing.
- **Every message reaches the caller.** A message of a type (`T`) suri
  knows that reads as its model is typed, a data message with its symbol
  (`S`). A known type that does not read, a data message without its
  symbol included, is `Undecoded`, holding its JSON text as sent. An
  unknown type, a `success` after the opening, an element that is not an
  object, or one whose `T` or `S` is not text, is `Other`, as sent.
  Neither ends the stream.
- **Messages.** Stocks: trades, quotes, minute, daily and updated bars
  (the REST models), trade corrections, trade cancels and errors, LULD
  bands, trading statuses (their codes kept as text) and order
  imbalances. Crypto: trades, quotes, bars and orderbooks; an orderbook
  with `reset` set is the whole book, one without it an update in which a
  size of 0 removes the level, and suri delivers each as sent. News:
  articles, as `news::Article`, without images.
- **Errors.** No `error` message ends a stream after the opening; it
  arrives as an `Error` message with Alpaca's code and scrubbed message:

  | Code | Meaning | During the opening | After it |
  |---|---|---|---|
  | 402 | auth failed | `Unauthorized` | `Error` message |
  | 405 | symbol limit exceeded; the subscription is unchanged | `Server` | `Error` message |
  | 407 | slow client; it may not precede the disconnect | `Server` | `Error` message; the Close that follows ends the stream |
  | 400, 401, 403, 404, 406, 409, 410, 500, any other | as Alpaca documents them | `Server` | `Error` message |

- **No compression.** suri offers no WebSocket extension, so every frame
  arrives uncompressed, a `"*"` subscription included.
- **Shared rules.** Polling, liveness, the ends, `Ended`, closing and
  scrubbing are as for `trade_updates`.

## Market data

Market Data types are in `suri::data` and its per-asset modules
(`suri::data::stocks`, `suri::data::crypto`, `suri::data::options`) and
`suri::data::news`; the methods are on `Client`.

- **Host.** Requests go to `https://data.alpaca.markets` for paper and live
  clients alike (paper Trading users read market data from the production
  host); a base-URL override replaces this host too. Every request sends
  the key-pair headers. Every endpoint is a read, retried as "Retries" says.
- **Numbers.** Prices, VWAPs and every other value the docs type as a
  double arrive as `f64`, read correctly rounded to the nearest `f64`
  (which keeps every value seen so far; a value with more than 15
  significant digits that is not the shortest form of some `f64` would be
  rounded); crypto sizes and volumes are doubles, so `f64`. Sizes,
  volumes, counts and IDs the docs type as integers are integers of the
  width their documented format gives (`u32`, `u64`, `i64`); a fraction or
  a value out of range there fails the decode.
- **Times.** Every time is a `Timestamp` read from RFC 3339 text with up to
  nine fractional digits; an auction day is a `Date`. Query bounds
  (`start`, `end`, `updated_since`) are `Timestamp`s, sent as `…Z`; an
  option chain's expiration filters are `Date`s, sent as `YYYY-MM-DD`.
- **Pages.** A historical endpoint returns a `Series`: each symbol's points
  in the order sent, the `currency` when Alpaca names it, and
  `next_page_token`. One call returns one page. A page may hold fewer
  points than the query's `limit` even when more exist; only a `None`
  token means there is no next page. A symbol whose value is `null`, or a
  point of another shape, fails the call.
- **Queries.** Lists are sent comma-joined. A `Timeframe` is sent in
  Alpaca's long form (`15Min`, `1Hour`, `1Day`, `1Week`, `3Month`); its
  amount, like `limit` and every other range, is checked by Alpaca, not by
  suri. A parameter not set is not sent, and Alpaca's default applies.

### Historical stock data

`Client::stock_bars` (`GET /v2/stocks/bars`), `stock_trades`
(`/v2/stocks/trades`), `stock_quotes` (`/v2/stocks/quotes`) and
`stock_auctions` (`/v2/stocks/auctions`) each read one page for a list of
symbols, sorted by symbol, then time. `BarsQuery` adds the timeframe and
the adjustments (`Adjustment`, comma-joined); every query takes the feed
(`Feed`: which feeds an endpoint and a subscription allow is Alpaca's to
decide; auctions accept only `sip`), the symbol mapping (`AsOf`: a date,
or `Unmapped`, sent as `-`) and the currency. A trade's `update` is `None`
when the trade is valid; a quote's price of 0 means no active bid or ask.

### Latest stock data and snapshots

`Client::stock_latest_bars` (`GET /v2/stocks/bars/latest`),
`stock_latest_trades` (`/v2/stocks/trades/latest`) and
`stock_latest_quotes` (`/v2/stocks/quotes/latest`) return a `Latest`: each
symbol's latest minute bar, trade or quote, and the currency when Alpaca
names it; a symbol with no data is absent. Alpaca leaves out of the latest
trades those whose conditions do not update a bar's price.
`Client::stock_snapshots` (`/v2/stocks/snapshots`) returns `Snapshots`:
each symbol's latest trade, latest quote, minute bar, daily bar and
previous daily bar, any of which Alpaca may leave out, and a `currency`
Alpaca may send beside the symbols. Each takes a `LatestQuery` (symbols,
feed, currency); without a feed, Alpaca picks one by the caller's
subscription.

### Stock codes

`Client::stock_conditions` (`GET /v2/stocks/meta/conditions/{ticktype}`)
returns the names of a tape's trade or quote condition codes, and
`Client::stock_exchanges` (`/v2/stocks/meta/exchanges`) the names of the
exchange codes, each as a map from code to name. The tick type
(`TickType`) is a path segment; the tape (`ConditionsQuery`) is sent as
given, and Alpaca decides which tapes it lists codes for.

### Crypto

Every crypto method takes a `Location` first, the path segment Alpaca
calls `loc`: `Us` (Alpaca US), `Us1` (Kraken US) and `Eu1` (Kraken EU),
which Alpaca describes, and `Us2` and `Bs1`, which are in its schema
only. `Client::crypto_bars`, `crypto_trades` and `crypto_quotes` read one
page of history (`suri::data::BarsQuery` and `HistoryQuery`, without the
stock-only parameters); `crypto_latest_bars`, `crypto_latest_trades`,
`crypto_latest_quotes`, `crypto_latest_orderbooks` and `crypto_snapshots`
take a `SymbolsQuery` and return a `Latest`. Pairs are written with a
slash (`BTC/USD`), sent percent-encoded and returned as written. Sizes and
volumes are `f64`; a trade's ID is an `i64`. An orderbook's levels come as
sent, in Alpaca's order, a size of 0 included; its `reset` is `false`
over REST. The key pair is sent on crypto requests too, although Alpaca
documents historical crypto data as needing none.

### Options

Symbols are option contract symbols (`AAPL240315C00172500`).
`Client::option_bars` (`GET /v1beta1/options/bars`) and `option_trades`
(`/v1beta1/options/trades`) read one page of history, sorted by symbol,
then time, with the shared `BarsQuery` and `HistoryQuery`; they take no
feed, and which feed serves them Alpaca does not say. Alpaca keeps option
history from February 2024. `option_latest_trades`
(`/v1beta1/options/trades/latest`) and `option_latest_quotes`
(`/v1beta1/options/quotes/latest`) take a `LatestQuery` (symbols, feed)
and return a `Latest`; a body whose map is under none of the keys a
`Latest` reads (`bars`, `trades`, `quotes`, `orderbooks`, `snapshots`),
such as one under `trade`, fails the call.
`option_snapshots` (`/v1beta1/options/snapshots`) takes a `SnapshotsQuery`
and `option_chain` (`/v1beta1/options/snapshots/{underlying_symbol}`) an
underlying `Symbol` and a `ChainQuery`; both return one page of
`Snapshots`, each contract's latest trade and quote, bars, greeks and
implied volatility, any of which Alpaca may leave out (greeks and
implied volatility need a latest quote with a non-zero bid and ask, the
underlying's latest trade, an expiry after today and an implied
volatility Alpaca's model can compute), and `next_page_token`. The
chain's strike filters are sent as their exact decimal text and its
contract type as `call` or `put`. `option_conditions` and
`option_exchanges` (`/v1beta1/options/meta/…`) return the option
condition and exchange code tables, code to name. A trade or quote
carries one condition code, which may be a space; exchange codes are
option exchange codes and keep their case. Without a feed (`Feed`:
`opra` or `indicative`), Alpaca picks one by the caller's subscription.

### News

`Client::news` (`GET /v1beta1/news`) reads one `Page` of articles across
stocks and crypto, with `next_page_token`; with no parameter set, Alpaca
returns the latest 10. A `Query` takes the interval, the order by update
time (`Sort`, Alpaca's default descending), the symbols (comma-joined),
`limit`, and the `include_content` and `exclude_contentless` flags, sent
as `true` or `false`. An `Article`'s text is read as sent: Alpaca's
schema forbids empty text, but its responses carry empty summaries and
contents, and the content may contain HTML. An article without a URL, or
with a `null` one, has `url` `None`; one without an image list has no
images. An image size Alpaca has not documented is kept as `Unknown`.

## Market clock

`Client::clock` reads `GET /v2/clock`: the server's time, whether the market
is open, and the next open and close, each a `Timestamp`.

## Calendar

`Client::calendar` reads `GET /v2/calendar`: each market day's date, the
market's open and close, the extended session's open and close, and the
settlement date when it is sent. The four times are `ClockTime`s, a local
market time to the minute with no date and no time zone, as Alpaca sends
them: `HH:MM` for the market's hours and `HHMM` for the session's, each
with two digits per part; a time in any other form fails with a `Decode`
error. A `CalendarQuery`'s `start` and `end` are sent as dates
(`YYYY-MM-DD`).

## Decisions

| Topic | Decision | Reason |
|---|---|---|
| Decimal representation | The exact text, with no arithmetic | suri does no arithmetic, and Alpaca's money strings can exceed the range of common decimal crates |
| Decimal equality | Text equality | It is the value as sent; numeric equality would need a normalisation suri has no use for |
| Time precision | Nanoseconds, UTC; the offset is applied and dropped | Alpaca documents nanosecond precision, and a single instant type compares correctly whatever offset each value was sent with |
| Leap seconds and long fractions | Rejected, never folded or trimmed | Silent adjustment would change a value without saying so |
| Unknown enum values | Kept as `Unknown` with their text | A new status must not fail a whole response, and an unknown value must not pass for a known one |
| Path values | Checked types with an allowlist of bytes | A `.` or `..` segment would silently change which endpoint a request reaches |
| Hosts | Paper and live Trading hosts for the US region; others arrive with their endpoints | The first release covers Alpaca's US Trading and Market Data APIs |
| Environment variables | None read | A library that reads its caller's environment can send credentials or requests somewhere the caller did not choose |
| Redirects | Never followed; an error naming only the target's origin | A followed redirect could resend the key pair or a request that changes state to another host |
| Unknown JSON fields | Ignored | Alpaca adds response fields without changing the API version |
| Market clock and calendar versions | `/v2/clock` and `/v2/calendar` | They report the US equity market the first release covers; the newer clock and calendar add other markets |
| Retries | Reads only, on 429, 500, 502, 503, 504 and failed connections, with capped exponential backoff and added jitter, never before a rate-limit reset by this machine's clock, within a deadline | A read is safe to repeat; Alpaca asks clients to back off, wait for the reset and add jitter, and documents 500 as worth retrying later |
| HTTP version | HTTP/1.1 only | One connection model to reason about and test; the documentation leaves the protocol version open and lists new HTTP versions among the changes Alpaca makes without notice |
| Compression | Uncompressed responses requested (`Accept-Encoding: identity`); nothing is decompressed | No decompression code or limits to get right; Alpaca says it may add gzip, so suri asks for none rather than accept any |
| Response field types | Exact decimals for amounts sent as strings, `Inbound` for text enumerations, optional unless always present | A response field that does not fit its type fails the call, so types follow the documented wire form and leave room for what Alpaca adds |
| Optional fields | Always present only when the schema requires a field, or every documented example and recorded response carries it, and none omits it; otherwise an `Option` | A field missing from a real response must not fail the call; an `Option` loses nothing, and a missing always-present field is reported as a `Decode` error |
| Mutations are never retried | One attempt for every request that is not a `GET` | Alpaca advises against resending a request whose outcome is unknown; only the caller can find out whether it took effect |
| Paging | One page per call; the caller passes the cursor | A loop inside suri would hide how many requests a call makes and when it stops |
| Numbers received as numbers | Portfolio history's JSON numbers, an asset's deprecated `maintenance_margin_requirement` and every Market Data double as the correctly rounded nearest `f64` | They are binary floating-point values on the wire; reading them correctly rounded keeps every observed value, and money sent as strings stays exact |
| Symbols in paths | Percent-encoded as one segment (`BTC/USD` as `BTC%2FUSD`) | Alpaca asks for the encoded form; a bare `/` would split the value into two segments and reach another path |
| Client order IDs | Generated by `OrderRequest::new` as a random version 4 UUID; replaceable, never absent | Without one, an order whose response is lost cannot be looked up; Alpaca generates one only on its side, where the caller cannot see it |
| Order placement is never retried | One attempt, every error carrying the client order ID | Alpaca advises against resending a timed-out order; a lookup, the dashboard or Alpaca's support can tell whether it was placed |
| Order shapes | One variant per order class, each with the fields that class requires | A missing take-profit or leg list, or a quantity and a notional together, is a mistake caught when the code is compiled rather than by a rejected order |
| Stream ownership | The stream value owns its connection and reads it only while the caller polls; no background task, no queue | Nothing can be dropped inside suri, and there is no overload to report; the caller decides when to read |
| Stream liveness | A Ping after `request_timeout` without any frame while polled, and `Timeout` after another `request_timeout` | A connection that died without a Close would otherwise leave `next` waiting forever; Alpaca documents no keep-alive for this stream |
| Stream reconnection | None; the caller opens a new stream | Reconnecting hides a gap the caller cannot see, and the stream has no resume key |
| Trade-updates authentication | The documented `auth` message, not alpaca-py's `authenticate` form | The docs are the specification |
| Stream feature | Off by default and requires `rest` | Most callers use REST; the stream reuses the client and its types |
| Unreadable trade updates | Delivered as sent (`Undecoded`); the stream goes on | A stream message cannot be fetched again; the raw message keeps its order ID for the caller to reconcile, and ending the stream would lose every later update too |
| Market-data authentication | The key-pair headers on the upgrade | The docs offer them beside an `auth` message; headers keep the secret out of every frame |
| Market-data error codes | During the opening every `error` ends it (402 `Unauthorized`, others `Server`); after it, none ends the stream by itself | The stream is unusable until authenticated; the docs never say an error ends an open stream, 405 leaves it as it was, and the disconnect that may follow 407 is a Close |
| One frame per call | `DataStream::next` returns one frame's messages; suri keeps no queue | Nothing waits inside suri to be dropped, and a call stays cancel-safe |
| Stream compression | None offered; every frame arrives uncompressed | Offering it is a later, additive change once a need is measured |
| Market Data host | `https://data.alpaca.markets` for paper and live; the base-URL override replaces it | Paper Trading users read market data from the production host; the sandbox data host is for Broker partners |
| Single-symbol stock endpoints | Not offered; pass one symbol to the multi-symbol method | They return the same data in another envelope |
| Request times | `start`, `end` and `updated_since` take a `Timestamp` only | The docs accept a bare date without saying in which time zone it is read |
| Stock feeds | One `Feed` for every stock endpoint | The docs' feed lists differ by endpoint and their guide says the parameter works everywhere; Alpaca decides which it accepts |
| Market-data integers | The width of the documented format (`u32`, `u64`, `i64`) | A value the format allows always decodes |

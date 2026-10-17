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
   suri receives reaches you or is reported as an error, and a stream that
   ends without your asking ends with an error saying later messages may be
   lost.
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
  one value.

## Configuration

A `Client` is built from a `Config`: a region, an environment and an API key
pair, with every limit at a documented default. `Client::new` checks the
whole configuration and reports the first problem as a `ConfigError`, so a
bad value fails when the client is built, never at request time.

- **Hosts:** region `US` with environment `Paper` uses
  `https://paper-api.alpaca.markets`, and `Live` uses
  `https://api.alpaca.markets`. Each endpoint carries its own version prefix,
  such as `/v2`.
- **Credentials:** the key ID and secret must be non-empty printable ASCII
  (bytes `!` to `~`), so a trailing newline read from a file is caught here.
  Neither can be read back from suri, and `Debug` prints `Credentials { .. }`.
- **Limits:**

  | Value | Default | Allowed |
  |---|---|---|
  | `connect_timeout`: connecting, TLS included | 10 s | 1 ms to 60 s, and no longer than `request_timeout` |
  | `request_timeout`: one request, from connecting until the whole body has arrived | 30 s | 1 ms to 10 min |
  | `max_body_bytes`: the largest response body read | 32 MiB | 1 KiB to 1 GiB |
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
  the TLS library's certificate verifier, which on Unix systems other than
  macOS, iOS and Android reads `SSL_CERT_FILE` and `SSL_CERT_DIR` in place of
  the system's certificate store.

## Requests

- Each request carries the key pair in the `APCA-API-KEY-ID` and
  `APCA-API-SECRET-KEY` headers, marked sensitive, and
  `User-Agent: suri/<version>`.
- suri speaks HTTP/1.1 directly to the host: no proxy, and no compression
  (it sends `Accept-Encoding: identity` and decompresses nothing).
- A redirect is never followed; it is an error (below).
- A value placed in a URL path, such as a symbol, is sent as one encoded
  segment: `BTC/USD` is sent as `BTC%2FUSD`.

## Responses

A successful call returns a `Response`: the decoded data and its
`ResponseMeta`, which holds the status, the `X-Request-ID` header when sent
(Alpaca's support asks for it) and the rate-limit headers when all three
(`X-RateLimit-Limit`, `X-RateLimit-Remaining` and `X-RateLimit-Reset`, read
as Unix seconds) were sent as whole numbers. Fields a response has beyond the
documented ones are ignored, so a field Alpaca adds never fails a call.

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
taken effect: as Alpaca advises for a timed-out order, find out (look the
order up) before sending it again, never resend it blindly.

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

suri retries reads, and only reads; every endpoint it calls so far is one.

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
  before the backoff or the reset.

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
    `rustls_platform_verifier`): ERROR when a certificate fails
    verification and, on Linux and other Unix systems except macOS, iOS and
    Android, WARN and DEBUG when `Client::new` loads the system's root
    certificates.

  These records name hosts, ports, certificate errors and counts, not
  credentials: suri's tests capture every record from suri and its HTTP
  libraries at TRACE, with requests that succeed, fail, retry and have
  Alpaca echo the credentials back, and find none. No filter is needed.

## Market clock

`Client::clock` reads `GET /v2/clock`: the server's time, whether the market
is open, and the next open and close, each a `Timestamp`.

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
| Market clock version | `/v2/clock` | It reports the US equity market the first release covers; the newer clock adds other markets |
| Retries | Reads only, on 429, 500, 502, 503, 504 and failed connections, with capped exponential backoff and added jitter, never before a rate-limit reset, within a deadline | A read is safe to repeat; Alpaca asks clients to back off, wait for the reset and add jitter, and documents 500 as worth retrying later |

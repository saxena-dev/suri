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

## Decisions

| Topic | Decision | Reason |
|---|---|---|
| Decimal representation | The exact text, with no arithmetic | suri does no arithmetic, and Alpaca's money strings can exceed the range of common decimal crates |
| Decimal equality | Text equality | It is the value as sent; numeric equality would need a normalisation suri has no use for |
| Time precision | Nanoseconds, UTC; the offset is applied and dropped | Alpaca documents nanosecond precision, and a single instant type compares correctly whatever offset each value was sent with |
| Leap seconds and long fractions | Rejected, never folded or trimmed | Silent adjustment would change a value without saying so |

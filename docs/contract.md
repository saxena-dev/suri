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

## Decisions

| Topic | Decision | Reason |
|---|---|---|
| Decimal representation | The exact text, with no arithmetic | suri does no arithmetic, and Alpaca's money strings can exceed the range of common decimal crates |
| Decimal equality | Text equality | It is the value as sent; numeric equality would need a normalisation suri has no use for |

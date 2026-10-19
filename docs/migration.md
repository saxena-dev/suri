# Migrating between versions

This guide lists every break between suri's versions and what to do about
it. What suri does is defined by its [contract](contract.md); the
[changelog](../CHANGELOG.md) lists every change.

## Compatibility policy

- **Breaks ship only in minor versions before 1.0.** A break goes into a
  new minor version (0.1 to 0.2), never a patch (0.1.0 to 0.1.1), so
  Cargo's default `^0.1` requirement never picks one up. Each break is
  listed below with an ID (`BR-1`, `BR-2`, …) and what to do.
- **A change in behaviour is a break.** Observable behaviour is the
  contract, so a change that compiles unchanged but behaves differently is
  a break too, and it is listed first, because the compiler will not point
  to it.
- **The minimum Rust version and the default features.** suri needs Rust
  1.88.0 or later (`rust-version` in `Cargo.toml`), and its default feature
  set is `rest`. A change to either ships in a minor version, never a
  patch, and is listed here.

## 0.1.0

The first release: there is nothing to migrate from.

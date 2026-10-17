#!/usr/bin/env bash
# Local quality gate.
#
#   scripts/verify.sh           baseline for every task: formatting, clippy on
#                               all targets with every feature, tests with every
#                               feature (doc tests included)
#   scripts/verify.sh --rows    baseline, then clippy on every feature row's
#                               targets with warnings denied; use when a change
#                               adds or alters build targets or features
#   scripts/verify.sh --full    formatting, clippy and tests on every feature
#                               row, rustdoc with warnings denied, package
#                               contents; use at epic close and before a release
#   scripts/verify.sh --full --msrv
#                               also checks every row and runs the tests on the
#                               toolchain named by rust-version in Cargo.toml
#                               (fails while Cargo.toml sets none)
#   scripts/verify.sh --tree-hash
#                               print the hash of the uncommitted changes and
#                               exit, to compare with a log header
#
# Each run writes its own log under ${CARGO_TARGET_DIR:-target}/verification/.
# The log header records HEAD and a hash of the uncommitted changes, so a run
# that started before the last edit can be told apart from one that did not.
#
# Exits non-zero on the first failure. Every cargo call uses --locked. Never
# runs cargo publish in any form and never contacts the network for
# documentation.
set -euo pipefail
cd "$(dirname "$0")/.."

tree_hash() {
  {
    git diff HEAD
    git ls-files -z --others --exclude-standard \
      | while IFS= read -r -d '' f; do shasum -a 256 "$f"; done
  } | shasum -a 256 | cut -c1-16
}

mode=base
msrv=0
for arg in "$@"; do
  case "$arg" in
    --tree-hash) tree_hash; exit 0 ;;
    --rows) mode=rows ;;
    --full) mode=full ;;
    --msrv) msrv=1 ;;
    *) echo "usage: scripts/verify.sh [--rows | --full [--msrv] | --tree-hash]" >&2; exit 2 ;;
  esac
done
if [ "$msrv" = 1 ] && [ "$mode" != full ]; then
  echo "--msrv needs --full" >&2
  exit 2
fi

# Feature rows: no features, each feature alone, every feature. The crate has
# no features yet; add a row for each feature when one lands.
ROWS=(none all)

row_args() {
  case "$1" in
    none) echo "--no-default-features" ;;
    all) echo "--all-features" ;;
    *) echo "--no-default-features --features $1" ;;
  esac
}

log_dir="${CARGO_TARGET_DIR:-target}/verification"
mkdir -p "$log_dir"
log="$log_dir/verify-$mode$([ "$msrv" = 1 ] && echo -msrv)-$(date -u +%Y%m%dT%H%M%SZ).log"
exec > >(tee "$log") 2>&1

start_hash=$(tree_hash)
echo "verify ($mode) at $(git rev-parse --short HEAD), working-tree hash $start_hash"
echo "log: $log"

run() {
  echo "+ $*"
  "$@"
}

run cargo fmt --all -- --check

if [ "$mode" != full ]; then
  run cargo clippy --locked --all-targets --all-features -- -D warnings
  run cargo test --locked --all-features
fi

if [ "$mode" = rows ]; then
  for row in "${ROWS[@]}"; do
    [ "$row" = all ] && continue   # the baseline above already ran it
    # shellcheck disable=SC2046 # row_args output is deliberately word-split
    run cargo clippy --locked --all-targets $(row_args "$row") -- -D warnings
  done
fi

if [ "$mode" = full ]; then
  for row in "${ROWS[@]}"; do
    # shellcheck disable=SC2046
    run cargo clippy --locked --all-targets $(row_args "$row") -- -D warnings
    # shellcheck disable=SC2046
    run cargo test --locked $(row_args "$row")
  done
  RUSTDOCFLAGS="-D warnings" run cargo doc --locked --all-features --no-deps

  # The package never ships vendored sources, tooling, tests and their
  # fixtures, hidden directories or top-level Markdown other than README and
  # CHANGELOG.
  list=$(cargo package --locked --list --allow-dirty)
  bad=$(printf '%s\n' "$list" \
    | grep -E '^(vendor|tools|tests)/|^\.[^/]+/|^[^/]+\.md$' \
    | grep -vxE 'README\.md|CHANGELOG\.md' || true)
  if [ -n "$bad" ]; then
    echo "the package must not contain:" >&2
    echo "$bad" >&2
    exit 1
  fi
  echo "+ package contains no vendored, tooling, test or maintainer-local files"

  if [ "$msrv" = 1 ]; then
    rv=$(sed -n 's/^rust-version *= *"\(.*\)"/\1/p' Cargo.toml)
    if [ -z "$rv" ]; then
      echo "--msrv: Cargo.toml sets no rust-version" >&2
      exit 1
    fi
    for row in "${ROWS[@]}"; do
      # shellcheck disable=SC2046
      run cargo "+$rv" check --locked --all-targets $(row_args "$row")
    done
    run cargo "+$rv" test --locked --all-features
  fi
fi

echo "verify ($mode): all checks passed; working-tree hash at start $start_hash"

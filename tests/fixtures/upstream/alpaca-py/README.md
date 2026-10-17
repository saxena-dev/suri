# Upstream response fixtures

These files are HTTP response bodies taken from the test suite of
alpaca-py, the Alpaca Python SDK:

- repository: https://github.com/alpacahq/alpaca-py
- commit: 6c15a94e92558b337620d31d6b891e40e85bfef4
- licence: Apache License 2.0 (the `LICENSE` file in this directory)

They were extracted from the test sources by `tools/fixtures/extract.py` without running
any upstream code, and are used as test data only.

Changes made to the upstream material:

- each response body a test serves is stored in its own file, byte for
  byte as the test defines it;
- bodies the tests give as Python values (`json=`) are serialised with
  Python's `json.dumps`;
- strings shaped like credentials are replaced by runs of `X` of the same
  length, and each such change is noted in `index.json`.

Nothing else is changed. `index.json` records where each file came from.

def test_t(reqmock):
    sym = "AAPL"
    reqmock.get("https://api.alpaca.markets/v2/x", json={"s": sym, "l": [sym]}, additional_matcher=lambda r: str(sym) or True)

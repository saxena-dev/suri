def test_t(reqmock):
    inner = {"a": 1}
    reqmock.get("https://api.alpaca.markets/v2/x", json={"i": inner})
    inner["a"] = 2

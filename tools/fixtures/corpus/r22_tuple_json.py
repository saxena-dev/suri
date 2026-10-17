
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", json={"a": (1, 2), "b": 1.5, "c": True})

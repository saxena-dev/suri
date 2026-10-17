
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", json=None)


def test_a(reqmock):
    body = "1"
    f = lambda: reqmock.get("https://api.alpaca.markets/v2/a", text=body)
    f()

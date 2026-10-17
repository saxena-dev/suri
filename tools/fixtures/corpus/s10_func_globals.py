
body = "module"
def test_a(reqmock):
    test_a.__globals__["body"] = "evil"
    reqmock.get("https://api.alpaca.markets/v2/a", text=body)

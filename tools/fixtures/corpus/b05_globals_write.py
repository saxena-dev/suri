body = '{"x": 1}'
def test_t(reqmock):
    globals()["body"] = '{"x": 2}'
    reqmock.get("https://api.alpaca.markets/v2/x", text=body)

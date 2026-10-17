__body__ = '{"x": 1}'
def test_t(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/x", text=__body__)

body = '{"x": 1}'
def test_t(reqmock):
    body = '{"x": 2}'
    def inner(r=reqmock.get("https://api.alpaca.markets/v2/x", text=body)):
        pass

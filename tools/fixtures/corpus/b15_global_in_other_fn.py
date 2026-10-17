body = '{"x": 1}'
def setup():
    global body
    body = '{"x": 2}'
setup()
def test_t(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/x", text=body)

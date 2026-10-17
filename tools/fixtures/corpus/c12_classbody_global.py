body = '{"x": 1}'
class K:
    global body
    body = '{"x": 2}'
def test_t(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/x", text=body)

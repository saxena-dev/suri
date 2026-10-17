a = '{"x": 1}'
def test_t(reqmock):
    b = a
    f(b)
    reqmock.get("https://api.alpaca.markets/v2/x", text=b)
def f(x): pass

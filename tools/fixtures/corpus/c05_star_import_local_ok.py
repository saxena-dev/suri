from string import *
def test_t(reqmock):
    digits = '{"x": 1}'
    reqmock.get("https://api.alpaca.markets/v2/x", text=digits)

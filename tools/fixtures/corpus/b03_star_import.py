digits = '{"x": 1}'
from string import *
def test_t(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/x", text=digits)

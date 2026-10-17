import sys
body = '{"x": 1}'
sys.modules[__name__].body = '{"x": 2}'
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text=body)

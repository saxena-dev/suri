import sys
body = "module"
me = sys.modules[__name__]
def test_a(reqmock):
    me.body = "changed"
    reqmock.get("https://api.alpaca.markets/v2/a", text=body)

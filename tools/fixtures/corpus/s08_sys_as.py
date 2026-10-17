
import sys as s
body = "module"
def test_a(reqmock):
    s.modules[__name__].body = "evil"
    reqmock.get("https://api.alpaca.markets/v2/a", text=body)

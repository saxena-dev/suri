
from sys import modules
body = "module"
def test_a(reqmock):
    modules[__name__].body = "evil"
    reqmock.get("https://api.alpaca.markets/v2/a", text=body)

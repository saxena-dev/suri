
from importlib import import_module
body = "module"
def test_a(reqmock):
    me = import_module(__name__).body = "evil"
    reqmock.get("https://api.alpaca.markets/v2/a", text=body)

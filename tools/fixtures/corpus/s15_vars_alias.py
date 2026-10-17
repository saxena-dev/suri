
g = globals
body = "module"
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text=body)

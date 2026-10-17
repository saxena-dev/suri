import sys
body = "module"
def test_a(reqmock):
    sys._getframe().f_globals["body"] = "changed"
    reqmock.get("https://api.alpaca.markets/v2/a", text=body)

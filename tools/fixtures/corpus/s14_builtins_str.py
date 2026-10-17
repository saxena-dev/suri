
import builtins
builtins.format = lambda v, s="": "evil"
x = "a"
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text=f"{x}")


x = "v"
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text=f"{x=}|{ x = !s:>4}|{x!a}")

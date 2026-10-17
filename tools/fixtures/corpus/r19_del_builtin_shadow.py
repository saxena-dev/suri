
str = lambda x: "evil"
x = 5
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text=f"{x!s}")

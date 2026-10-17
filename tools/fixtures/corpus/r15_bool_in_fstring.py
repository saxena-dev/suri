
v = True
n = -0.0
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text=f"{v}{n}{1e400}{10**3 if False else 5}")

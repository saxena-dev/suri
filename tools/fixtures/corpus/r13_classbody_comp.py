
body = "g"
def test_a(reqmock):
    class K:
        body = "c"
        r = [reqmock.get("https://api.alpaca.markets/v2/a", text=body) for _ in range(1)]

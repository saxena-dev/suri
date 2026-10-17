
def test_a(reqmock):
    body = "a"
    def h(b=reqmock.get("https://api.alpaca.markets/v2/a", text=body)):
        pass

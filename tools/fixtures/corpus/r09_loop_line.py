
body = "a"
def reg(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text=body)
def test_a(reqmock):
    global body
    reg(reqmock)
    body = "b"
    reg(reqmock)

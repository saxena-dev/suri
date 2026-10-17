
body = "a"
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text=body)
def test_b():
    global body
    body += "x"

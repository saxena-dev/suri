
def test_a(reqmock):
    body = "a"
    reqmock.get("https://api.alpaca.markets/v2/a", text=body, status_code=(body := 200))

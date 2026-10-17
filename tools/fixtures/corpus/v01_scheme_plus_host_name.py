def test_a(reqmock):
    host = "api.alpaca.markets"
    reqmock.get("https://" + host + "/v2/a", text="{}")

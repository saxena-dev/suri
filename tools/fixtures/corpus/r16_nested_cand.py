
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text="1", additional_matcher=reqmock.post("https://api.alpaca.markets/v2/b", text="2"))

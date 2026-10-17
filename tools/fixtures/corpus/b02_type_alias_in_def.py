def test_t(reqmock):
    sym = "AAPL"
    reqmock.get("https://api.alpaca.markets/v2/x", text=f'{{"s": "{sym}"}}')
    type sym = int

def test_t(reqmock):
    sym = "AAPL"
    type sym = int
    reqmock.get("https://api.alpaca.markets/v2/x", text=f'{{"s": "{sym}"}}')

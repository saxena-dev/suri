sym = "AAPL"
type sym = int
def test_t(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/x", text=f'{{"s": "{sym}"}}')

sym = "AAPL"
class TestC:
    type sym = int
    def test_t(self, reqmock):
        reqmock.get("https://api.alpaca.markets/v2/x", text=f'{{"s": "{sym}"}}')

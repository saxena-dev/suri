
body = "module"
class TestT:
    def test_a(self, reqmock):
        reqmock.get("https://api.alpaca.markets/v2/a", text=body)

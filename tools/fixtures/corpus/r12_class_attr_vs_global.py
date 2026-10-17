
body = "g"
class TestK:
    body = "c"
    def test_m(self, reqmock):
        reqmock.get("https://api.alpaca.markets/v2/a", text=body)

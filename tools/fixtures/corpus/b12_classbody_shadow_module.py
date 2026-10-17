body = '{"x": 1}'
class TestC:
    body = '{"x": 2}'
    def test_t(self, reqmock):
        reqmock.get("https://api.alpaca.markets/v2/x", text=body)

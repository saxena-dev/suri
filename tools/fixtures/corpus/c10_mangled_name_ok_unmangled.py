_TestC__body = '{"x": 1}'
class TestC:
    def test_t(self, reqmock):
        reqmock.get("https://api.alpaca.markets/v2/x", text=_TestC__body)

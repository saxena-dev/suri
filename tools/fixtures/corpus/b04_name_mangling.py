__body = '{"x": 1}'
_TestC__body = '{"x": 2}'
class TestC:
    def test_t(self, reqmock):
        reqmock.get("https://api.alpaca.markets/v2/x", text=__body)

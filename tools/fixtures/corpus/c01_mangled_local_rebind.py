class TestC:
    def test_t(self, reqmock):
        _TestC__body = '{"x": 1}'
        __body = '{"x": 2}'
        reqmock.get("https://api.alpaca.markets/v2/x", text=_TestC__body)

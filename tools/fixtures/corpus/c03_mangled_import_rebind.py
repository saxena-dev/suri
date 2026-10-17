class TestC:
    def test_t(self, reqmock):
        _TestC__body = '{"x": 1}'
        from string import digits as __body
        reqmock.get("https://api.alpaca.markets/v2/x", text=_TestC__body)

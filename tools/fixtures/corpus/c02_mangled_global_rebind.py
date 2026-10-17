_TestC__body = '{"x": 1}'
class TestC:
    def test_t(self, reqmock):
        global __body
        __body = '{"x": 2}'
        reqmock.get("https://api.alpaca.markets/v2/x", text=_TestC__body)

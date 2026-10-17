__class__ = '{"x": 1}'
class TestC:
    def test_t(self, reqmock):
        reqmock.get("https://api.alpaca.markets/v2/x", text=f"{__class__}")

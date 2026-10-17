def __helper(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text="{}")
def _TestC__helper(m):
    m.get("https://api.alpaca.markets/v2/b", text='{"hidden": 1}')
class TestC:
    def test_t(self, reqmock):
        __helper(reqmock)

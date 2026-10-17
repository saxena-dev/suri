_K__v = '{"x": 1}'
class _K:
    def m(self):
        global __v
        __v = '{"x": 2}'
def test_t(reqmock):
    _K().m()
    reqmock.get("https://api.alpaca.markets/v2/x", text=_K__v)

def test_t(reqmock):
    _K__v = '{"x": 1}'
    class K:
        nonlocal_dummy = 0
        def m(self):
            nonlocal __v
            __v = '{"x": 2}'
    K().m()
    reqmock.get("https://api.alpaca.markets/v2/x", text=_K__v)

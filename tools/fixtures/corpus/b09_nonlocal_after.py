def test_t(reqmock):
    body = '{"x": 1}'
    def change():
        nonlocal body
        body = '{"x": 2}'
    reqmock.get("https://api.alpaca.markets/v2/x", text=body)
    change()

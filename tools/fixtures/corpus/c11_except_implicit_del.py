def test_t(reqmock):
    body = '{"x": 1}'
    try:
        raise ValueError
    except ValueError as body:
        pass
    reqmock.get("https://api.alpaca.markets/v2/x", text=body)

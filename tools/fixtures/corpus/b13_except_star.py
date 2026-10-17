body = '{"x": 1}'
def test_t(reqmock):
    try:
        raise ExceptionGroup("g", [ValueError('{"x": 2}')])
    except* ValueError as body:
        pass
    reqmock.get("https://api.alpaca.markets/v2/x", text=body)

body = '{"x": 1}'
match ['{"x": 2}']:
    case [*body]:
        pass
def test_t(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/x", text=f"{body}")

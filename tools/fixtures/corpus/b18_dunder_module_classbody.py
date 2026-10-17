__module__ = '{"x": 1}'
class TestC:
    reqmock.get("https://api.alpaca.markets/v2/x", text=__module__)

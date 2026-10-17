import json as _json
class json:
    @staticmethod
    def dumps(v, **k):
        return _json.dumps(v, separators=(",", ":"), **k)
def test_t(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/x", text=json.dumps({"a": 1, "b": [1, 2]}))

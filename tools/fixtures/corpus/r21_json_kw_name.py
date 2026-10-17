import json
indent = 2
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text=json.dumps({"a": 1}, indent=4))

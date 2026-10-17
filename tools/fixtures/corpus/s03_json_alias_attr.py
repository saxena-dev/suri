
import json
m = json
def test_a(reqmock):
    m.dumps = lambda *a, **k: "evil"
    reqmock.get("https://api.alpaca.markets/v2/a", text=json.dumps({"a": 1}))

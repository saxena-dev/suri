
import json
import json as j2
j2.dumps = lambda *a, **k: "evil"
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text=json.dumps({"a": 1}))

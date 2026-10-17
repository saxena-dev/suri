
import json
m = json
setattr(m, "dumps", lambda *a, **k: "evil")
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text=json.dumps({"a": 1}))

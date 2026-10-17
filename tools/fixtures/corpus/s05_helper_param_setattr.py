
import json
def patch(mod):
    setattr(mod, "dumps", lambda *a, **k: "evil")
patch(json)
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text=json.dumps({"a": 1}))

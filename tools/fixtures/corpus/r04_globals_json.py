import json
import types
globals()["json"] = types.SimpleNamespace(dumps=lambda v: "evil")
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text=json.dumps({"a": 1}))

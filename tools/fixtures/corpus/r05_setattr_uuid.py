import sys
from uuid import UUID
setattr(sys.modules[__name__], "UUID", lambda s: "evil")
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text=f"{UUID('2a87c089-ffb6-472b-a4a3-cd9305c8605d')}")

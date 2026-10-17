
from uuid import UUID
UUID.__str__ = lambda self: "evil"
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text=f"{UUID('2a87c088-ffb6-472b-a4a3-cd9305c8605c')}")

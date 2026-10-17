import enum
class E(str, enum.Enum):
    A = "x"
body = "a" + "b"
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text=body + f"{body}")


BASE = "https://api.alpaca.markets"
def test_a(reqmock):
    reqmock.get(f"{BASE!r}/v2/a", text="{}")

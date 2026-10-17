
BASE = "https://api.alpaca.markets.evil.example"
def test_a(reqmock):
    reqmock.get(f"{BASE:.26}/v2/a", text="{}")

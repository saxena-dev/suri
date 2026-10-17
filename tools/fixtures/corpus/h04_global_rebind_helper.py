def helper(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text="{}")
def _other(m):
    m.get("https://api.alpaca.markets/v2/b", text="{}")
def setup_module():
    global helper
    helper = _other
def test_a(reqmock):
    helper(reqmock)

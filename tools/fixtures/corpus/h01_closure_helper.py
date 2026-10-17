def helper(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text="{}")
def test_a(reqmock):
    def helper(m):
        m.get("https://api.alpaca.markets/v2/b", text="{}")
    def inner():
        helper(reqmock)
    inner()

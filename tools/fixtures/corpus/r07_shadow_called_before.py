
def test_a(reqmock):
    def helper():
        reqmock.get("https://api.alpaca.markets/v2/a", text="1")
    helper()
    def helper():
        reqmock.get("https://api.alpaca.markets/v2/a", text="2")
    helper()

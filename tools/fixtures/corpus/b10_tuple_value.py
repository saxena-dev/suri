def test_t(reqmock):
    t = ("a", "b")
    reqmock.get("https://api.alpaca.markets/v2/x", text=f'{{"t": "{t}"}}')

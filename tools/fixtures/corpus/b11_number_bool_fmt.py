px = 1.10
n = 10**0
q = 3
def test_t(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/x", text=f'{{"p": {px}, "q": {q:03d}, "r": "{q!r}"}}')

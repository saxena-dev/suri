import functools
def deco(f):
    @functools.wraps(f)
    def w(reqmock):
        reqmock.get("https://api.alpaca.markets/v2/b", text="w")
        return f(reqmock)
    return w
@deco
def helper(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text="1")
def test_a(reqmock):
    helper(reqmock)

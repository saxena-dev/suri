import contextlib
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text="1")
with contextlib.nullcontext(None) as test_a:
    pass

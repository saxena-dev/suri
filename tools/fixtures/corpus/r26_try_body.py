
def test_a(reqmock):
    body = "a"
    try:
        reqmock.get("https://api.alpaca.markets/v2/a", text=body)
    finally:
        pass

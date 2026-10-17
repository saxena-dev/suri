
CODE = 201
H = "text/plain"
def test_a(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/a", text="x", status_code=CODE, headers={"X-Kind": H})

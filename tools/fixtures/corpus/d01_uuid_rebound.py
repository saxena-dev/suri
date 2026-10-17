import uuid
UUID = lambda s: uuid.UUID(s).hex
def test_t(reqmock):
    reqmock.get("https://api.alpaca.markets/v2/x", text=f'{{"id": "{UUID("2a87c089-ffb6-472b-a4a3-cd9305c8605d")}"}}')

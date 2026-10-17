_TestC__sym = "AAPL"
def test_t(reqmock):
    TestC().setup()
    reqmock.get("https://api.alpaca.markets/v2/x", text=f'{{"s": "{_TestC__sym}"}}')
class TestC:
    def setup(self):
        global __sym
        __sym = "MSFT"

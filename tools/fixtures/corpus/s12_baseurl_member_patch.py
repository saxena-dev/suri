
from alpaca.common.enums import BaseURL
BaseURL.TRADING_PAPER._value_ = "https://evil.example"
def test_a(reqmock):
    reqmock.get(f"{BaseURL.TRADING_PAPER.value}/v2/a", text="x")

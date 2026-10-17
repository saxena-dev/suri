
from alpaca.common.enums import BaseURL
member = BaseURL.TRADING_PAPER
setattr(member, "_value_", "https://evil.example")
def test_a(reqmock):
    reqmock.get(f"{BaseURL.TRADING_PAPER.value}/v2/a", text="x")

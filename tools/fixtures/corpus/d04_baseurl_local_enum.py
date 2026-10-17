from enum import Enum
class BaseURL(Enum):
    BROKER_SANDBOX = "http://localhost:8080"
def test_t(reqmock):
    reqmock.get(f"{BaseURL.BROKER_SANDBOX.value}/v1/accounts", text="[]")

import requests_mock
@requests_mock.Mocker()
class TestT(requests_mock.Mocker):
    def test_x(self, m):
        pass
f = lambda reqmock: reqmock.get("https://api.alpaca.markets/v2/a", text="{}")

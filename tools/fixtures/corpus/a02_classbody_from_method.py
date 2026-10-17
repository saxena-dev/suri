body = '{"x": 1}'
class TestA:
    body = '{"x": 2}'
    def test_m(self, reqmock):
        reqmock.get("https://h/a", text=body)

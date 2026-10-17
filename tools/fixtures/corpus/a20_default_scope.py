body = '{"x": 1}'
def test_s(reqmock):
    def inner(cb=reqmock.get("https://h/a", text=body)):
        body = '{"x": 2}'

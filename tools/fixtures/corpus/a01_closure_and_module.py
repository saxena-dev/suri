body = '{"x": 1}'
def test_closure(reqmock):
    body = '{"x": 2}'
    def inner():
        reqmock.get("https://h/a", text=body)
    inner()

body = '{"x": 1}'
f = lambda reqmock: reqmock.get("https://h/a", text=body)
def test_f(reqmock):
    f(reqmock)

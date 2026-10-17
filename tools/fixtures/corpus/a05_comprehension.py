u = '{"x": 1}'
def test_c(reqmock):
    [reqmock.get("https://h/a", text=u) for u in ['{"x": 2}']]
def test_d(reqmock):
    v = '{"x": 1}'
    [reqmock.get("https://h/b", text=v) for v in ['{"x": 3}']]

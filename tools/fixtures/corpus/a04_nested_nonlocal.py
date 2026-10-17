def test_n(reqmock):
    body = '{"x": 1}'
    def h():
        nonlocal body
        body = '{"x": 2}'
    h()
    reqmock.get("https://h/a", text=body)

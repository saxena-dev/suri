def test_w(reqmock):
    body = '{"x": 1}'
    [(body := '{"x": 2}') for _ in [0]]
    reqmock.get("https://h/a", text=body)

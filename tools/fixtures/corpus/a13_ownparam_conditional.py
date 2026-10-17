def test_p(reqmock, body='{"x": 2}'):
    if False:
        body = '{"x": 1}'
    reqmock.get("https://h/a", text=body)

def test_m(reqmock):
    body = '{"x": 1}'
    match '{"x": 2}':
        case body:
            pass
    reqmock.get("https://h/a", text=body)

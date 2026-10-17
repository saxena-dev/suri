body = '{"x": 1}'
class C:
    body = '{"x": 2}'
    reqmock.get("https://h/a", text=body)

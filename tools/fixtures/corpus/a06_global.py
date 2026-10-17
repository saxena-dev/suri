body = '{"x": 1}'
def setter():
    global body
    body = '{"x": 2}'
def test_g(reqmock):
    setter()
    reqmock.get("https://h/a", text=body)

body = '{"x": 1}'
def body(request, context):
    return '{"x": 2}'
def test_d(reqmock):
    reqmock.get("https://h/a", text=body)

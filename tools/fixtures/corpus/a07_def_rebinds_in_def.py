def test_d(reqmock):
    body = '{"x": 1}'
    def body(request, context):
        return '{"x": 2}'
    reqmock.get("https://h/a", text=body)

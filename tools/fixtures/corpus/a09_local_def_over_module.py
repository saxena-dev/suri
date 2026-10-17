body = '{"x": 1}'
def test_d(reqmock):
    def body(request, context):
        return '{"x": 2}'
    reqmock.get("https://h/a", text=body)

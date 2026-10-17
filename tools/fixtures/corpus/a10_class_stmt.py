body = '{"x": 1}'
class body:
    def __new__(cls, request, context):
        return '{"x": 2}'
def test_d(reqmock):
    reqmock.get("https://h/a", text=body)

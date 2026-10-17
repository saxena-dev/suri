body = '{"x": 1}'
def test_x(reqmock):
    def h():
        global body
        del body
    reqmock.get("https://h/a", text=body)

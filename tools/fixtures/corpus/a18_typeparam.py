body = '{"x": 1}'
def test_t[body](reqmock):
    reqmock.get("https://h/a", text=body)

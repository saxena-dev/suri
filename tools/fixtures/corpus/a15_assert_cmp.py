def mutate(b):
    b["x"] = 2
    return b
def test_a(reqmock):
    body = {"x": 1}
    reqmock.get("https://h/a", json=body)
    assert body == {"x": 1}
def test_b(reqmock):
    body = {"x": 1}
    reqmock.get("https://h/b", json=body)
    assert mutate(body) == {"x": 2}
def test_c(reqmock):
    body = {"x": 1}
    reqmock.get("https://h/c", json=body)
    assert body == mutate(body)

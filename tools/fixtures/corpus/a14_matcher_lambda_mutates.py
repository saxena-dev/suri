def test_l(reqmock):
    body = {"x": 1}
    reqmock.get("https://h/a", json=body, additional_matcher=lambda r: body.clear() or True)

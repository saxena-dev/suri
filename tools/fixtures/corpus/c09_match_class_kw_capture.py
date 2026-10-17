body = '{"x": 1}'
class P:
    __match_args__ = ("a",)
    def __init__(self): self.a = '{"x": 2}'
def test_t(reqmock):
    global_holder = 0
    match P():
        case P(a=body):
            pass
    reqmock.get("https://api.alpaca.markets/v2/x", text=body)

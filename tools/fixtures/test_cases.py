"""The extractor's edge cases, one test per case (rows needing a whole
upstream repository are in test_cli.py)."""

import json
import unittest

from test_support import analyse, entries, only, records


class EdgeCases(unittest.TestCase):

    def test_fstring_body_with_resolved_names_uuid_and_escaped_braces(self):
        e = only('''
            from uuid import UUID
            def test_a(reqmock):
                acct = UUID("2A87C088-FFB6-472B-A4A3-CD9305C8605C")
                sym = "AAPL"
                reqmock.get("https://api.alpaca.markets/v2/x", text=f"""{{"id": "{acct}", "s": "{sym}"}}""")
        ''')
        self.assertEqual(e.kind, "fixture")
        self.assertEqual(e.body, b'{"id": "2a87c088-ffb6-472b-a4a3-cd9305c8605c", "s": "AAPL"}')
        self.assertEqual(e.origin, "text")

    def test_fstring_body_over_a_parameter_skips(self):
        e = only('''
            def test_a(reqmock, sym):
                reqmock.get("https://api.alpaca.markets/v2/x", text=f"{sym}")
        ''')
        self.assertEqual((e.kind, e.reason, e.detail), ("skip", "body-not-literal", "sym"))

    def test_fstring_body_over_a_loop_variable_and_a_call_skip(self):
        found = entries('''
            def test_a(reqmock):
                for s in ["A"]:
                    reqmock.get("https://api.alpaca.markets/v2/a", text=f"{s}")
                reqmock.get("https://api.alpaca.markets/v2/b", text=f"{len('x')}")
        ''')
        self.assertEqual([(e.reason, e.detail) for e in found], [("body-not-literal", "s"), ("body-not-literal", "Call")])

    def test_text_name_extracts_json_name_and_dict_valued_display_skip(self):
        found = entries('''
            def test_a(reqmock):
                fake_text = "plain"
                body = {"a": 1}
                sym = "AAPL"
                reqmock.get("https://api.alpaca.markets/v2/a", text=fake_text)
                reqmock.get("https://api.alpaca.markets/v2/b", json=body)
                reqmock.get("https://api.alpaca.markets/v2/c", json={"s": sym, "n": 2})
                reqmock.get("https://api.alpaca.markets/v2/d", json={"runs": [body]})
        ''')
        self.assertEqual([e.kind for e in found], ["fixture", "skip", "fixture", "skip"])
        self.assertEqual(found[0].body, b"plain")
        self.assertEqual((found[1].reason, found[1].detail), ("body-not-literal", "body"))
        self.assertEqual(json.loads(found[2].body), {"s": "AAPL", "n": 2})
        self.assertEqual(found[2].origin, "json-literal")
        self.assertEqual((found[3].reason, found[3].detail), ("body-not-literal", "body"))

    def test_body_from_a_call_skips(self):
        e = only('''
            def test_a(reqmock, model):
                reqmock.patch("https://api.alpaca.markets/v2/a", json=model.model_dump())
        ''')
        self.assertEqual((e.reason, e.detail), ("body-not-literal", "Call"))

    def test_response_lists_positional_and_keyword(self):
        found = entries('''
            def test_a(reqmock):
                one = "[1]"
                reqmock.get("https://api.alpaca.markets/v2/a", [{"text": one}, {"text": "[]", "status_code": 201}])
                reqmock.get("https://api.alpaca.markets/v2/b", response_list=[{"json": [2]}, {"text": f"{one}"}])
        ''')
        self.assertEqual([(e.template, e.ordinal, e.index) for e in found],
                         [("/v2/a", 0, 0), ("/v2/a", 0, 1), ("/v2/b", 0, 0), ("/v2/b", 0, 1)])
        self.assertEqual([e.body for e in found], [b"[1]", b"[]", b"[2]", b"[1]"])
        self.assertEqual(found[1].status, 201)

    def test_status_code_literal_and_resolved_name(self):
        found = entries('''
            def test_a(reqmock):
                code = 422
                reqmock.post("https://api.alpaca.markets/v2/a", status_code=207, text="[]")
                reqmock.post("https://api.alpaca.markets/v2/b", status_code=code, text="{}")
                reqmock.post("https://api.alpaca.markets/v2/c", text="{}")
        ''')
        self.assertEqual([e.status for e in found], [207, 422, 200])

    def test_status_code_unresolvable_skips(self):
        e = only('''
            def test_a(reqmock, code):
                reqmock.post("https://api.alpaca.markets/v2/a", status_code=code, text="{}")
        ''')
        self.assertEqual((e.reason, e.detail), ("status-not-literal", "code"))

    def test_no_body_is_empty_with_origin_none(self):
        found = records('''
            def test_a(reqmock):
                reqmock.delete("https://api.alpaca.markets/v2/a", status_code=204)
                reqmock.delete("https://api.alpaca.markets/v2/b")
        ''')
        self.assertEqual([(r["kind"], r["origin"], r["status"], r["bytes"]) for r in found],
                         [("empty", "none", 204, 0), ("empty", "none", 200, 0)])
        self.assertTrue(found[0]["file"].endswith(".empty"))

    def test_empty_text_is_empty_with_origin_text(self):
        (r,) = records('''
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", text="")
        ''')
        self.assertEqual((r["kind"], r["origin"], r["bytes"]), ("empty", "text", 0))

    def test_non_json_text_body_is_text(self):
        (r,) = records('''
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", text="<a href='x'>Moved</a>")
        ''')
        self.assertEqual(r["kind"], "text")
        self.assertTrue(r["file"].endswith(".txt"))

    def test_content_keyword_skips(self):
        e = only('''
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", content=b"{}")
        ''')
        self.assertEqual((e.reason, e.detail, e.template), ("unsupported-keyword", "content", "/v2/a"))

    def test_headers_display_is_recorded_and_a_dict_name_skips(self):
        found = entries('''
            def test_a(reqmock):
                where = "https://x.example/doc"
                hdrs = {"Location": "y"}
                reqmock.get("https://api.alpaca.markets/v2/a", status_code=301, headers={"Location": where}, text="")
                reqmock.get("https://api.alpaca.markets/v2/b", headers=hdrs, text="")
        ''')
        self.assertEqual(found[0].headers, {"Location": "https://x.example/doc"})
        self.assertEqual((found[1].reason, found[1].detail), ("headers-not-literal", "hdrs"))

    def test_additional_matcher_notes_function_and_lambda(self):
        found = records('''
            def match(request):
                return True
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", text="{}", additional_matcher=match)
                reqmock.get("https://api.alpaca.markets/v2/b", text="{}", additional_matcher=lambda r: True)
        ''')
        self.assertEqual([r["notes"] for r in found], [["request-matcher:match"], ["request-matcher:<lambda>"]])

    def test_unknown_keyword_and_text_with_json_skip(self):
        found = entries('''
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", exc=ValueError)
                reqmock.get("https://api.alpaca.markets/v2/b", text="{}", json={})
        ''')
        self.assertEqual([(e.reason, e.detail) for e in found],
                         [("unsupported-keyword", "exc"), ("unsupported-keyword", "json")])

    def test_register_uri_head_options_any_and_regex(self):
        found = entries('''
            import re
            import requests_mock
            def test_a(reqmock):
                reqmock.register_uri("GET", "https://api.alpaca.markets/v2/a", text="{}")
                reqmock.head("https://api.alpaca.markets/v2/b")
                reqmock.options("https://api.alpaca.markets/v2/c")
                reqmock.get(requests_mock.ANY, text="{}")
                reqmock.get(re.compile("x"), text="{}")
        ''')
        self.assertEqual([(e.reason, e.detail) for e in found],
                         [("unknown-pattern", "Attribute")] * 3
                         + [("url-not-literal", "Attribute"), ("url-not-literal", "Call")])
        self.assertEqual([e.template for e in found[3:]], [None, None])

    def test_same_path_twice_differing_in_the_query(self):
        found = entries('''
            def test_a(reqmock):
                token = "abc"
                reqmock.get(f"https://data.alpaca.markets/v1beta1/news?limit=50&page_token={token}", text="{}")
                reqmock.get("https://data.alpaca.markets/v1beta1/news?limit=50&page_token=MTcz", text="{}")
        ''')
        self.assertEqual([(e.template, e.ordinal, e.query) for e in found],
                         [("/v1beta1/news", 0, "limit=50&page_token={token}"),
                          ("/v1beta1/news", 1, "limit=50&page_token=MTcz")])

    def test_same_url_registered_twice(self):
        found = entries('''
            def test_a(reqmock):
                reqmock.post("https://api.alpaca.markets/v2/orders", text="{}")
                reqmock.post("https://api.alpaca.markets/v2/orders", text="[]")
        ''')
        self.assertEqual([(e.ordinal, e.body) for e in found], [(0, b"{}"), (1, b"[]")])

    def test_host_less_url_takes_family_from_the_package(self):
        e = only('''
            def test_a(reqmock):
                reqmock.get("/v1/accounts", text="[]")
        ''', path="tests/broker/broker_client/test_accounts_routes.py")
        self.assertEqual((e.family, e.template), ("broker", "/v1/accounts"))

    def test_query_string_with_interpolations_is_a_template(self):
        e = only('''
            def test_a(reqmock):
                sym = "AAPL"
                reqmock.get(f"https://data.alpaca.markets/v2/stocks/{sym}/bars?start={sym}&x=1", text="{}")
        ''')
        self.assertEqual((e.template, e.query), ("/v2/stocks/{sym}/bars", "start={sym}&x=1"))

    def test_shadowed_definition_is_excluded_and_branches_are_not(self):
        a = analyse('''
            import pytest
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/dead", text="{}")
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/live", text="{}")
            def test_b(reqmock, flag):
                if flag:
                    def inner():
                        reqmock.get("https://api.alpaca.markets/v2/one", text="{}")
                else:
                    def inner():
                        reqmock.get("https://api.alpaca.markets/v2/two", text="{}")
            @pytest.mark.skip
            def test_c(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/skipped", text="{}")
        ''')
        self.assertEqual(a.excluded, [("test_a", 3)])
        self.assertEqual([e.template for e in a.entries], ["/v2/live", "/v2/one", "/v2/two", "/v2/skipped"])
        self.assertEqual([e.ordinal for e in a.entries], [0, 0, 0, 0])

    def test_shadowing_counts_only_real_rebinding_in_module_and_class_blocks(self):
        a = analyse('''
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", text="{}")
            test_a.slow = True
            REG = {}
            REG[test_a] = 1
            def test_b(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/b", text="{}")
            del test_b
            def test_c(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/c", text="{}")
            with open("x") as test_c:
                pass
            def test_d(reqmock):
                def helper():
                    reqmock.get("https://api.alpaca.markets/v2/d1", text="{}")
                helper()
                def helper():
                    reqmock.get("https://api.alpaca.markets/v2/d2", text="{}")
        ''')
        self.assertEqual(a.excluded, [("test_b", 7), ("test_c", 10)])
        self.assertEqual([e.template for e in a.entries], ["/v2/a", "/v2/d1", "/v2/d2"])

    def test_json_none_skips(self):
        e = only('''
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", json=None)
        ''')
        self.assertEqual((e.reason, e.detail), ("body-not-json", "None"))

    def test_text_with_a_content_type_skips_and_text_is_utf8(self):
        found = entries('''
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", text="caf\u00e9", headers={"Content-Type": "text/plain"})
                reqmock.get("https://api.alpaca.markets/v2/b", text="plain", headers={"content-type": "text/csv"})
                reqmock.get("https://api.alpaca.markets/v2/c", text="caf\u00e9")
        ''')
        self.assertEqual([(e.kind, e.reason, e.detail) for e in found],
                         [("skip", "body-not-literal", "charset"), ("skip", "body-not-literal", "charset"),
                          ("fixture", None, None)])
        self.assertEqual(found[2].body, "caf\u00e9".encode("utf-8"))

    def test_json_literal_bytes_are_json_dumps_with_defaults(self):
        e = only('''
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", json={"a": 1, "b": [1, 2], "c": "\u00e9"})
        ''')
        self.assertEqual(e.body, b'{"a": 1, "b": [1, 2], "c": "\\u00e9"}')

    def test_duplicate_test_names_in_classes_have_distinct_ids(self):
        found = entries('''
            class TestA:
                def test_x(self, reqmock):
                    reqmock.get("https://api.alpaca.markets/v2/a", text="{}")
            class TestB:
                def test_x(self, reqmock):
                    reqmock.get("https://api.alpaca.markets/v2/a", text="{}")
        ''')
        self.assertEqual([e.qualname for e in found], ["TestA.test_x", "TestB.test_x"])

    def test_parametrize_extracts_once_and_a_used_parameter_skips(self):
        found = entries('''
            import pytest
            @pytest.mark.parametrize("sym", ["A", "B"])
            def test_a(reqmock, sym):
                reqmock.get("https://api.alpaca.markets/v2/a", text="{}")
                reqmock.get(f"https://api.alpaca.markets/v2/b/{sym}", text=f"{sym}")
        ''')
        self.assertEqual([(e.kind, e.reason) for e in found], [("fixture", None), ("skip", "body-not-literal")])

    def test_module_level_helper_called_with_reqmock(self):
        found = entries('''
            def setup_mock(reqmock):
                first = "[1]"
                reqmock.get("https://api.alpaca.markets/v2/a", [{"text": first}, {"text": "[]"}])
            def test_a(reqmock):
                setup_mock(reqmock)
            def test_b(reqmock):
                setup_mock(reqmock=reqmock)
        ''')
        self.assertEqual([(e.kind, e.qualname, e.index) for e in found],
                         [("fixture", "setup_mock", 0), ("fixture", "setup_mock", 1)])

    def test_stream_payloads_without_reqmock_are_not_candidates(self):
        self.assertEqual(entries('''
            import msgpack
            def test_a(ws):
                ws.send(msgpack.packb([{"T": "t", "p": 1.5}]))
        '''), [])

    def test_unknown_node_type_skips(self):
        e = only('''
            def test_a(reqmock):
                sym = "A"
                reqmock.get("https://api.alpaca.markets/v2/a", text=t"{sym}")
        ''')
        self.assertEqual((e.reason, e.detail), ("body-not-literal", "TemplateStr"))

    def test_json_values_that_are_not_json_skip(self):
        found = entries('''
            from uuid import UUID
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", json={1, 2})
                reqmock.get("https://api.alpaca.markets/v2/b", json={"a": b"x"})
                reqmock.get("https://api.alpaca.markets/v2/c", json={"id": UUID("2a87c088-ffb6-472b-a4a3-cd9305c8605c")})
                reqmock.get("https://api.alpaca.markets/v2/d", json={1: "x"})
        ''')
        self.assertEqual([(e.reason, e.detail) for e in found],
                         [("body-not-json", "set"), ("body-not-json", "bytes"),
                          ("body-not-json", "UUID"), ("body-not-json", "key")])

    def test_csv_body_is_text(self):
        (r,) = records('''
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", text="symbol,qty\\nAAPL,1\\n")
        ''')
        self.assertEqual(r["kind"], "text")

    def test_watched_names_outside_permitted_positions_are_one_skip_each(self):
        found = entries('''
            def test_a(reqmock):
                m = reqmock
                reqmock.head("https://api.alpaca.markets/v2/a")
        ''')
        self.assertEqual([(e.reason, e.detail, e.method, e.ordinal) for e in found],
                         [("unknown-pattern", "Assign", "UNKNOWN", 0), ("unknown-pattern", "Attribute", "UNKNOWN", 1)])

    def test_duplicate_key_and_json_constants_are_noted_with_bytes_unchanged(self):
        found = records('''
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", text='{"currency": "USD", "currency": "EUR"}')
                reqmock.get("https://api.alpaca.markets/v2/b", text='{"x": NaN, "y": -Infinity}')
        ''')
        self.assertEqual(found[0]["notes"], ["duplicate-key:currency"])
        self.assertEqual(found[0]["bytes"], len('{"currency": "USD", "currency": "EUR"}'))
        self.assertEqual(found[1]["notes"], ["json-constant:-Infinity", "json-constant:NaN"])

    def test_crlf_inside_a_triple_quoted_literal_reads_as_python_does(self):
        source = 'def test_a(reqmock):\n    reqmock.get("https://api.alpaca.markets/v2/a", text="""{\r\n}""")\n'
        e = only(source)
        self.assertEqual(e.body, b"{\n}")


if __name__ == "__main__":
    unittest.main()

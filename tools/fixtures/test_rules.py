"""URL templates, families, file names, watched names, name resolution,
ordinals and qualnames."""

import unittest

from fxoutput import redact, segments_ok, slug
from test_support import analyse, entries, only, records

URL = "https://api.alpaca.markets/v2/x"


class Urls(unittest.TestCase):

    def test_both_documented_examples(self):
        found = entries('''
            from alpaca.common.enums import BaseURL
            def test_a(reqmock):
                account_id = "a"
                asset_id = "b"
                document_id = "c"
                fake_dl_url = f"https://fake-dl.com/{document_id}"
                reqmock.get(f"{BaseURL.BROKER_SANDBOX.value}/v1/trading/accounts/{account_id}/positions/{asset_id}", text="{}")
                reqmock.get(fake_dl_url, text="x")
        ''', path="tests/broker/broker_client/test_trading_routes.py")
        self.assertEqual([(e.template, e.family) for e in found],
                         [("/v1/trading/accounts/{account_id}/positions/{asset_id}", "broker"),
                          ("/{document_id}", "other")])

    def test_baseurl_plus_literal_is_one_interpolation_then_text(self):
        e = only('''
            from alpaca.common.enums import BaseURL
            def test_a(reqmock):
                reqmock.get(BaseURL.BROKER_SANDBOX.value + "/v1/accounts/activities", text="[]")
        ''')
        self.assertEqual((e.template, e.family), ("/v1/accounts/activities", "broker"))

    def test_evaluated_base_with_a_path_or_another_scheme_skips(self):
        found = entries('''
            from alpaca.common.enums import BaseURL
            def test_a(reqmock):
                base = "https://api.alpaca.markets/v2"
                reqmock.get(f"{base}/orders", text="{}")
                reqmock.get(f"{BaseURL.TRADING_STREAM_PAPER.value}/x", text="{}")
                reqmock.get("wss://stream.data.alpaca.markets/v2/iex", text="{}")
        ''')
        self.assertEqual([(e.reason, e.template) for e in found], [("url-not-literal", None)] * 3)

    def test_url_name_is_expanded_not_evaluated(self):
        e = only('''
            def test_a(reqmock, order_id):
                url = "https://paper-api.alpaca.markets/v2/orders/" + order_id
                reqmock.get(url, text="{}")
        ''')
        self.assertEqual((e.template, e.family), ("/v2/orders/{order_id}", "trading"))

    def test_interpolation_labels(self):
        e = only('''
            def test_a(reqmock, ids, obj):
                reqmock.get(f"https://data.alpaca.markets/v2/{obj.symbol}/{ids[0]}", text="{}")
        ''')
        self.assertEqual(e.template, "/v2/{symbol}/{}")

    def test_every_host_row(self):
        hosts = {
            "broker-api.alpaca.markets": "broker", "broker-api.sandbox.alpaca.markets": "broker",
            "api.alpaca.markets": "trading", "paper-api.alpaca.markets": "trading",
            "data.alpaca.markets": "data", "data.sandbox.alpaca.markets": "data",
            "example.com": "other",
        }
        for host, family in hosts.items():
            e = only(f'''
                def test_a(reqmock):
                    reqmock.get("https://{host}/v1/a", text="{{}}")
            ''')
            self.assertEqual(e.family, family, host)

    def test_host_less_family_per_package(self):
        for package, family in [("broker", "broker"), ("trading", "trading"), ("data", "data"), ("common", "other")]:
            e = only('''
                def test_a(reqmock):
                    reqmock.get("/v1/a", text="{}")
            ''', path=f"tests/{package}/test_x.py")
            self.assertEqual(e.family, family)


class FileNames(unittest.TestCase):

    def test_slug(self):
        self.assertEqual(slug("GET", "/v1/trading/accounts/{account_id}/positions/{asset_id}"),
                         "get-v1-trading-accounts-account-id-positions-asset-id")
        self.assertEqual(slug("GET", "/v2/orders:by_client_order_id"), "get-v2-orders-by-client-order-id")
        long = slug("GET", "/" + "a" * 77 + "/b")
        self.assertEqual(long, "get-" + "a" * 76)
        self.assertEqual(slug("GET", "/" + "a" * 75 + "/bbbb"), "get-" + "a" * 75)

    def test_file_names_are_lowercased_with_ordinal_and_index(self):
        found = records('''
            class TestCase:
                def test_X(self, reqmock):
                    reqmock.get("https://api.alpaca.markets/v2/A", [{"text": "{}"}])
        ''', path="tests/Trading/test_Routes.py")
        self.assertEqual(found[0]["file"], "trading/test_routes/testcase.test_x/get-v2-a-0-0.json")

    def test_segment_rule(self):
        self.assertTrue(segments_ok("broker/broker_client/test_x/testa.test_y"))
        self.assertFalse(segments_ok("trading/<module>"))
        self.assertFalse(segments_ok("trading/caf\u00e9"))


class WatchedNames(unittest.TestCase):

    def assertUnknown(self, source, details, path="tests/trading/test_cases.py"):
        found = entries(source, path)
        self.assertEqual([(e.reason, e.detail) for e in found if e.reason == "unknown-pattern"],
                         [("unknown-pattern", d) for d in details])

    def test_permitted_positions_give_no_skip(self):
        self.assertUnknown('''
            import requests_mock
            from requests_mock import Mocker
            def helper(reqmock):
                pass
            def test_a(reqmock: Mocker):
                helper(reqmock)
                helper(reqmock=reqmock)
                assert reqmock.called and reqmock.called_once and reqmock.call_count == 1
                assert reqmock.request_history and reqmock.last_request
        ''', [])

    def test_unknown_names_parameters_and_imports(self):
        self.assertUnknown('''
            import requests_mock as rm
            import os, requests_mock.mocker
            from requests_mock import mock
            from requests_mock.mocker import Mocker
            from elsewhere import Mocker as M2, thing as reqmock
            def test_a(requests_mock):
                print(reqmock)
                with requests_mock.Mocker() as m:
                    pass
        ''', ["Import", "Import", "ImportFrom", "ImportFrom", "ImportFrom", "arg", "Call", "Attribute"])

    def test_requests_mock_in_conftest_only_in_the_fixture_call(self):
        self.assertUnknown('''
            import pytest
            import requests_mock
            @pytest.fixture
            def reqmock():
                with requests_mock.Mocker() as m:
                    yield m
        ''', [], path="tests/conftest.py")
        self.assertUnknown('''
            import requests_mock
            def test_a():
                with requests_mock.Mocker() as m:
                    pass
        ''', ["Attribute"])

    def test_helper_rules(self):
        self.assertUnknown('''
            def helper(m):
                m.get("https://api.alpaca.markets/v2/a", text="{}")
            def other(reqmock):
                pass
            other = 1
            def test_a(reqmock, *rest):
                helper(reqmock)
                other(reqmock)
                def local(reqmock):
                    pass
                local(reqmock)
                outer(*rest, reqmock)
            def outer(x, reqmock):
                pass
        ''', ["Call", "Call", "Call", "Call"])

    def test_local_def_shadowing_the_helper(self):
        self.assertUnknown('''
            def helper(reqmock):
                pass
            def test_a(reqmock):
                def helper(m):
                    pass
                helper(reqmock)
        ''', ["Call"])

    def test_unknown_skip_lines_and_details(self):
        found = entries('''
            import pytest, requests_mock as rm
            from requests_mock import mock
            def test_a(reqmock,
                       requests_mock=None):
                reqmock.called = True
                print(reqmock)
            @pytest.fixture
            def reqmock():
                pass
        ''')
        self.assertEqual([(e.line, e.detail, e.qualname) for e in found], [
            (2, "Import", "<module>"), (3, "ImportFrom", "<module>"), (5, "arg", "test_a"), (6, "Attribute", "test_a"),
            (7, "Call", "test_a"), (9, "FunctionDef", "<module>")])

    def test_argument_shapes(self):
        found = entries('''
            def test_a(reqmock, items):
                reqmock.get(url="https://api.alpaca.markets/v2/a", text="{}")
                reqmock.get("https://api.alpaca.markets/v2/b", items)
                reqmock.get("https://api.alpaca.markets/v2/c", [], 3)
                reqmock.get("https://api.alpaca.markets/v2/d", [{"text": "{}", "reason": "x"}, "y"])
                reqmock.get("https://api.alpaca.markets/v2/e", [{"text": "{}"}], text="{}")
        ''')
        self.assertEqual([(e.reason, e.detail, e.template, e.index) for e in found], [
            ("unsupported-keyword", "url", None, None),
            ("unsupported-keyword", "Name", "/v2/b", None),
            ("unsupported-keyword", "Constant", "/v2/c", None),
            ("unsupported-keyword", "reason", "/v2/d", 0),
            ("unsupported-keyword", "Constant", "/v2/d", 1),
            ("unsupported-keyword", "response_list", "/v2/e", None),
        ])

    def test_nested_candidate_is_a_candidate(self):
        found = entries('''
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", text=reqmock.post("https://api.alpaca.markets/v2/b", text="{}"))
        ''')
        self.assertEqual([(e.template, e.kind) for e in found], [("/v2/a", "skip"), ("/v2/b", "fixture")])

    def test_excluded_definition_is_not_classified(self):
        self.assertEqual(entries('''
            def test_a(reqmock):
                m = reqmock
            def test_a(reqmock):
                pass
        '''), [])


class Resolution(unittest.TestCase):

    def body(self, source):
        e = only(source)
        return e.body if e.kind == "fixture" else (e.reason, e.detail)

    def test_each_other_binding_unresolves(self):
        forms = [
            "x += 'b'", "x: str = 'b'", "for x in []: pass", "with open('f') as x: pass",
            "try:\n        pass\n    except Exception as x:\n        pass", "(x := 'b')", "import x",
            "global x", "del x", "def x(): pass", "class x: pass", "type x = int",
            "match 1:\n        case x:\n            pass", "def g[x](): pass", "[x for x in []]",
            "lambda x: x", "a, x = 1, 2", "a, *x = 1, 2",
        ]
        for form in forms:
            source = "def test_a(reqmock):\n    x = 'a'\n    " + form + \
                     "\n    reqmock.get('https://api.alpaca.markets/v2/a', text=f'{x}')\n"
            self.assertEqual(only(source).reason, "body-not-literal", form)

    def test_star_import_and_module_level_binding(self):
        self.assertEqual(self.body('''
            body = "module"
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", text=body)
        '''), b"module")
        self.assertEqual(self.body('''
            from x import *
            body = "module"
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", text=body)
        '''), ("body-not-literal", "body"))

    def test_dynamic_rebinding_makes_module_names_unresolvable(self):
        for call in ('globals()["body"] = "x"', 'setattr(this, "body", "x")', 'monkeypatch.setattr(this, "body", "x")',
                     'monkeypatch.setattr("pkg.mod.body", "x")', 'exec("body = 1")', 'sys.modules[__name__].body = "x"',
                     'setattr(importlib.import_module("m"), "body", "x")'):
            source = ('import sys\nimport importlib\nimport this\nbody = "module"\n'
                      'def test_a(reqmock, monkeypatch):\n    ' + call +
                      '\n    reqmock.get("https://api.alpaca.markets/v2/a", text=body)\n')
            self.assertEqual(only(source).reason, "body-not-literal", call)

    def test_every_dynamic_form(self):
        forms = ["vars()", "eval('1')", "__import__('m')", "import_module('m')", "from importlib import import_module",
                 "from sys import modules", "import sys as s\n    s.modules", "mock.patch('pkg.mod.body')",
                 "mock.patch.object(this, 'body')", "test_a.__globals__", "this.__dict__",
                 "import importlib\n    importlib.import_module('m').body = 'x'", "locals()",
                 "sys._getframe().f_globals", "sys._getframe().f_locals", "this.body = 'x'"]
        for form in forms:
            source = ('import this\nbody = "module"\ndef test_a(reqmock, mock):\n    ' + form +
                      '\n    reqmock.get("https://api.alpaca.markets/v2/a", text=body)\n')
            self.assertEqual(only(source).reason, "body-not-literal", form)

    def test_json_uuid_baseurl_recognised_only_when_every_use_is_a_read(self):
        cases = {
            "json.dumps = str": "import json",
            "m = json": "import json",
            "import json as j2": "import json",
            "UUID.__str__ = repr": "from uuid import UUID",
            "U = UUID\nU.__str__ = repr": "from uuid import UUID",
            "x = BaseURL.TRADING_PAPER": "from alpaca.common.enums import BaseURL",
        }
        for change, imp in cases.items():
            source = (imp + '\n' + change + '\n'
                      'def test_a(reqmock):\n    reqmock.get("https://api.alpaca.markets/v2/a", text=' +
                      {"json": 'json.dumps({"a": 1})', "UUID": 'f"{UUID(\'2a87c088-ffb6-472b-a4a3-cd9305c8605c\')}"',
                       "BaseURL": 'BaseURL.TRADING_PAPER.value'}[imp.split()[-1]] + ')\n')
            self.assertEqual(only(source).kind, "skip", change)

    def test_setattr_on_a_local_object_rebinds_no_module_name(self):
        e = only('''
            from alpaca.common.enums import BaseURL
            def test_a(reqmock, model, field, client):
                setattr(model, field, None)
                client.x = 1
                reqmock.get(f"{BaseURL.TRADING_PAPER.value}/v2/a", text="{}")
        ''')
        self.assertEqual((e.kind, e.family), ("fixture", "trading"))

    def test_dunder_names_never_resolve(self):
        self.assertEqual(self.body('''
            class C:
                __body = "a"
                def test_a(self, reqmock):
                    reqmock.get("https://api.alpaca.markets/v2/a", text=_C__body)
        '''), ("body-not-literal", "_C__body"))

    def test_closure_variable_and_nested_def_assignment_do_not_bind(self):
        self.assertEqual(self.body('''
            body = "module"
            def test_a(reqmock):
                body = "outer"
                def inner():
                    reqmock.get("https://api.alpaca.markets/v2/a", text=body)
                inner()
        '''), ("body-not-literal", "body"))
        self.assertEqual(self.body('''
            def test_a(reqmock):
                def setter():
                    body = "inner"
                reqmock.get("https://api.alpaca.markets/v2/a", text=body)
        '''), ("body-not-literal", "body"))

    def test_binding_after_the_use_does_not_bind(self):
        self.assertEqual(self.body('''
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", text=body)
                body = "late"
        '''), ("body-not-literal", "body"))

    def test_binding_must_precede_and_be_a_direct_statement(self):
        self.assertEqual(self.body('''
            def test_a(reqmock, flag):
                if flag:
                    body = "a"
                reqmock.get("https://api.alpaca.markets/v2/a", text=body)
        '''), ("body-not-literal", "body"))

    def test_dict_values_and_aliases_never_resolve(self):
        self.assertEqual(self.body('''
            def test_a(reqmock):
                a = {"k": 1}
                b = a
                reqmock.get("https://api.alpaca.markets/v2/a", json={"x": b})
        '''), ("body-not-literal", "a"))

    def test_uuid_conversions_and_json_dumps(self):
        found = entries('''
            import json
            from uuid import UUID
            def test_a(reqmock):
                u = UUID("2a87c088-ffb6-472b-a4a3-cd9305c8605c")
                reqmock.get("https://api.alpaca.markets/v2/a", text=f"{u!s}")
                reqmock.get("https://api.alpaca.markets/v2/b", text=f"{u!r}")
                reqmock.get("https://api.alpaca.markets/v2/c", text=json.dumps({"id": u}))
                reqmock.get("https://api.alpaca.markets/v2/d", text=json.dumps({"a": [1, 2]}, indent=1))
        ''')
        self.assertEqual(found[0].body, b"2a87c088-ffb6-472b-a4a3-cd9305c8605c")
        self.assertEqual([(e.reason, e.detail) for e in found[1:3]],
                         [("body-not-literal", "UUID"), ("body-not-json", "UUID")])
        self.assertEqual(found[3].body, b'{\n "a": [\n  1,\n  2\n ]\n}')

    def test_dynamic_rebinding_also_covers_uuid_json_and_baseurl(self):
        self.assertEqual(self.body('''
            import json
            globals()["json"] = None
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", text=json.dumps({"a": 1}))
        '''), ("body-not-literal", "Call"))

    def test_rebound_uuid_json_or_baseurl_are_not_recognised(self):
        self.assertEqual(self.body('''
            from uuid import UUID
            UUID = str
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", text=f"{UUID('2a87c088-ffb6-472b-a4a3-cd9305c8605c')}")
        '''), ("body-not-literal", "Call"))


class HostFormatting(unittest.TestCase):

    def test_host_uses_the_conversion_and_format_spec(self):
        found = entries('''
            def test_a(reqmock):
                base = "https://api.alpaca.markets.other.example"
                plain = "https://api.alpaca.markets"
                reqmock.get(f"{base:.26}/v2/a", text="{}")
                reqmock.get(f"{plain!r}/v2/b", text="{}")
        ''')
        self.assertEqual([(e.kind, e.family, e.host) for e in found],
                         [("fixture", "trading", "api.alpaca.markets"), ("skip", None, None)])


class Redaction(unittest.TestCase):

    def test_each_pattern(self):
        pem = "-----BEGIN RSA PRIVATE KEY-----\nMIIB\n-----END RSA PRIVATE KEY-----"
        jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2ln"
        text = "k=AKIAABCDEFGHIJKLMNOP t=ASIAABCDEFGHIJKLMNOP " + pem + " " + jwt + " Bearer abcdefghijklmnop"
        redacted, kinds = redact(text)
        self.assertEqual(kinds, {"aws-access-key-id", "private-key", "jwt", "bearer-token"})
        self.assertEqual(len(redacted), len(text))
        for secret in ("AKIA", "ASIA", "MIIB", "eyJ", "abcdefghijklmnop"):
            self.assertNotIn(secret, redacted)

    def test_identity_parts_are_redacted_before_ids_and_file_names(self):
        found = records('''
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/AKIAABCDEFGHIJKLMNOP/x?k=AKIAABCDEFGHIJKLMNOP", text="{}")
                reqmock.get("https://api.alpaca.markets/v2/AKIAABCDEFGHIJKLMNOP/y", AKIAABCDEFGHIJKLMNOP=1)
        ''')
        blob = repr(found)
        self.assertNotIn("AKIA", blob)
        self.assertNotIn("akia", blob)
        self.assertEqual(found[0]["path"], "/v2/" + "X" * 20 + "/x")
        self.assertEqual(found[0]["query"], "k=" + "X" * 20)
        self.assertEqual(found[0]["notes"], ["redacted:aws-access-key-id"])
        self.assertEqual(found[1]["detail"], "X" * 20)

    def test_notes_are_redacted(self):
        (found,) = records('''
            def AKIAABCDEFGHIJKLMNOP(r):
                return True
            def test_a(reqmock):
                reqmock.get("https://api.alpaca.markets/v2/a", text='{"AKIAABCDEFGHIJKLMNOP": 1, "AKIAABCDEFGHIJKLMNOP": 2}',
                            additional_matcher=AKIAABCDEFGHIJKLMNOP)
        ''')
        self.assertNotIn("AKIA", repr(found))
        self.assertEqual(found["notes"], ["duplicate-key:" + "X" * 20, "redacted:aws-access-key-id",
                                          "request-matcher:" + "X" * 20])


class Ordinals(unittest.TestCase):

    def test_ordinals_count_skips_in_pre_order(self):
        found = entries('''
            def test_a(reqmock, p):
                reqmock.get("https://api.alpaca.markets/v2/a", text=f"{p}")
                reqmock.get("https://api.alpaca.markets/v2/a", text="{}")
        ''')
        self.assertEqual([(e.kind, e.ordinal) for e in found], [("skip", 0), ("fixture", 1)])

    def test_fixing_a_skip_renames_nothing(self):
        before = entries('''
            def test_a(reqmock, p):
                reqmock.get("https://api.alpaca.markets/v2/a", text=f"{p}")
                reqmock.get("https://api.alpaca.markets/v2/a", text="[]")
        ''')
        after = entries('''
            def test_a(reqmock):
                p = "x"
                reqmock.get("https://api.alpaca.markets/v2/a", text=f"{p}")
                reqmock.get("https://api.alpaca.markets/v2/a", text="[]")
        ''')
        self.assertEqual([e.ordinal for e in before], [e.ordinal for e in after])
        self.assertEqual(before[1].body, after[1].body)


class Qualnames(unittest.TestCase):

    def test_parameters_decorators_defaults_bases_lambdas_comprehensions(self):
        a = analyse('''
            def deco(x):
                return x
            class Base:
                pass
            def test_a(reqmock):
                @deco(reqmock.get("https://api.alpaca.markets/v2/deco", text="{}"))
                def inner(m=reqmock.get("https://api.alpaca.markets/v2/default", text="{}")):
                    reqmock.get("https://api.alpaca.markets/v2/body", text="{}")
                class K(Base, x=reqmock.get("https://api.alpaca.markets/v2/base", text="{}")):
                    def m(self):
                        f = lambda: reqmock.get("https://api.alpaca.markets/v2/lambda", text="{}")
                        [reqmock.get("https://api.alpaca.markets/v2/comp", text="{}") for _ in range(1)]
        ''')
        quals = {e.template: e.qualname for e in a.entries}
        self.assertEqual(quals, {
            "/v2/deco": "test_a", "/v2/default": "test_a", "/v2/body": "test_a.inner",
            "/v2/base": "test_a", "/v2/lambda": "test_a.K.m", "/v2/comp": "test_a.K.m",
        })


if __name__ == "__main__":
    unittest.main()

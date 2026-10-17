"""The differential test: every module in corpus/ is analysed by the
extractor and also run by CPython against a fake ``reqmock`` that records
each registration and works out what requests-mock would serve. The
extractor must give the value CPython serves, or skip.

Each value is compared as (body, status, headers) together with the URL's
host. Rules: a value that matches no CPython registration on the same line
fails; a CPython registration on a line where the extractor has no entry
fails unless the module has an unknown-pattern skip (the extractor reported
what it could not follow). An extracted value on a line CPython never
registered (code the harness did not call) is reported, not failed.
"""

import enum
import json
import os
import subprocess
import sys
import types
import unittest
import urllib.parse
from json import dumps as _dumps, loads as _loads  # held before any corpus module can patch json
from unittest import mock

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # also run as a child with -I

from fxcandidates import Analysis  # noqa: E402
from test_support import BASEURL, HERE  # noqa: E402

CORPUS = os.path.join(HERE, "corpus")
READS = {"request_history": [], "called": False, "called_once": False, "call_count": 0, "last_request": None}


class FakeMocker:
    def __init__(self):
        self.registrations = []

    def __getattr__(self, name):
        if name in READS:
            return READS[name]

        def register(*args, **kwargs):
            self.registrations.append((sys._getframe(1).f_lineno, name, args, kwargs))
            return types.SimpleNamespace()
        return register


class _Request:
    qs, text, headers = {}, "", {}

    def json(self):
        return {}


def served_one(fields):
    """What requests-mock serves for one registered response. A request
    matcher is applied first, for its side effects, but its answer is not
    used: whether it accepts depends on the real request, and the extractor
    records it as a note instead."""
    matcher = fields.get("additional_matcher")
    if callable(matcher):
        try:
            matcher(_Request())  # run for its side effects; its answer depends on the real request
        except Exception:
            pass
    meta = (fields.get("status_code", 200), dict(fields.get("headers") or {}))
    if "text" in fields:
        text = fields["text"]
        return ("text", text(_Request(), types.SimpleNamespace()) if callable(text) else text) + meta
    if fields.get("json") is not None:
        return ("json", _loads(_dumps(fields["json"]))) + meta
    return ("none", None) + meta


def served(args, kwargs):
    items = args[1] if len(args) > 1 and isinstance(args[1], list) else kwargs.get("response_list")
    try:
        if items is not None:
            matcher = {k: v for k, v in kwargs.items() if k == "additional_matcher"}
            return [served_one(dict(item, **matcher)) for item in items]
        return [served_one(kwargs)]
    except Exception as e:
        return [("ERR", repr(e))]


def fake_modules():
    def marker(*args, **kwargs):
        if len(args) == 1 and callable(args[0]) and not kwargs:
            return args[0]
        return lambda f: f

    class Marks:
        def __getattr__(self, name):
            return marker

    pytest = types.ModuleType("pytest")
    pytest.fixture = marker
    pytest.mark = Marks()
    rm = types.ModuleType("requests_mock")
    rm.Mocker = rm.mock = type("Mocker", (), {})
    rm.ANY = object()
    enums = types.ModuleType("alpaca.common.enums")
    enums.BaseURL = enum.Enum("BaseURL", dict(BASEURL), type=str)
    return {"pytest": pytest, "requests_mock": rm, "requests_mock.mocker": rm,
            "alpaca": types.ModuleType("alpaca"), "alpaca.common": types.ModuleType("alpaca.common"),
            "alpaca.common.enums": enums}


def host_of(args, kwargs):
    url = args[0] if args else kwargs.get("url")
    return urllib.parse.urlsplit(url).netloc if isinstance(url, str) else None


def run_with_cpython(path, source):
    """{line: [(method, host, [served values])]} for every registration the
    module makes. Each module runs in its own interpreter, so a module that
    patches json, uuid or builtins cannot affect any other test."""
    proc = subprocess.run([sys.executable, "-I", os.path.abspath(__file__), "--run", path],
                          input=source, capture_output=True, text=True, timeout=60)
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr)
    return {int(line): [tuple(r) for r in regs] for line, regs in _loads(proc.stdout).items()}


def _run_here(path, source):
    fake = FakeMocker()
    module = types.ModuleType("corpus_module")
    module.reqmock = fake
    namespace = module.__dict__
    with mock.patch.dict(sys.modules, dict(fake_modules(), corpus_module=module)):
        try:
            exec(compile(source, path, "exec"), namespace)
            for name, value in list(namespace.items()):
                if name.startswith("test_") and callable(value):
                    _call(value, fake)
                elif name.startswith("Test") and isinstance(value, type):
                    for attr in dir(value):
                        if attr.startswith("test_"):
                            _call(getattr(value(), attr), fake)
        except Exception:
            pass
    result = {}
    for line, name, args, kwargs in fake.registrations:
        result.setdefault(line, []).append((name.upper(), host_of(args, kwargs), served(args, kwargs)))
    return _plain(result)


def _plain(value):
    """A JSON round trip, so values compare the same on both sides."""
    return _loads(_dumps(value, default=repr))


def _call(function, fake):
    try:
        function(fake)
    except Exception:
        pass


def tool_value(entry):
    meta = (entry.status, dict(entry.headers))
    if entry.origin == "text":
        return ("text", entry.body.decode("utf-8")) + meta
    if entry.origin == "json-literal":
        return ("json", json.loads(entry.body)) + meta
    return ("none", None) + meta


def compare(entries, python):
    """(problems, unexercised lines) for one module."""
    problems, unexercised = [], []
    lines = {e.line for e in entries}
    unknown = any(e.reason == "unknown-pattern" for e in entries)
    for e in entries:
        if e.kind != "fixture":
            continue
        if e.line not in python:
            unexercised.append(e.line)
            continue
        want = _plain([e.host or "", tool_value(e)])
        seen = [_plain([host or "", values[e.index or 0] if (e.index or 0) < len(values) else ["NOT-SERVED"]])
                for method, host, values in python[e.line] if method == e.method]
        if not seen or any(s != want for s in seen):
            problems.append("line {}: tool {!r}, CPython {!r}".format(e.line, want, seen))
    for line in python:
        if line not in lines and not unknown:
            problems.append("line {}: CPython registers, the tool reports nothing".format(line))
    return problems, unexercised


class Differential(unittest.TestCase):

    def test_corpus(self):
        names = sorted(n for n in os.listdir(CORPUS) if n.endswith(".py"))
        self.assertGreaterEqual(len(names), 62)
        for name in names:
            with self.subTest(name=name):
                path = os.path.join(CORPUS, name)
                with open(path, encoding="utf-8") as f:
                    source = f.read()
                analysis = Analysis("tests/corpus/" + name, "corpus/" + name[:-3], source, BASEURL)
                problems, _ = compare(analysis.entries, run_with_cpython(path, source))
                self.assertEqual(problems, [])


class Harness(unittest.TestCase):
    """The comparison itself catches what it must."""

    def entry(self, **kw):
        base = dict(kind="fixture", line=2, index=None, origin="text", body=b"a", reason=None,
                    host="h", status=200, headers={}, method="GET")
        base.update(kw)
        return types.SimpleNamespace(**base)

    def test_flags_a_different_body_status_header_or_host(self):
        ok = ("text", "a", 200, {})
        for python in ({2: [("GET", "h", [("text", "b", 200, {})])]}, {2: [("GET", "h", [("text", "a", 201, {})])]},
                       {2: [("GET", "h", [("text", "a", 200, {"X": "1"})])]}, {2: [("GET", "evil", [ok])]}):
            self.assertEqual(len(compare([self.entry()], python)[0]), 1, python)

    def test_flags_a_second_same_method_registration_that_differs(self):
        python = {2: [("GET", "h", [("text", "a", 200, {})]), ("GET", "h", [("text", "z", 200, {})])]}
        self.assertEqual(len(compare([self.entry()], python)[0]), 1)

    def test_flags_a_value_python_cannot_serve(self):
        problems, _ = compare([self.entry()], {2: [("GET", "h", [("ERR", "TypeError")])]})
        self.assertEqual(len(problems), 1)

    def test_flags_a_silent_miss_unless_an_unknown_pattern_was_reported(self):
        self.assertEqual(len(compare([], {5: [("GET", "h", [("text", "x", 200, {})])]})[0]), 1)
        unknown = self.entry(kind="skip", line=9, reason="unknown-pattern")
        self.assertEqual(compare([unknown], {5: [("GET", "h", [("text", "x", 200, {})])]})[0], [])

    def test_passes_equal_values_and_skips_matching_by_method(self):
        skip = self.entry(kind="skip", line=3, reason="body-not-literal")
        python = {2: [("POST", "h", [("text", "z", 200, {})]), ("GET", "h", [("text", "a", 200, {})])],
                  3: [("GET", "h", [("text", "z", 200, {})])]}
        self.assertEqual(compare([self.entry(), skip], python)[0], [])

    def test_runs_a_module_under_cpython(self):
        source = ('def test_a(reqmock):\n    x = "v"\n'
                  '    reqmock.get("https://h/a", text=f"{x}", status_code=201, headers={"A": "b"})\n')
        self.assertEqual(run_with_cpython("m.py", source), {3: [("GET", "h", [["text", "v", 201, {"A": "b"}]])]})


if __name__ == "__main__":
    if sys.argv[1:2] == ["--run"]:
        sys.stdout.write(_dumps(_run_here(sys.argv[2], sys.stdin.read())) + "\n")
    else:
        unittest.main()

"""Candidates and watched names of one test module.

A candidate is ``reqmock.<method>(...)``. Each one becomes fixture entries
(one per response) or skips with a reason; every other use of a watched name
outside its permitted positions is an ``unknown-pattern`` skip, so a way of
mocking the tool does not understand is reported rather than missed.
"""

import ast
import json
import re

from fxresolve import (
    ALPACA_FAMILIES, CANDIDATE_KEYWORDS, DEFS, ITEM_KEYWORDS, METHODS, READ_ATTRS, WATCHED,
    Entry, Fail, Module, NotJson, Unresolved, check_json,
)

HOST_ONLY = re.compile(r"https?://[^/?#]+")


class Skip(Exception):
    def __init__(self, reason, detail):
        super().__init__(reason, detail)
        self.reason = reason
        self.detail = detail


def is_candidate(node):
    return (isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute)
            and isinstance(node.func.value, ast.Name) and node.func.value.id == "reqmock"
            and node.func.attr in METHODS)


def is_annotation(parent, child):
    return ((isinstance(parent, ast.arg) and child is parent.annotation)
            or (isinstance(parent, DEFS) and child is parent.returns)
            or (isinstance(parent, ast.AnnAssign) and child is parent.annotation))


class Analysis:
    def __init__(self, path, module, source, baseurl):
        self.m = Module(path, module, source, baseurl)
        self.is_conftest = path == "tests/conftest.py"
        self.events = []
        self._excluded = {id(d) for d in self.m.excluded}
        self._visit(self.m.tree, owned=False, annotation=False)
        self.entries = self._entries()
        self.excluded = [(".".join([c.name for c in self.m.chain(d)] + [d.name]), d.lineno)
                         for d in self.m.excluded]

    # Traversal --------------------------------------------------------------

    def _visit(self, node, owned, annotation):
        for child in ast.iter_child_nodes(node):
            if id(child) in self._excluded:
                continue
            ann = annotation or is_annotation(node, child)
            if is_candidate(child):
                self.events.append(("candidate", child))
                self._visit(child, owned=True, annotation=ann)
                continue
            if not owned and not ann:
                self._classify(child)
            self._visit(child, owned, ann)

    def _unknown(self, node, line, detail):
        self.events.append(("unknown", node, line, detail))

    def _classify(self, node):
        if isinstance(node, DEFS + (ast.ClassDef,)) and node.name in WATCHED and not self.is_conftest:
            # A module's own reqmock fixture would override the one checked in
            # tests/conftest.py.
            self._unknown(node, node.lineno, type(node).__name__)
        elif isinstance(node, ast.Name) and node.id in WATCHED:
            if not self._permitted(node):
                self._unknown(node, node.lineno, type(self.m.parent[node]).__name__)
        elif isinstance(node, ast.arg) and node.arg in WATCHED - {"reqmock"}:
            self._unknown(node, node.lineno, "arg")
        elif isinstance(node, ast.Import):
            for alias in node.names:
                binds = alias.asname or alias.name.split(".")[0]
                from_rm = alias.name == "requests_mock" or alias.name.startswith("requests_mock.")
                ok = alias.name == "requests_mock" and alias.asname is None
                if (from_rm or binds in WATCHED) and not ok:
                    self._unknown(node, node.lineno, "Import")
        elif isinstance(node, ast.ImportFrom):
            mod = node.module or ""
            from_rm = node.level == 0 and (mod == "requests_mock" or mod.startswith("requests_mock."))
            for alias in node.names:
                binds = alias.asname or alias.name
                ok = (node.level == 0 and mod == "requests_mock" and alias.name == "Mocker"
                      and alias.asname is None)
                if (from_rm or binds in WATCHED) and not ok:
                    self._unknown(node, node.lineno, "ImportFrom")

    def _permitted(self, name):
        par = self.m.parent[name]
        if name.id == "reqmock":
            if (isinstance(par, ast.Attribute) and par.value is name and par.attr in READ_ATTRS
                    and isinstance(par.ctx, ast.Load)):
                return True
            if isinstance(par, ast.Call) and name in par.args:
                return self._helper_call(par, name)
            if isinstance(par, ast.keyword) and isinstance(self.m.parent.get(par), ast.Call):
                return self._helper_call(self.m.parent[par], name)
            return False
        if name.id == "requests_mock" and self.is_conftest:
            grand = self.m.parent.get(par)
            return (isinstance(par, ast.Attribute) and par.value is name and par.attr == "Mocker"
                    and isinstance(grand, ast.Call) and grand.func is par)
        return False

    def _helper_call(self, call, name):
        func = call.func
        if not isinstance(func, ast.Name) or "__" in func.id or self.m.star_import:
            return False
        nodes = self.m.all_names.get(func.id, [])
        if len(nodes) != 1 or not isinstance(nodes[0], DEFS):
            return False
        helper = nodes[0]
        if self.m.parent[helper] is not self.m.tree:
            return False
        params = helper.args.posonlyargs + helper.args.args
        if name in call.args:
            i = call.args.index(name)
            if any(isinstance(a, ast.Starred) for a in call.args[:i]):
                return False
            return i < len(params) and params[i].arg == "reqmock"
        return any(k.value is name and k.arg == "reqmock" for k in call.keywords) and \
            "reqmock" in [a.arg for a in helper.args.args + helper.args.kwonlyargs]

    # Entries ----------------------------------------------------------------

    def _entries(self):
        entries, ordinals = [], {}
        for event in self.events:
            if event[0] == "unknown":
                _, node, line, detail = event
                entry = Entry(kind="skip", qualname=self.m.qualname(node), line=line, method="UNKNOWN",
                              template=None, reason="unknown-pattern", detail=detail)
                new = [entry]
            else:
                new = self._candidate(event[1])
            key = (new[0].qualname, new[0].method, new[0].template if new[0].template is not None else "{}")
            ordinal = ordinals.get(key, 0)
            ordinals[key] = ordinal + 1
            for entry in new:
                entry.ordinal = ordinal
            entries.extend(new)
        return entries

    def _candidate(self, call):
        base = dict(qualname=self.m.qualname(call), line=call.lineno, method=call.func.attr.upper())
        keywords, shape = {}, None
        for k in call.keywords:
            if k.arg is None:
                shape = shape or ("unsupported-keyword", "**")
            elif k.arg not in CANDIDATE_KEYWORDS:
                shape = shape or ("unsupported-keyword", k.arg)
            else:
                keywords[k.arg] = k.value
        args = call.args
        if any(isinstance(a, ast.Starred) for a in args):
            shape = shape or ("unsupported-keyword", "Starred")
        elif len(args) > 2:
            shape = shape or ("unsupported-keyword", type(args[2]).__name__)
        elif len(args) == 2 and not isinstance(args[1], ast.List):
            shape = shape or ("unsupported-keyword", type(args[1]).__name__)
        if shape and shape[1] == "url" or not args:
            reason = shape if shape and shape[1] == "url" else ("url-not-literal", "Call")
            return [Entry(kind="skip", template=None, reason=reason[0], detail=reason[1], **base)]
        try:
            template, query, family, host = self.url(args[0])
        except Unresolved as e:
            return [Entry(kind="skip", template=None, reason="url-not-literal", detail=e.detail, **base)]
        base.update(template=template, query=query, family=family, host=host)
        if shape:
            return [Entry(kind="skip", reason=shape[0], detail=shape[1], **base)]
        notes = []
        if "additional_matcher" in keywords:
            m = keywords["additional_matcher"]
            label = (m.id if isinstance(m, ast.Name) else m.attr if isinstance(m, ast.Attribute)
                     else "<lambda>" if isinstance(m, ast.Lambda) else type(m).__name__)
            notes.append("request-matcher:" + label)
        items = None
        if "response_list" in keywords or len(args) == 2:
            single = [k for k in ITEM_KEYWORDS if k in keywords]
            if ("response_list" in keywords and len(args) == 2) or single:
                return [Entry(kind="skip", reason="unsupported-keyword", detail="response_list", **base)]
            items = keywords.get("response_list", args[1] if len(args) == 2 else None)
            if not isinstance(items, ast.List):
                return [Entry(kind="skip", reason="unsupported-keyword", detail=type(items).__name__, **base)]
        if items is None:
            return [self._response({k: v for k, v in keywords.items() if k in ITEM_KEYWORDS}, base, notes, None)]
        out = []
        for i, item in enumerate(items.elts):
            if not isinstance(item, ast.Dict) or any(
                    not (isinstance(k, ast.Constant) and isinstance(k.value, str)) for k in item.keys):
                out.append(Entry(kind="skip", index=i, reason="unsupported-keyword",
                                 detail=type(item).__name__, **base))
                continue
            fields = {k.value: v for k, v in zip(item.keys, item.values)}
            bad = [k for k in fields if k not in ITEM_KEYWORDS]
            if bad:
                out.append(Entry(kind="skip", index=i, reason="unsupported-keyword", detail=bad[0], **base))
                continue
            out.append(self._response(fields, base, notes, i))
        return out or [Entry(kind="skip", reason="unsupported-keyword", detail="response_list", **base)]

    def _response(self, fields, base, notes, index):
        try:
            if "text" in fields and "json" in fields:
                raise Skip("unsupported-keyword", "json")
            body, origin = b"", "none"
            if "text" in fields:
                value = self._value(fields["text"], "body-not-literal")
                if not isinstance(value, str):
                    raise Skip("body-not-literal", type(value).__name__)
                try:
                    body = value.encode("utf-8")
                except UnicodeEncodeError:
                    raise Skip("body-not-literal", "str") from None
                origin = "text"
            elif "json" in fields:
                value = self._value(fields["json"], "body-not-literal")
                if value is None:
                    raise Skip("body-not-json", "None")  # requests-mock serves no body for json=None
                try:
                    check_json(value)
                except NotJson as e:
                    raise Skip("body-not-json", e.detail) from None
                body, origin = json.dumps(value).encode("utf-8"), "json-literal"
            status = 200
            if "status_code" in fields:
                status = self._value(fields["status_code"], "status-not-literal")
                if not isinstance(status, int) or isinstance(status, bool):
                    raise Skip("status-not-literal", type(status).__name__)
            headers = {}
            if "headers" in fields:
                headers = self._value(fields["headers"], "headers-not-literal")
                if not isinstance(headers, dict) or not all(
                        isinstance(k, str) and isinstance(v, str) for k, v in headers.items()):
                    raise Skip("headers-not-literal", type(headers).__name__)
            if origin == "text" and any(k.lower() == "content-type" for k in headers):
                # requests-mock then encodes the text with that header's
                # charset, which this tool does not model.
                raise Skip("body-not-literal", "charset")
        except Skip as s:
            return Entry(kind="skip", index=index, reason=s.reason, detail=s.detail, **base)
        return Entry(kind="fixture", index=index, status=status, headers=headers, body=body,
                     origin=origin, notes=notes, **base)

    def _value(self, node, reason):
        try:
            return self.m.evaluate(node)
        except Unresolved as e:
            raise Skip(reason, e.detail) from None
        except NotJson as e:
            raise Skip("body-not-json", e.detail) from None

    # URLs -------------------------------------------------------------------

    def url(self, node):
        parts = self._expand(node)
        merged = []
        for kind, value in parts:
            if kind == "lit" and merged and merged[-1][0] == "lit":
                merged[-1] = ("lit", merged[-1][1] + value)
            else:
                merged.append((kind, value))
        host = None
        if merged and merged[0][0] == "lit" and re.match(r"https?://", merged[0][1]):
            rest = merged[0][1].split("://", 1)[1]
            cut = min([i for i in (rest.find("/"), rest.find("?")) if i >= 0], default=-1)
            if cut < 0 and len(merged) > 1:
                raise Unresolved("host")
            host = rest if cut < 0 else rest[:cut]
            merged[0] = ("lit", "" if cut < 0 else rest[cut:])
        elif merged and merged[0][0] == "expr":
            first = merged[0][1]
            value = (self.m._fstring_part(first, frozenset()) if isinstance(first, ast.FormattedValue)
                     else self.m.evaluate(first))
            if not (isinstance(value, str) and HOST_ONLY.fullmatch(value)):
                raise Unresolved("host")
            host = value.split("://", 1)[1]
            merged = merged[1:]
        elif not (merged and merged[0][0] == "lit" and merged[0][1].startswith("/")):
            raise Unresolved(type(node).__name__ if not merged else "scheme")
        path, query, in_query = "", None, False
        for kind, value in merged:
            text = value if kind == "lit" else "{" + self._label(value) + "}"
            if kind == "lit" and not in_query and "?" in value:
                before, after = value.split("?", 1)
                path, query, in_query = path + before, after, True
            elif in_query:
                query += text
            else:
                path += text
        if host is None:
            top = self.m.module.split("/", 1)[0]
            family = top if top in ("broker", "trading", "data") else "other"
        else:
            family = ALPACA_FAMILIES.get(host.lower(), "other")
        return path, query, family, host

    def _label(self, node):
        if isinstance(node, ast.FormattedValue):
            node = node.value
        if isinstance(node, ast.Name):
            return node.id
        if isinstance(node, ast.Attribute):
            return node.attr
        return ""

    def _expand(self, node):
        if isinstance(node, ast.Name):
            return self._flatten(self.m.binding_of(node).value)
        return self._flatten(node)

    def _flatten(self, node):
        if isinstance(node, ast.BinOp) and isinstance(node.op, ast.Add):
            return self._flatten(node.left) + self._flatten(node.right)
        if isinstance(node, ast.Constant) and isinstance(node.value, str):
            return [("lit", node.value)]
        if isinstance(node, ast.JoinedStr):
            out = []
            for v in node.values:
                out.append(("lit", v.value) if isinstance(v, ast.Constant) else ("expr", v))
            return out
        return [("expr", node)]


def check_conftest(sources):
    """Fail unless tests/conftest.py defines the reqmock fixture as the spec
    requires and no other conftest.py defines reqmock."""
    source = sources.get("tests/conftest.py")
    if source is None:
        raise Fail("tests/conftest.py is missing")
    tree = ast.parse(source, filename="tests/conftest.py")
    parents = {c: p for p in ast.walk(tree) for c in ast.iter_child_nodes(p)}
    calls = [n for n in ast.walk(tree) if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute)
             and n.func.attr == "Mocker" and isinstance(n.func.value, ast.Name)
             and n.func.value.id == "requests_mock"]
    if len(calls) != 1:
        raise Fail("tests/conftest.py must contain exactly one requests_mock.Mocker( call")
    node = calls[0]
    while node in parents and not isinstance(node, DEFS):
        node = parents[node]
    if not (isinstance(node, DEFS) and node.name == "reqmock" and any(_is_fixture(d) for d in node.decorator_list)):
        raise Fail("the requests_mock.Mocker( call is not inside a pytest fixture named reqmock")
    for path, text in sources.items():
        if path != "tests/conftest.py" and path.endswith("/conftest.py"):
            other = ast.parse(text, filename=path)
            if any(isinstance(n, DEFS) and n.name == "reqmock" for n in ast.walk(other)):
                raise Fail(path + " defines reqmock")


def _is_fixture(decorator):
    target = decorator.func if isinstance(decorator, ast.Call) else decorator
    return (isinstance(target, ast.Attribute) and target.attr == "fixture"
            and isinstance(target.value, ast.Name) and target.value.id == "pytest")

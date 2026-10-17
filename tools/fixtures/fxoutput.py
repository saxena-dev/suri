"""From analysed modules to the fixture tree: identities, file names, kinds,
notes, redaction, the index, the README and LICENSE, and writing or checking
the tree."""

import ast
import hashlib
import json
import os
import re
import shutil
import tempfile

from fxcandidates import Analysis, check_conftest
from fxresolve import Fail

REPOSITORY = "https://github.com/alpacahq/alpaca-py"
LICENSE_ID = "Apache-2.0"
ENTRY_PATH = "tools/fixtures/extract.py"
PYTHON = "3.14"
SEGMENT = re.compile(r"[a-z0-9_][a-z0-9_.]*")
REDACTIONS = [
    ("aws-access-key-id", re.compile(r"(?:AKIA|ASIA)[0-9A-Z]{16}")),
    ("private-key", re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----", re.S)),
    ("jwt", re.compile(r"eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+")),
    ("bearer-token", re.compile(r"[Bb]earer [A-Za-z0-9._~+/=-]{16,}")),
]
EXTENSIONS = {"json": "json", "text": "txt", "empty": "empty"}

README = """# Upstream response fixtures

These files are HTTP response bodies taken from the test suite of
alpaca-py, the Alpaca Python SDK:

- repository: {repository}
- commit: {commit}
- licence: Apache License 2.0 (the `LICENSE` file in this directory)

They were extracted from the test sources by `{entry}` without running
any upstream code, and are used as test data only.

Changes made to the upstream material:

- each response body a test serves is stored in its own file, byte for
  byte as the test defines it;
- bodies the tests give as Python values (`json=`) are serialised with
  Python's `json.dumps`;
- strings shaped like credentials are replaced by runs of `X` of the same
  length, and each such change is noted in `index.json`.

Nothing else is changed. `index.json` records where each file came from.
"""


def extractor_sha256(tool_dir):
    lines = []
    for name in sorted(os.listdir(tool_dir)):
        full = os.path.join(tool_dir, name)
        if name.endswith(".py") and not name.startswith("test_") and os.path.isfile(full):
            with open(full, "rb") as f:
                lines.append(hashlib.sha256(f.read()).hexdigest() + "  " + name + "\n")
    return hashlib.sha256("".join(lines).encode("utf-8")).hexdigest()


def redact(text):
    kinds = set()
    for kind, pattern in REDACTIONS:
        def mask(match, kind=kind):
            kinds.add(kind)
            return "X" * len(match.group(0))
        text = pattern.sub(mask, text)
    return text, kinds


def body_kind(body):
    """The kind of a body and its JSON notes (duplicate keys, constants)."""
    if not body:
        return "empty", []
    notes = set()

    def pairs(items):
        seen = set()
        for key, _ in items:
            if key in seen:
                notes.add("duplicate-key:" + key)
            seen.add(key)
        return dict(items)

    def constant(name):
        notes.add("json-constant:" + name)
        return float(name.replace("Infinity", "inf"))

    try:
        json.loads(body.decode("utf-8"), object_pairs_hook=pairs, parse_constant=constant)
    except (ValueError, UnicodeDecodeError):
        return "text", []
    return "json", sorted(notes)


def slug(method, template):
    text = re.sub(r"[^a-z0-9]+", "-", (method + " " + template).lower()).strip("-")
    return text[:80].rstrip("-")


def segments_ok(path):
    return all(SEGMENT.fullmatch(s) for s in path.split("/"))


def parse_baseurl(source):
    tree = ast.parse(source, filename="alpaca/common/enums.py")
    for node in tree.body:
        if isinstance(node, ast.ClassDef) and node.name == "BaseURL":
            return {s.targets[0].id: s.value.value for s in node.body
                    if isinstance(s, ast.Assign) and len(s.targets) == 1 and isinstance(s.targets[0], ast.Name)
                    and isinstance(s.value, ast.Constant) and isinstance(s.value.value, str)}
    raise Fail("alpaca/common/enums.py has no class BaseURL")


def build(up, tool_dir):
    """Every output file as {relative path: bytes}."""
    tests = sorted((p for p in up.blobs if p.startswith("tests/") and p.endswith(".py")),
                   key=lambda p: p.encode("utf-8"))
    live = [p for p in tests if p.startswith("tests/live/")]
    tests = [p for p in tests if not p.startswith("tests/live/")]
    blobs = {p: up.read(p) for p in tests + ["alpaca/common/enums.py", "LICENSE"]}
    texts = {}
    for path in tests + ["alpaca/common/enums.py"]:
        try:
            texts[path] = blobs[path].decode("utf-8")
        except UnicodeDecodeError:
            raise Fail(path + " is not UTF-8") from None
    check_conftest(texts)
    baseurl = parse_baseurl(texts["alpaca/common/enums.py"])
    excluded = [{"path": "tests/live/", "qualname": None, "line": None, "reason": "live-probe"}] if live else []
    fixtures, skipped, files = [], [], {}
    for path in tests:
        module = path[len("tests/"):-len(".py")]
        try:
            analysis = Analysis(path, module, texts[path], baseurl)
        except SyntaxError as e:
            raise Fail(path + " does not parse with this interpreter: " + str(e)) from None
        for qual, line in analysis.excluded:
            excluded.append({"path": redact(path)[0], "qualname": redact(qual)[0], "line": line,
                             "reason": "shadowed-definition"})
        for entry in analysis.entries:
            record, data = make_record(path, module, entry)
            if entry.kind == "skip":
                skipped.append(record)
                continue
            if record["file"] in files:
                raise Fail("two fixtures map to the file " + record["file"])
            files[record["file"]] = data
            fixtures.append(record)
    index = {
        "schema": 1,
        "upstream": {"repository": REPOSITORY, "commit": up.commit, "tree": up.tree, "license": LICENSE_ID},
        "extractor": {"path": ENTRY_PATH, "sha256": extractor_sha256(tool_dir), "python": PYTHON},
        "sources": [{"path": redact(p)[0], "sha256": hashlib.sha256(blobs[p]).hexdigest()}
                    for p in sorted(blobs, key=lambda p: p.encode("utf-8"))],
        "excluded": sorted(excluded, key=lambda e: (e["path"], e["line"] is not None, e["line"] or 0)),
        "fixtures": sorted(fixtures, key=lambda r: r["id"]),
        "skipped": sorted(skipped, key=lambda r: r["id"]),
    }
    out = {name: data for name, data in files.items()}
    out["index.json"] = (json.dumps(index, indent=2, ensure_ascii=False) + "\n").encode("utf-8")
    out["LICENSE"] = blobs["LICENSE"]
    out["README.md"] = README.format(repository=REPOSITORY, commit=up.commit, entry=ENTRY_PATH).encode("utf-8")
    return out


def make_record(path, module, entry):
    kinds = set()

    def clean(text):
        if text is None:
            return None
        text, found = redact(text)
        kinds.update(found)
        return text

    # Every string that reaches the index or a file name is redacted first, so
    # the id and the lowercased file name are built from redacted parts.
    path, module, qualname = clean(path), clean(module), clean(entry.qualname)
    template_or_none = clean(entry.template)
    template = template_or_none if template_or_none is not None else "{}"
    ident = "{}::{}::{} {}#{}".format(module, qualname, entry.method, template, entry.ordinal)
    if entry.index is not None:
        ident += "@{}".format(entry.index)
    source = {"path": path, "line": entry.line, "qualname": qualname}
    if entry.kind == "skip":
        return {"id": ident, "source": source, "method": entry.method, "path": template_or_none,
                "reason": entry.reason, "detail": clean(entry.detail)}, None
    text = clean(entry.body.decode("utf-8"))
    headers = {clean(k): clean(v) for k, v in entry.headers.items()}
    query = clean(entry.query)
    data = text.encode("utf-8")
    kind, notes = body_kind(data)  # from the stored (redacted) bytes, so notes carry no secret
    notes = set(notes) | {clean(n) for n in entry.notes} | {"redacted:" + k for k in kinds}
    directory = module.lower() + "/" + qualname.lower()
    if not segments_ok(directory):
        raise Fail("fixture directory '" + directory + "' has a segment outside [a-z0-9_.]")
    name = "{}-{}".format(slug(entry.method, template), entry.ordinal)
    if entry.index is not None:
        name += "-{}".format(entry.index)
    record = {
        "id": ident,
        "file": directory + "/" + name + "." + EXTENSIONS[kind],
        "source": source,
        "family": entry.family,
        "method": entry.method,
        "path": template_or_none,
        "query": query,
        "status": entry.status,
        "headers": headers,
        "kind": kind,
        "origin": entry.origin,
        "bytes": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
        "notes": sorted(notes),
    }
    return record, data


def sibling(out, suffix="new"):
    out = os.path.abspath(out)
    return os.path.join(os.path.dirname(out), "." + os.path.basename(out) + "." + suffix)


def remove_leftover(out):
    for suffix in ("new", "old"):
        if os.path.isdir(sibling(out, suffix)):
            shutil.rmtree(sibling(out, suffix))


def write_tree(root, files):
    for rel, data in files.items():
        full = os.path.join(root, *rel.split("/"))
        os.makedirs(os.path.dirname(full), exist_ok=True)
        with open(full, "wb") as f:
            f.write(data)


def install(out, files):
    """Build the tree beside ``out``, then swap it in: the previous tree is
    renamed aside before the new one takes its place, and removed last."""
    new, old = sibling(out), sibling(out, "old")
    write_tree(new, files)
    if os.path.isdir(out):
        os.rename(out, old)
    os.rename(new, out)
    if os.path.isdir(old):
        shutil.rmtree(old, ignore_errors=True)  # a leftover is removed at the next start


def read_tree(root):
    found = {}
    for base, _, names in os.walk(root):
        for name in names:
            full = os.path.join(base, name)
            rel = os.path.relpath(full, root).replace(os.sep, "/")
            with open(full, "rb") as f:
                found[rel] = f.read()
    return found


def differences(out, files, commit):
    """What differs between the committed tree and a fresh extraction."""
    with tempfile.TemporaryDirectory() as tmp:
        write_tree(tmp, files)
        fresh = read_tree(tmp)
    current = read_tree(out) if os.path.isdir(out) else {}
    diffs = []
    for rel in sorted(set(fresh) | set(current)):
        if rel not in current:
            diffs.append("missing: " + rel)
        elif rel not in fresh:
            diffs.append("extra: " + rel)
        elif current[rel] != fresh[rel]:
            diffs.append("changed: " + rel)
    try:
        recorded = json.loads(current.get("index.json", b"{}").decode("utf-8"))["upstream"]["commit"]
    except (ValueError, KeyError, TypeError):
        recorded = None
    if recorded != commit:
        diffs.append("index.json records commit {} not {}".format(recorded, commit))
    return diffs

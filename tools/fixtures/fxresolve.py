"""Static analysis of one alpaca-py test module.

Finds every requests-mock registration made through the ``reqmock`` fixture
and works out what it serves, without running any code: a small evaluator
handles literals, string concatenation, f-strings, ``UUID("...")``,
``json.dumps(...)``, ``BaseURL.<NAME>.value`` and names bound exactly once to
a string, number or UUID. Anything else is reported as a skip with a reason,
never guessed.
"""

import ast
import json
import uuid

WATCHED = frozenset({"reqmock", "requests_mock", "Mocker"})
METHODS = frozenset({"get", "post", "put", "patch", "delete"})
READ_ATTRS = frozenset({"request_history", "call_count", "last_request", "called", "called_once"})
CANDIDATE_KEYWORDS = frozenset({"text", "json", "status_code", "headers", "additional_matcher", "response_list"})
ITEM_KEYWORDS = frozenset({"text", "json", "status_code", "headers"})
DEFS = (ast.FunctionDef, ast.AsyncFunctionDef)
SCOPES = DEFS + (ast.ClassDef, ast.Lambda, ast.ListComp, ast.SetComp, ast.DictComp, ast.GeneratorExp)
TYPE_PARAMS = tuple(getattr(ast, n) for n in ("TypeVar", "ParamSpec", "TypeVarTuple") if hasattr(ast, n))
ALPACA_FAMILIES = {
    "broker-api.alpaca.markets": "broker",
    "broker-api.sandbox.alpaca.markets": "broker",
    "api.alpaca.markets": "trading",
    "paper-api.alpaca.markets": "trading",
    "data.alpaca.markets": "data",
    "data.sandbox.alpaca.markets": "data",
}


class Fail(Exception):
    """A condition that stops the whole run (exit 2)."""


class Unresolved(Exception):
    def __init__(self, detail):
        super().__init__(detail)
        self.detail = detail


class NotJson(Exception):
    def __init__(self, detail):
        super().__init__(detail)
        self.detail = detail


class Entry:
    """One extracted response or one skip, before naming and output."""

    def __init__(self, **kw):
        self.kind = kw["kind"]  # "fixture" or "skip"
        self.qualname = kw["qualname"]
        self.line = kw["line"]
        self.method = kw["method"]
        self.template = kw["template"]  # None when the path is unknown
        self.ordinal = kw.get("ordinal", 0)
        self.index = kw.get("index")
        self.query = kw.get("query")
        self.family = kw.get("family")
        self.host = kw.get("host")
        self.status = kw.get("status", 200)
        self.headers = kw.get("headers", {})
        self.body = kw.get("body", b"")
        self.origin = kw.get("origin", "none")
        self.notes = list(kw.get("notes", ()))
        self.reason = kw.get("reason")
        self.detail = kw.get("detail")


def node_bindings(node):
    """The names one node binds in the scope that contains it."""
    if isinstance(node, ast.Name) and isinstance(node.ctx, (ast.Store, ast.Del)):
        yield node.id
    elif isinstance(node, DEFS + (ast.ClassDef,)):
        yield node.name
    elif isinstance(node, ast.arg):
        yield node.arg
    elif isinstance(node, (ast.Import, ast.ImportFrom)):
        for alias in node.names:
            if alias.name != "*":
                yield alias.asname or alias.name.split(".")[0]
    elif isinstance(node, (ast.Global, ast.Nonlocal)):
        yield from node.names
    elif isinstance(node, ast.ExceptHandler) and node.name:
        yield node.name
    elif isinstance(node, (ast.MatchAs, ast.MatchStar)) and node.name:
        yield node.name
    elif isinstance(node, ast.MatchMapping) and node.rest:
        yield node.rest
    elif isinstance(node, TYPE_PARAMS):
        yield node.name


def target_names(target):
    """Names an assignment target binds (not names read inside it)."""
    if isinstance(target, ast.Name):
        return {target.id}
    if isinstance(target, (ast.Tuple, ast.List)):
        return set().union(*(target_names(t) for t in target.elts)) if target.elts else set()
    if isinstance(target, ast.Starred):
        return target_names(target.value)
    return set()


def direct_bindings(stmt):
    """Names a statement rebinds or unbinds unconditionally at its own level."""
    if isinstance(stmt, DEFS + (ast.ClassDef,)):
        return {stmt.name}
    if isinstance(stmt, (ast.Import, ast.ImportFrom)):
        return set(node_bindings(stmt))
    if isinstance(stmt, ast.Assign):
        targets = stmt.targets
    elif isinstance(stmt, ast.AnnAssign) and stmt.value is not None:
        targets = [stmt.target]
    elif isinstance(stmt, ast.Delete):
        targets = stmt.targets
    elif isinstance(stmt, (ast.With, ast.AsyncWith)):
        targets = [i.optional_vars for i in stmt.items if i.optional_vars is not None]
    else:
        targets = []
    return set().union(*(target_names(t) for t in targets)) if targets else set()


def _callee(call):
    func = call.func
    return func.id if isinstance(func, ast.Name) else func.attr if isinstance(func, ast.Attribute) else None


def walk_children(node):
    """Depth-first pre-order over a node's descendants, children in
    ``ast.iter_child_nodes`` order."""
    for child in ast.iter_child_nodes(node):
        yield child
        yield from walk_children(child)


def outside_children(node):
    """Children of a scope node that Python evaluates in the enclosing scope."""
    out = []
    if isinstance(node, DEFS + (ast.Lambda,)):
        out += node.args.defaults + [d for d in node.args.kw_defaults if d is not None]
    if isinstance(node, DEFS):
        out += node.decorator_list + ([node.returns] if node.returns else [])
    if isinstance(node, ast.ClassDef):
        out += node.decorator_list + node.bases + node.keywords
    return out


class Module:
    def __init__(self, path, module, source, baseurl):
        self.path = path
        self.module = module
        self.baseurl = baseurl
        self.tree = ast.parse(source, filename=path)
        self.parent = {}
        for node in ast.walk(self.tree):
            for child in ast.iter_child_nodes(node):
                self.parent[child] = node
        self.outside = {}  # id(node) -> ids of the scopes it is evaluated outside of
        for node in ast.walk(self.tree):
            if isinstance(node, SCOPES):
                for child in outside_children(node):
                    for n in ast.walk(child):
                        self.outside.setdefault(id(n), set()).add(id(node))
        self.star_import = any(
            isinstance(n, ast.ImportFrom) and any(a.name == "*" for a in n.names) for n in ast.walk(self.tree)
        )
        self.all_names = {}
        for node in ast.walk(self.tree):
            for name in node_bindings(node):
                self.all_names.setdefault(name, []).append(node)
        self.dynamic = self._dynamic_rebinding()
        self.excluded = self._shadowed_definitions()
        self.known = {
            "UUID": self._imported_once("UUID", lambda n, a: isinstance(n, ast.ImportFrom) and n.module == "uuid"),
            "json": self._imported_once("json", lambda n, a: isinstance(n, ast.Import) and a.name == "json"),
            "BaseURL": self._imported_once(
                "BaseURL", lambda n, a: isinstance(n, ast.ImportFrom) and n.module == "alpaca.common.enums"
            ),
        }

    # Scopes -------------------------------------------------------------

    def chain(self, node):
        """Enclosing defs and classes, outermost first, honouring the parts
        of a definition that Python evaluates in the enclosing scope."""
        result = []
        outside = self.outside.get(id(node), set())
        child = node
        while child in self.parent:
            par = self.parent[child]
            if isinstance(par, DEFS + (ast.ClassDef,)) and id(par) not in outside:
                result.append(par)
            child = par
        return list(reversed(result))

    def qualname(self, node):
        names = [d.name for d in self.chain(node)]
        return ".".join(names) if names else "<module>"

    def defs_of(self, node):
        """Enclosing function definitions of a use, innermost first."""
        return [d for d in reversed(self.chain(node)) if isinstance(d, DEFS)]

    def own_nodes(self, scope):
        """Nodes in a scope's own namespace (nested scopes appear, their
        interiors do not)."""
        roots = list(scope.body)
        if isinstance(scope, DEFS):
            a = scope.args
            roots += a.posonlyargs + a.args + a.kwonlyargs + [x for x in (a.vararg, a.kwarg) if x]
        stack = list(reversed(roots))
        while stack:
            node = stack.pop()
            yield node
            if isinstance(node, SCOPES):
                continue
            stack.extend(reversed(list(ast.iter_child_nodes(node))))

    def own_bindings(self, scope, name):
        return [n for n in self.own_nodes(scope) if name in node_bindings(n)]

    def region_bindings(self, scope, name):
        if isinstance(scope, ast.Module):
            return list(self.all_names.get(name, []))
        roots = [scope.args] + list(scope.body) + list(getattr(scope, "type_params", []) or [])
        found = []
        for root in roots:
            for node in [root] + list(walk_children(root)):
                if name in node_bindings(node):
                    found.append(node)
        return found

    def _ancestors(self, node):
        while node in self.parent:
            node = self.parent[node]
            yield node

    def top_statement(self, scope, node):
        child = node
        while self.parent.get(child) is not scope:
            if child not in self.parent:
                raise Unresolved("scope")
            child = self.parent[child]
        return child

    def _shadowed_definitions(self):
        """Definitions in a module or class block (what pytest collects after
        the module has run) rebound unconditionally by a later statement of
        the same block, so they never run. Alternate if/else branches are not
        shadowed, and nothing in a function body is: a function may call a
        definition before rebinding it."""
        excluded = []
        for node in ast.walk(self.tree):
            owner = node
            while not isinstance(owner, (ast.Module, ast.ClassDef) + DEFS + (ast.Lambda,)):
                owner = self.parent[owner]
            if not isinstance(owner, (ast.Module, ast.ClassDef)) or any(
                    isinstance(a, DEFS + (ast.Lambda,)) for a in self._ancestors(owner)):
                continue
            for _, block in ast.iter_fields(node):
                if not (isinstance(block, list) and block and isinstance(block[0], ast.stmt)):
                    continue
                for i, stmt in enumerate(block):
                    if isinstance(stmt, DEFS + (ast.ClassDef,)) and any(
                            stmt.name in direct_bindings(later) for later in block[i + 1:]):
                        excluded.append(stmt)
        return excluded

    def _dynamic_rebinding(self):
        """Whether the module can rebind its own names without syntax: through
        globals(), locals(), vars(), exec, eval or __import__, through
        sys.modules, import_module, __globals__, f_globals or __dict__, by
        assigning an attribute of an imported module, or by setattr
        (monkeypatch.setattr included) on anything but a plain name that no
        import binds. Every module-level
        name is then unresolvable, as with a star import."""
        imported = {name for name, nodes in self.all_names.items()
                    if any(isinstance(n, (ast.Import, ast.ImportFrom)) for n in nodes)}
        for n in ast.walk(self.tree):
            if isinstance(n, ast.Name) and n.id in ("globals", "locals", "vars", "exec", "eval", "__import__",
                                                    "import_module"):
                return True
            if isinstance(n, ast.Attribute) and n.attr in ("import_module", "modules", "__globals__", "f_globals",
                                                           "f_locals", "__dict__"):
                return True
            if (isinstance(n, ast.Attribute) and isinstance(n.ctx, (ast.Store, ast.Del))
                    and isinstance(n.value, ast.Name) and n.value.id in imported):
                return True  # an attribute of an imported module, the test module itself included
            if isinstance(n, ast.ImportFrom) and n.module in ("sys", "importlib") and any(
                    a.name in ("modules", "import_module") for a in n.names):
                return True
            if isinstance(n, ast.Call) and n.args:
                callee, target = _callee(n), n.args[0]
                if callee == "patch" and isinstance(target, ast.Constant) and isinstance(target.value, str):
                    return True  # unittest.mock.patch("pkg.module.name")
                if callee in ("setattr", "object") and not (
                        isinstance(target, ast.Name) and target.id not in imported):
                    return True  # setattr, monkeypatch.setattr, mock.patch.object
        return False

    def _imported_once(self, name, ok):
        if self.star_import or self.dynamic:
            return False
        nodes = self.all_names.get(name, [])
        if len(nodes) != 1 or not isinstance(nodes[0], (ast.Import, ast.ImportFrom)):
            return False
        node = nodes[0]
        if not any(a.name == name and a.asname is None and ok(node, a) for a in node.names):
            return False
        # The object must be used only where the evaluator reads it, so nothing
        # in the module can change it in place (an alias, an attribute store,
        # a second import of its module under another name).
        modname = {"UUID": "uuid", "json": "json", "BaseURL": "alpaca.common.enums"}[name]
        for n in ast.walk(self.tree):
            if isinstance(n, ast.Import) and n is not node and any(a.name == modname for a in n.names):
                return False
            if isinstance(n, ast.Name) and n.id == name and isinstance(n.ctx, ast.Load) and not self._read_only(n):
                return False
        return True

    def _read_only(self, n):
        par = self.parent[n]
        grand = self.parent.get(par)
        if n.id == "json":
            return (isinstance(par, ast.Attribute) and par.attr == "dumps" and isinstance(grand, ast.Call)
                    and grand.func is par)
        if n.id == "UUID":
            return isinstance(par, ast.Call) and par.func is n
        return (isinstance(par, ast.Attribute) and isinstance(grand, ast.Attribute) and grand.attr == "value"
                and isinstance(grand.ctx, ast.Load))

    # Names ----------------------------------------------------------------

    def binding_of(self, use):
        """The single plain assignment that binds ``use``, or raise."""
        name = use.id
        if "__" in name:
            raise Unresolved(name)
        defs = self.defs_of(use)
        scope = None
        if defs and self.own_bindings(defs[0], name):
            scope = defs[0]
        else:
            # A closure variable or a `global` declaration elsewhere is another
            # binding in the module-wide region below, so it is unresolved too.
            if self.star_import or self.dynamic:
                raise Unresolved(name)
            scope = self.tree
            if not self.own_bindings(scope, name):
                raise Unresolved(name)
        own = self.own_bindings(scope, name)
        if len(own) != 1 or not isinstance(own[0], ast.Name):
            raise Unresolved(name)
        assign = self.parent.get(own[0])
        if not (isinstance(assign, ast.Assign) and len(assign.targets) == 1 and assign.targets[0] is own[0]):
            raise Unresolved(name)
        if assign not in scope.body:
            raise Unresolved(name)
        use_stmt = self.top_statement(scope, use)
        if scope.body.index(assign) >= scope.body.index(use_stmt):
            raise Unresolved(name)
        if len(self.region_bindings(scope, name)) != 1:
            raise Unresolved(name)
        return assign

    def resolve(self, use, active):
        assign = self.binding_of(use)
        if id(assign) in active:
            raise Unresolved(use.id)
        value = self.evaluate(assign.value, active | {id(assign)})
        if isinstance(value, (str, int, float, uuid.UUID)):
            return value
        raise Unresolved(use.id)

    # Evaluation -----------------------------------------------------------

    def evaluate(self, node, active=frozenset()):
        if isinstance(node, ast.Constant):
            if isinstance(node.value, (str, bytes, bool, int, float)) or node.value is None:
                return node.value
            raise Unresolved("Constant")
        if isinstance(node, ast.Name):
            return self.resolve(node, active)
        if isinstance(node, ast.JoinedStr):
            return "".join(self._fstring_part(v, active) for v in node.values)
        if isinstance(node, ast.BinOp) and isinstance(node.op, ast.Add):
            left, right = self.evaluate(node.left, active), self.evaluate(node.right, active)
            if isinstance(left, str) and isinstance(right, str):
                return left + right
            raise Unresolved("BinOp")
        if isinstance(node, ast.UnaryOp) and isinstance(node.op, (ast.USub, ast.UAdd)):
            value = self.evaluate(node.operand, active)
            if isinstance(value, (int, float)) and not isinstance(value, bool):
                return -value if isinstance(node.op, ast.USub) else value
            raise Unresolved("UnaryOp")
        if isinstance(node, ast.Dict):
            if any(k is None for k in node.keys):
                raise Unresolved("Dict")
            return {self._key(k, active): self.evaluate(v, active) for k, v in zip(node.keys, node.values)}
        if isinstance(node, (ast.List, ast.Tuple, ast.Set)):
            if any(isinstance(e, ast.Starred) for e in node.elts):
                raise Unresolved("Starred")
            items = [self.evaluate(e, active) for e in node.elts]
            return {ast.List: list, ast.Tuple: tuple, ast.Set: _set}[type(node)](items)
        if isinstance(node, ast.Attribute):
            return self._baseurl(node)
        if isinstance(node, ast.Call):
            return self._call(node, active)
        raise Unresolved(type(node).__name__)

    def _key(self, node, active):
        key = self.evaluate(node, active)
        try:
            hash(key)
        except TypeError:
            raise Unresolved("Dict") from None
        return key

    def _fstring_part(self, node, active):
        if isinstance(node, ast.Constant):
            return node.value
        if not isinstance(node, ast.FormattedValue):
            raise Unresolved(type(node).__name__)
        value = self.evaluate(node.value, active)
        if node.conversion == ord("r"):
            if isinstance(value, uuid.UUID):
                raise Unresolved("UUID")
            value = repr(value)
        elif node.conversion == ord("s"):
            value = str(value)
        elif node.conversion == ord("a"):
            value = ascii(value)
        spec = self.evaluate(node.format_spec, active) if node.format_spec is not None else ""
        try:
            return format(value, spec)
        except (TypeError, ValueError):
            raise Unresolved("FormattedValue") from None

    def _baseurl(self, node):
        inner = node.value
        if (node.attr == "value" and isinstance(inner, ast.Attribute) and isinstance(inner.value, ast.Name)
                and inner.value.id == "BaseURL" and self.known["BaseURL"] and inner.attr in self.baseurl):
            return self.baseurl[inner.attr]
        raise Unresolved("Attribute")

    def _call(self, node, active):
        func = node.func
        if isinstance(func, ast.Name) and func.id == "UUID" and self.known["UUID"]:
            if len(node.args) == 1 and not node.keywords and isinstance(node.args[0], ast.Constant) \
                    and isinstance(node.args[0].value, str):
                try:
                    return uuid.UUID(node.args[0].value)
                except ValueError:
                    raise Unresolved("Call") from None
            raise Unresolved("Call")
        if (isinstance(func, ast.Attribute) and func.attr == "dumps" and isinstance(func.value, ast.Name)
                and func.value.id == "json" and self.known["json"] and len(node.args) == 1
                and all(k.arg and isinstance(k.value, ast.Constant) for k in node.keywords)):
            value = self.evaluate(node.args[0], active)
            check_json(value)
            try:
                return json.dumps(value, **{k.arg: k.value.value for k in node.keywords})
            except (TypeError, ValueError):
                raise NotJson("Call") from None
        raise Unresolved("Call")


class _set(frozenset):
    """A set display's value: never JSON (check_json rejects it)."""


def check_json(value):
    """Raise NotJson unless ``value`` is plain JSON data."""
    if isinstance(value, dict):
        for key, item in value.items():
            if not isinstance(key, str):
                raise NotJson("key")
            check_json(item)
    elif isinstance(value, (list, tuple)):
        for item in value:
            check_json(item)
    elif isinstance(value, frozenset):
        raise NotJson("set")
    elif not (isinstance(value, (str, int, float)) or value is None):
        raise NotJson(type(value).__name__)

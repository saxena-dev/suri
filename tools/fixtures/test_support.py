"""Shared helpers for the extractor's tests (no tests here)."""

import os
import subprocess
import sys
import tempfile
import textwrap

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)

from fxcandidates import Analysis  # noqa: E402
from fxoutput import make_record  # noqa: E402

BASEURL = {
    "BROKER_SANDBOX": "https://broker-api.sandbox.alpaca.markets",
    "TRADING_PAPER": "https://paper-api.alpaca.markets",
    "DATA": "https://data.alpaca.markets",
    "TRADING_STREAM_PAPER": "wss://paper-api.alpaca.markets/stream",
}

CONFTEST = '''\
import pytest
import requests_mock


@pytest.fixture
def reqmock():
    with requests_mock.Mocker() as m:
        yield m
'''

ENUMS = '''\
from enum import Enum


class BaseURL(str, Enum):
    BROKER_SANDBOX = "https://broker-api.sandbox.alpaca.markets"
    TRADING_PAPER = "https://paper-api.alpaca.markets"
    DATA = "https://data.alpaca.markets"
    TRADING_STREAM_PAPER = "wss://paper-api.alpaca.markets/stream"
'''

LICENSE = "Apache License\nVersion 2.0, January 2004\n"


def analyse(source, path="tests/trading/test_cases.py"):
    module = path[len("tests/"):-len(".py")]
    return Analysis(path, module, textwrap.dedent(source), BASEURL)


def entries(source, path="tests/trading/test_cases.py"):
    return analyse(source, path).entries


def records(source, path="tests/trading/test_cases.py"):
    module = path[len("tests/"):-len(".py")]
    return [make_record(path, module, e)[0] for e in entries(source, path)]


def only(source, path="tests/trading/test_cases.py"):
    found = entries(source, path)
    assert len(found) == 1, [(e.kind, e.reason, e.detail) for e in found]
    return found[0]


def git_env():
    env = dict(os.environ)
    env.update(GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1",
               GIT_AUTHOR_DATE="2000-01-01T00:00:00Z", GIT_COMMITTER_DATE="2000-01-01T00:00:00Z")
    return env


def git(cwd, *args):
    return subprocess.run(["git", "-c", "user.name=Fixture Test", "-c", "user.email=test@example.invalid",
                           "-c", "init.defaultBranch=main", *args],
                          cwd=cwd, env=git_env(), check=True, capture_output=True, text=True).stdout


def make_upstream(root, tests):
    """A synthetic upstream repository holding ``tests`` ({path: source})
    plus the fixture, enum and licence files; returns its commit ID."""
    files = {"tests/conftest.py": CONFTEST, "alpaca/common/enums.py": ENUMS, "LICENSE": LICENSE}
    files.update({p: textwrap.dedent(s) for p, s in tests.items()})
    git(root, "init", "-q")
    for path, text in files.items():
        full = os.path.join(root, path)
        os.makedirs(os.path.dirname(full), exist_ok=True)
        with open(full, "w", encoding="utf-8", newline="\n") as f:
            f.write(text)
    git(root, "add", "-A")
    git(root, "commit", "-q", "-m", "upstream")
    return git(root, "rev-parse", "HEAD").strip()


def run_tool(*args):
    """Run the extractor as the maintainers do; returns (exit code, stderr)."""
    proc = subprocess.run([sys.executable, "-I", os.path.join(HERE, "extract.py"), *args],
                          env=git_env(), capture_output=True, text=True)
    return proc.returncode, proc.stderr


class TempDir:
    def __enter__(self):
        self._tmp = tempfile.TemporaryDirectory()
        return self._tmp.name

    def __exit__(self, *exc):
        self._tmp.cleanup()

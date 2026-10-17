"""The extractor end to end on synthetic upstream repositories: exit codes,
refusals, --check, the swap, determinism, LICENSE, README and redaction."""

import json
import os
import pathlib
import shutil
import socket
import subprocess
import sys
import unittest
from unittest import mock

import fxoutput
from test_support import HERE, LICENSE, TempDir, git, make_upstream, run_tool

TEST_MODULE = '''
    from alpaca.common.enums import BaseURL
    def test_order(reqmock):
        reqmock.get(
            f"{BaseURL.TRADING_PAPER.value}/v2/orders?status=open",
            text='{"id": "1", "url": "https://s3.example/x?X-Amz-Credential=AKIAABCDEFGHIJKLMNOP%2F"}',
            headers={"Authorization": "Bearer abcdefghijklmnopqrstuvwxyz0123"},
        )
        reqmock.delete(f"{BaseURL.TRADING_PAPER.value}/v2/orders", status_code=207, text="[]")
'''


class Tool(unittest.TestCase):

    def setUp(self):
        self._tmp = TempDir()
        self.tmp = self._tmp.__enter__()
        self.up = os.path.join(self.tmp, "up")
        os.mkdir(self.up)
        self.out = os.path.join(self.tmp, "fixtures", "alpaca-py")

    def tearDown(self):
        self._tmp.__exit__(None, None, None)

    def extract(self, commit, *extra):
        return run_tool("--upstream", self.up, "--commit", commit, "--out", self.out, *extra)

    def tree(self):
        return fxoutput.read_tree(self.out)

    def test_extracts_index_license_readme_and_fixtures(self):
        commit = make_upstream(self.up, {"tests/trading/test_orders.py": TEST_MODULE,
                                         "tests/live/test_live.py": "def test_x(reqmock): pass\n"})
        code, err = self.extract(commit)
        self.assertEqual(code, 0, err)
        tree = self.tree()
        self.assertEqual(tree["LICENSE"], LICENSE.encode())
        readme = tree["README.md"].decode()
        self.assertIn(commit, readme)
        self.assertIn("https://github.com/alpacahq/alpaca-py", readme)
        index = json.loads(tree["index.json"])
        self.assertEqual(list(index), ["schema", "upstream", "extractor", "sources", "excluded", "fixtures", "skipped"])
        self.assertEqual(index["upstream"]["commit"], commit)
        self.assertEqual(index["extractor"], {"path": "tools/fixtures/extract.py",
                                              "sha256": fxoutput.extractor_sha256(HERE), "python": "3.14"})
        self.assertEqual([s["path"] for s in index["sources"]],
                         ["LICENSE", "alpaca/common/enums.py", "tests/conftest.py", "tests/trading/test_orders.py"])
        self.assertEqual(index["excluded"], [{"path": "tests/live/", "qualname": None, "line": None,
                                              "reason": "live-probe"}])
        first, second = sorted(index["fixtures"], key=lambda f: f["method"])
        self.assertEqual(first["id"], "trading/test_orders::test_order::DELETE /v2/orders#0")
        self.assertEqual((first["status"], first["kind"], first["origin"]), (207, "json", "text"))
        self.assertEqual(second["file"], "trading/test_orders/test_order/get-v2-orders-0.json")
        self.assertEqual(second["query"], "status=open")
        self.assertEqual(second["family"], "trading")
        self.assertEqual(second["source"], {"path": "tests/trading/test_orders.py", "line": 4, "qualname": "test_order"})
        body = tree[second["file"]].decode()
        self.assertIn("X-Amz-Credential=" + "X" * 20 + "%2F", body)
        self.assertEqual(second["headers"], {"Authorization": "X" * len("Bearer abcdefghijklmnopqrstuvwxyz0123")})
        self.assertEqual(second["notes"], ["redacted:aws-access-key-id", "redacted:bearer-token"])
        self.assertEqual(second["bytes"], len(body.encode()))
        self.assertTrue(tree["index.json"].endswith(b"}\n"))

    def test_two_runs_are_byte_identical_and_hold_no_local_facts(self):
        commit = make_upstream(self.up, {"tests/trading/test_orders.py": TEST_MODULE})
        self.assertEqual(self.extract(commit)[0], 0)
        first = self.tree()
        self.assertEqual(self.extract(commit)[0], 0)
        self.assertEqual(self.tree(), first)
        blob = b"".join(first.values())
        for local in (self.tmp.encode(), HERE.encode(), socket.gethostname().encode(), b"2000-01-01"):
            self.assertNotIn(local, blob)

    def test_check_passes_then_fails_on_each_difference(self):
        commit = make_upstream(self.up, {"tests/trading/test_orders.py": TEST_MODULE})
        self.assertEqual(self.extract(commit)[0], 0)
        self.assertEqual(self.extract(commit, "--check")[0], 0)
        name = next(n for n in self.tree() if n.endswith(".json") and n != "index.json")
        path = os.path.join(self.out, *name.split("/"))
        original = pathlib.Path(path).read_bytes()
        for change in ("changed", "extra", "missing"):
            if change == "changed":
                pathlib.Path(path).write_bytes(original + b" ")
            elif change == "extra":
                pathlib.Path(self.out, "stray.json").write_bytes(b"{}")
            else:
                os.remove(path)
            code, err = self.extract(commit, "--check")
            self.assertEqual(code, 1, change)
            self.assertIn(change + ": ", err)
            self.assertEqual(self.extract(commit)[0], 0)

    def test_check_fails_on_a_commit_mismatch(self):
        first = make_upstream(self.up, {"tests/trading/test_orders.py": TEST_MODULE})
        self.assertEqual(self.extract(first)[0], 0)
        index = os.path.join(self.out, "index.json")
        data = json.loads(pathlib.Path(index).read_text())
        data["upstream"]["commit"] = "0" * 40
        pathlib.Path(index).write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
        code, err = self.extract(first, "--check")
        self.assertEqual(code, 1)
        self.assertIn("records commit " + "0" * 40, err)

    def assertRefused(self, commit, message, *extra):
        before = self.tree() if os.path.isdir(self.out) else None
        code, err = self.extract(commit, *extra)
        self.assertEqual(code, 2, err)
        self.assertIn(message, err)
        self.assertEqual(self.tree() if os.path.isdir(self.out) else None, before)

    def test_refusals_exit_2_and_change_nothing(self):
        first = make_upstream(self.up, {"tests/trading/test_orders.py": TEST_MODULE})
        self.assertEqual(self.extract(first)[0], 0)
        with open(os.path.join(self.up, "LICENSE"), "a") as f:
            f.write("edit\n")
        self.assertRefused(first, "modified tracked files")
        git(self.up, "checkout", "-q", "--", "LICENSE")
        pathlib.Path(self.up, "tests", "new.py").write_text("x = 1\n")
        git(self.up, "add", "-A")
        git(self.up, "commit", "-q", "-m", "second")
        self.assertRefused(first, "not " + first)
        self.assertRefused("f" * 40, "is not in the upstream repository")
        self.assertRefused("abc", "full 40-hex")
        with TempDir() as plain:
            code, err = run_tool("--upstream", plain, "--commit", first, "--out", self.out)
            self.assertEqual(code, 2)
            self.assertIn("not a Git repository", err)

    def test_index_paths_are_redacted_and_exclusions_sorted(self):
        secret = "AKIAABCDEFGHIJKLMNOP"
        commit = make_upstream(self.up, {
            "tests/trading/test_%s.py" % secret: '''
                def test_AKIAABCDEFGHIJKLMNOP(reqmock):
                    pass
                def test_AKIAABCDEFGHIJKLMNOP(reqmock):
                    reqmock.get("https://api.alpaca.markets/v2/a", text="{}")
            ''',
            "tests/broker/test_b.py": '''
                def test_z(reqmock):
                    pass
                def test_z(reqmock):
                    pass
            ''',
            "tests/live/test_live.py": "x = 1\n",
        })
        self.assertEqual(self.extract(commit)[0], 0)
        tree = self.tree()
        self.assertNotIn(secret.encode(), b"".join(tree.values()) + "".join(tree).encode())
        self.assertNotIn(secret.lower().encode(), "".join(tree).encode())
        index = json.loads(tree["index.json"])
        self.assertEqual([(e["path"], e["qualname"]) for e in index["excluded"]], [
            ("tests/broker/test_b.py", "test_z"), ("tests/live/", None),
            ("tests/trading/test_" + "X" * 20 + ".py", "test_" + "X" * 20)])

    def test_fixture_check_parse_failure_and_collision_exit_2(self):
        cases = [
            ({"tests/conftest.py": "def reqmock():\n    pass\n"}, "exactly one requests_mock.Mocker"),
            ({"tests/sub/conftest.py": "def reqmock():\n    pass\n"}, "tests/sub/conftest.py defines reqmock"),
            ({"tests/trading/test_bad.py": "def test_a(:\n"}, "does not parse"),
            ({"tests/trading/test_seg.py": '''
                def test_caf\u00e9(reqmock):
                    reqmock.get("https://api.alpaca.markets/v2/a", text="{}")
            '''}, "has a segment outside"),
            ({"tests/trading/test_c.py": '''
                def test_a(reqmock):
                    reqmock.get("https://api.alpaca.markets/v2/orders:by_x", text="{}")
                    reqmock.get("https://api.alpaca.markets/v2/orders/by_x", text="{}")
            '''}, "two fixtures map to the file"),
        ]
        for files, message in cases:
            shutil.rmtree(self.up)
            os.mkdir(self.up)
            commit = make_upstream(self.up, files)
            self.assertRefused(commit, message)

    def test_failure_before_the_swap_keeps_the_previous_output_and_leftovers_go(self):
        commit = make_upstream(self.up, {"tests/trading/test_orders.py": TEST_MODULE})
        self.assertEqual(self.extract(commit)[0], 0)
        before = self.tree()
        leftover = fxoutput.sibling(self.out)
        os.makedirs(os.path.join(leftover, "junk"))
        self.assertRefused("e" * 40, "is not in the upstream repository")
        self.assertEqual(self.tree(), before)
        self.assertFalse(os.path.exists(leftover))

    def test_swap_keeps_a_complete_tree_if_removing_the_old_one_fails(self):
        first, second = {"a.json": b"1"}, {"b.json": b"2"}
        fxoutput.install(self.out, first)
        real = shutil.rmtree

        def fail_on_old(path, *a, **kw):
            if path.endswith(".old"):
                if not kw.get("ignore_errors"):
                    raise OSError("cannot remove")
                return None
            return real(path, *a, **kw)
        with mock.patch.object(fxoutput.shutil, "rmtree", fail_on_old):
            fxoutput.install(self.out, second)  # completes: the old tree is left for the next start
        self.assertEqual(self.tree(), second)
        self.assertTrue(os.path.isdir(fxoutput.sibling(self.out, "old")))
        fxoutput.remove_leftover(self.out)
        self.assertFalse(os.path.exists(fxoutput.sibling(self.out, "old")))

    def test_usage_error_and_unexpected_exception_exit_2(self):
        self.assertEqual(run_tool("--commit", "x")[0], 2)
        commit = make_upstream(self.up, {"tests/trading/test_orders.py": TEST_MODULE})
        os.makedirs(os.path.dirname(self.out))
        pathlib.Path(self.out).write_text("a file where the directory goes")
        code, err = self.extract(commit)
        self.assertEqual(code, 2)
        self.assertIn("Traceback", err)
        self.assertEqual(pathlib.Path(self.out).read_text(), "a file where the directory goes")

    def test_another_interpreter_exits_2(self):
        script = ("import runpy, sys\nsys.version_info = (3, 13, 0, 'final', 0)\n"
                  "runpy.run_path(sys.argv[1], run_name='__main__')\n")
        proc = subprocess.run([sys.executable, "-I", "-c", script, os.path.join(HERE, "extract.py"), "--help"],
                              capture_output=True, text=True)
        self.assertEqual(proc.returncode, 2)
        self.assertIn("needs CPython 3.14, not 3.13", proc.stderr)


class ExtractorHash(unittest.TestCase):

    def test_covers_every_tool_module_and_no_test_file(self):
        with TempDir() as tmp:
            for name in os.listdir(HERE):
                if name.endswith(".py"):
                    shutil.copy(os.path.join(HERE, name), tmp)
            base = fxoutput.extractor_sha256(tmp)
            self.assertEqual(base, fxoutput.extractor_sha256(HERE))
            with open(os.path.join(tmp, "test_cases.py"), "a") as f:
                f.write("# edit\n")
            self.assertEqual(fxoutput.extractor_sha256(tmp), base)
            for module in ("fxresolve.py", "extract.py"):
                with open(os.path.join(tmp, module), "a") as f:
                    f.write("# edit\n")
                changed = fxoutput.extractor_sha256(tmp)
                self.assertNotEqual(changed, base, module)
                base = changed
            pathlib.Path(tmp, "stray.py").write_text("")
            self.assertNotEqual(fxoutput.extractor_sha256(tmp), base)


if __name__ == "__main__":
    unittest.main()

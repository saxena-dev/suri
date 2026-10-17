"""Reading the pinned upstream commit through Git objects only."""

import re
import subprocess

from fxresolve import Fail

SHA = re.compile(r"[0-9a-f]{40}")


def git(upstream, *args, binary=False):
    proc = subprocess.run(["git", "-C", upstream, *args], capture_output=True)
    if proc.returncode != 0:
        raise Fail("git " + " ".join(args) + " failed in the upstream checkout: "
                   + proc.stderr.decode("utf-8", "replace").strip())
    return proc.stdout if binary else proc.stdout.decode("utf-8")


class Upstream:
    """One commit of the upstream repository, read through Git objects."""

    def __init__(self, path, commit):
        if not SHA.fullmatch(commit):
            raise Fail("--commit must be a full 40-hex commit ID")
        probe = subprocess.run(["git", "-C", path, "rev-parse", "--git-dir"], capture_output=True)
        if probe.returncode != 0:
            raise Fail(path + " is not a Git repository")
        exists = subprocess.run(["git", "-C", path, "cat-file", "-e", commit + "^{commit}"], capture_output=True)
        if exists.returncode != 0:
            raise Fail("commit " + commit + " is not in the upstream repository")
        head = git(path, "rev-parse", "HEAD").strip()
        if head != commit:
            raise Fail("the upstream checkout is at " + head + ", not " + commit)
        if git(path, "status", "--porcelain", "--untracked-files=no").strip():
            raise Fail("the upstream checkout has modified tracked files")
        self.path = path
        self.commit = commit
        self.tree = git(path, "rev-parse", commit + "^{tree}").strip()
        self.blobs = {}
        listing = git(path, "ls-tree", "-r", "-z", commit, binary=True)
        for record in listing.split(b"\0"):
            if not record:
                continue
            meta, name = record.split(b"\t", 1)
            if meta.split()[1] == b"blob":
                self.blobs[name.decode("utf-8")] = meta.split()[2].decode("ascii")

    def read(self, path):
        if path not in self.blobs:
            raise Fail(path + " is not in the upstream commit")
        return git(self.path, "cat-file", "blob", self.blobs[path], binary=True)

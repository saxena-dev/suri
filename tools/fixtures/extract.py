"""Extract alpaca-py's mocked HTTP responses into suri's fixture tree.

Usage:
    python3.14 -I tools/fixtures/extract.py --upstream DIR --commit SHA --out DIR [--check]

Exit codes: 0 success; 1 --check found a difference; 2 any error.
"""

import sys

if sys.version_info[:2] != (3, 14):
    sys.stderr.write("extract.py needs CPython 3.14, not %d.%d\n" % sys.version_info[:2])
    sys.exit(2)

import argparse
import os
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))


def run(argv):
    parser = argparse.ArgumentParser(prog="extract.py", description=__doc__.split("\n")[0])
    parser.add_argument("--upstream", required=True, help="the alpaca-py checkout")
    parser.add_argument("--commit", required=True, help="the full commit ID to extract")
    parser.add_argument("--out", required=True, help="the fixture directory to write or check")
    parser.add_argument("--check", action="store_true", help="compare with --out instead of writing")
    args = parser.parse_args(argv)

    if HERE not in sys.path:
        sys.path.insert(0, HERE)
    import fxgit
    import fxoutput
    from fxresolve import Fail

    sys.stderr.write("python " + sys.version.split()[0] + "\n")
    try:
        if not args.check:
            fxoutput.remove_leftover(args.out)
        upstream = fxgit.Upstream(args.upstream, args.commit)
        files = fxoutput.build(upstream, HERE)
        if args.check:
            diffs = fxoutput.differences(args.out, files, args.commit)
            for line in diffs:
                sys.stderr.write(line + "\n")
            return 1 if diffs else 0
        fxoutput.install(args.out, files)
    except Fail as e:
        sys.stderr.write("error: " + str(e) + "\n")
        return 2
    fixtures = sum(1 for name in files if name not in ("index.json", "LICENSE", "README.md"))
    sys.stderr.write("wrote {} fixtures to {}\n".format(fixtures, args.out))
    return 0


def main(argv=None):
    try:
        return run(sys.argv[1:] if argv is None else argv)
    except SystemExit as e:
        return 2 if e.code not in (0, None) else 0
    except Exception:
        traceback.print_exc()
        return 2


if __name__ == "__main__":
    sys.exit(main())

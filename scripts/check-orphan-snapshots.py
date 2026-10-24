#!/usr/bin/env python3
"""Fail if a committed golden snapshot has no test function behind it.

Soroban's per-test snapshots under test_snapshots/ are gitignored; the only
committed snapshots are the few golden ones in golden_snapshots/, each
written by a test function of the same name (golden_snapshot_tests.rs).
When such a test is renamed or deleted its file would stay behind and
nothing would notice, so CI fails on it instead.
"""
import glob
import os
import re
import sys

SRC = "contracts/milestone-escrow/src/**/*.rs"
SNAPSHOTS = "contracts/milestone-escrow/golden_snapshots/*.json"


def main() -> int:
    source = ""
    for path in glob.glob(SRC, recursive=True):
        with open(path, encoding="utf-8", errors="replace") as handle:
            source += handle.read()

    defined = set(re.findall(r"\bfn\s+([A-Za-z0-9_]+)", source))

    orphans = []
    for path in sorted(glob.glob(SNAPSHOTS)):
        name = re.sub(r"\.json$", "", os.path.basename(path))
        if name not in defined:
            orphans.append((path, name))

    if not orphans:
        print("golden_snapshots: no orphans")
        return 0

    print(f"golden_snapshots: {len(orphans)} orphaned snapshot(s)\n")
    for path, name in orphans:
        print(f"  {path}\n      no `fn {name}` in {SRC}")
    print(
        "\nEither the test was renamed (delete the stale snapshot) or it was "
        "removed (restore it, or delete the snapshot deliberately)."
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())

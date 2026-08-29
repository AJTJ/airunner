#!/usr/bin/env python3
"""How much would carrying a green forward across a docs-only commit have saved? (air-0j4)

air-backlog item 1 proposes that a green recorded at G carry forward to HEAD when
`git diff --name-only G..HEAD` touches only paths a repo declares verify-irrelevant. The bead
says measure first. This is the measurement: for every pair of consecutive GREEN verifies by
the same worker, what changed between the two shas?

A pair whose diff is entirely docs is a verify that bought nothing but a re-run of the one
before it -- the cost a carry-forward would remove.

Result on 2026-08-29 over this repo's ledger (74 greens, 8 recorded days):
    65 pairs, 8 docs-only, 55 with code changed, 2 with no diff.
At a 22 s mean green verify that is about 3 minutes saved across the whole history, against a
change that makes the one refusal fail toward PERMITTING on a per-repo path list. See
docs/digests/2026-08-29-gate-air-0j4.md.

IGNORE_* below is deliberately generous: this measures the CEILING of what a carry-forward
could ever save, so a narrow list would understate the case against building it too.
"""

import collections
import sqlite3
import subprocess
import sys

DB = sys.argv[1] if len(sys.argv) > 1 else "~/projects/ai_runner/.air/ledger.db"
REPO = sys.argv[2] if len(sys.argv) > 2 else "~/projects/ai_runner"
IGNORE_PREFIXES = ("docs/", ".beads/")
IGNORE_EXACT = ("README.md", "CLAUDE.md")


def changed(a, b):
    r = subprocess.run(
        ["git", "-C", REPO, "diff", "--name-only", f"{a}..{b}"],
        capture_output=True,
        text=True,
    )
    return None if r.returncode != 0 else [p for p in r.stdout.split("\n") if p]


def irrelevant(path):
    return path.startswith(IGNORE_PREFIXES) or path in IGNORE_EXACT


rows = sqlite3.connect(f"file:{DB}?mode=ro", uri=True).execute(
    "select worker, sha, finished_at from verify_runs "
    "where kind='verify' and exit_code=0 order by worker, finished_at"
).fetchall()

by_worker = collections.defaultdict(list)
for worker, sha, at in rows:
    by_worker[worker].append((sha, at))

pairs = docs_only = code = same = unreachable = 0
for worker, runs in sorted(by_worker.items()):
    for (a, _), (b, at) in zip(runs, runs[1:]):
        pairs += 1
        if a == b:
            same += 1
            continue
        files = changed(a, b)
        if files is None:
            unreachable += 1
        elif not files:
            same += 1
        elif all(irrelevant(p) for p in files):
            docs_only += 1
            print(f"  DOCS-ONLY  {worker:10} {a[:8]}..{b[:8]}  {at[:19]}  {files}")
        else:
            code += 1

print(f"\n{len(rows)} green verifies, {pairs} consecutive same-worker pairs")
print(f"  docs-only between them:   {docs_only}")
print(f"  code changed between them:{code:4}")
print(f"  same tree / no diff:      {same}")
print(f"  sha unreachable (rewound):{unreachable:4}")

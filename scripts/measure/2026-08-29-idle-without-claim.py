#!/usr/bin/env python3
"""Was `idle-without-claim` right, at the moment it fired? (air-d10, 2026-08-29)

Air cannot see a worker that is only reading: the PreToolUse matcher is
`Edit|Write|MultiEdit|Bash`, so `Read`/`Grep`/`Glob` produce no hook event and the session
stays in whatever state its last Stop left it in. The session TRANSCRIPT does see them --
every message is appended with a timestamp -- so it is the independent check on whether a
worker Air called idle was actually working.

For each `idle-without-claim` row, this asks the only question that matters: in the threshold
window the alarm called idle, had the transcript moved?

Result on 2026-08-29 over this repo's ledger: 18 rows, 1 with a moving transcript, 17 quiet.
The adopter's false-fire premise is not reproduced here; see
docs/digests/2026-08-29-gate-air-d10.md at c80db73 (digests deleted 2026-09-25).

Do NOT measure over `first_seen -> cleared_at`: that window includes the worker resuming after
the alarm, which is the nudge working rather than the alarm being wrong. It reports 12 of 18
"active" and means nothing.
"""

import datetime as dt
import glob
import os
import sqlite3
import sys

# Run from the main checkout, or pass the ledger path as the first argument.
DB = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.getcwd(), ".air/ledger.db")
# The default in status::Thresholds::idle_noclaim_min. Override to match a tuned fleet.
THRESHOLD_MIN = int(sys.argv[2]) if len(sys.argv) > 2 else 5
# Claude Code names a project's transcript directory after its path, with "/" and "_" as "-".
_MAIN = os.path.dirname(os.path.dirname(os.path.abspath(DB)))
TRANSCRIPTS = os.path.join(
    os.path.expanduser("~/.claude/projects"),
    _MAIN.replace("/", "-").replace("_", "-") + "--claude-worktrees-{}",
)


def stamps(worker):
    """Every message timestamp in a worker's transcripts, sorted, second precision."""
    out = []
    for f in glob.glob(TRANSCRIPTS.format(worker) + "/*.jsonl"):
        for line in open(f, errors="replace"):
            i = line.find('"timestamp":"')
            if i >= 0:
                out.append(line[i + 13 : i + 32])
    out.sort()
    return out


rows = sqlite3.connect(f"file:{DB}?mode=ro", uri=True).execute(
    "select worker, first_seen, detail from conditions "
    "where kind='idle-without-claim' order by first_seen"
).fetchall()

cache, moving = {}, 0
for worker, fired, detail in rows:
    cache.setdefault(worker, stamps(worker))
    at = fired[:19]
    lo = (dt.datetime.fromisoformat(at) - dt.timedelta(minutes=THRESHOLD_MIN)).isoformat()
    inside = [s for s in cache[worker] if lo <= s <= at]
    moving += bool(inside)
    print(
        f"{worker:8} fired {at}  transcript msgs in the {THRESHOLD_MIN} min it called idle: "
        f"{len(inside):3}  [{detail}]"
    )

print(
    f"\n{len(rows)} rows: {moving} fired while the transcript was moving, "
    f"{len(rows) - moving} fired on a genuinely quiet session"
)

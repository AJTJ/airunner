#!/usr/bin/env python3
"""Count a trial's friction from its event log: agent-to-agent messages, Air's refusals, and
Air's notices. Usage: scripts/trial/count.py <trial copy> [since ISO time].

The numbers to watch across releases (owner, 2026-09-26): messages and refusals that the
scenarios did not ask for. A trial that needs more of them than the last one added friction.
"""
import collections
import glob
import json
import sys

copy = sys.argv[1]
since = sys.argv[2] if len(sys.argv) > 2 else ""
rows = [
    json.loads(line)
    for path in sorted(glob.glob(f"{copy}/.air/events/*.ndjson"))
    for line in open(path)
    if line.strip()
]
rows = [r for r in rows if r.get("at", "") >= since]

messages = []
refusals = collections.Counter()
notices = collections.Counter()
for r in rows:
    cmd = r.get("command", "")
    dec = r.get("decision", "")
    inputs = r.get("inputs") or {}
    if cmd == "hook.PreToolUse" and inputs.get("tool") == "SendMessage":
        messages.append((r["at"][11:19], r.get("worker"), inputs.get("to")))
    elif "refus" in dec or dec == "denied":
        refusals[f"{cmd} {dec}"] += 1
    elif cmd == "fanout" or dec in (
        "batch-result", "batch-ready", "batch-held", "beads-ready", "main-moved",
        "lease-free", "queue-empty", "capture",
    ):
        notices[dec or cmd] += 1

print(f"agent-to-agent messages: {len(messages)}")
for at, who, to in messages:
    print(f"  {at} {who} -> {to}")
print(f"refusals by Air: {sum(refusals.values())}")
for k, n in refusals.most_common():
    print(f"  {n:3} {k}")
print(f"notices from Air: {sum(notices.values())}")
for k, n in notices.most_common():
    print(f"  {n:3} {k}")

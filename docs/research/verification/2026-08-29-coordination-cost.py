#!/usr/bin/env python3
"""What did coordination actually cost this fleet? (air-okc)

The owner's measure for Air is "parallel agency, few collisions, MINIMAL communication". Only
the last of those was ever countable, and only from today: air-q07 put `SendMessage` in the
PreToolUse matcher, and before that the event log contained none of it. So for the rounds
already run, the session transcripts are the raw record, and this reads them directly.

Per session, from `~/.claude/projects/<slug>/*.jsonl`:

  * SendMessage tool calls made, and the bytes of the `message` argument
  * cross-session messages received, and their bytes
  * assistant turns, as a denominator

`bytes` is the payload a model wrote or read. That is the cost the owner named: every one is a
model reading prose instead of working.

**Both counts are shape-independent on purpose.** A first version matched the JSON structure of
an incoming message and reported 0 received for four sessions that had visibly received
several — the same absence-read-as-observation this bead is about. Received messages are now
counted by scanning the raw text for the delimiter, which cannot go quietly wrong. Byte counts
are of the JSON-escaped text, so they run a few percent high; they are a scale, not an audit.

Run:  python3 docs/research/verification/2026-08-29-coordination-cost.py [project-slug-prefix]
"""

import glob
import json
import os
import re
import sys

PREFIX = sys.argv[1] if len(sys.argv) > 1 else "-Users-owner-projects-ai-runner"
ROOT = os.path.expanduser("~/.claude/projects")
INCOMING = re.compile(r"<cross-session-message\b.*?</cross-session-message>", re.S)

rows = []
for d in sorted(glob.glob(os.path.join(ROOT, PREFIX + "*"))):
    name = os.path.basename(d).replace(PREFIX, "").lstrip("-") or "main"
    name = name.replace("claude-worktrees-", "")
    sent = recv = turns = sent_bytes = recv_bytes = 0
    seen = set()
    for f in glob.glob(os.path.join(d, "*.jsonl")):
        text = open(f, errors="replace").read()
        # De-duplicated by content: Claude Code re-serialises earlier context into later
        # transcript lines, so a naive count of the delimiter reported 499 receipts against
        # 193 sends — every message can only be received once.
        for m in INCOMING.finditer(text):
            body = m.group(0)
            key = hash(body)
            if key in seen:
                continue
            seen.add(key)
            recv += 1
            recv_bytes += len(body)
        for line in text.split("\n"):
            if '"assistant"' in line:
                turns += 1
            if '"SendMessage"' not in line:
                continue
            try:
                v = json.loads(line)
            except Exception:
                continue
            content = (v.get("message") or {}).get("content")
            for b in content if isinstance(content, list) else []:
                if isinstance(b, dict) and b.get("type") == "tool_use" and b.get("name") == "SendMessage":
                    sent += 1
                    sent_bytes += len(str((b.get("input") or {}).get("message", "")))
    if sent or recv:
        rows.append((name, turns, sent, sent_bytes, recv, recv_bytes))

hdr = f"{'session':12} {'turns':>6} {'sent':>5} {'sent bytes':>11} {'recv':>5} {'recv bytes':>11}"
print(hdr)
print("-" * len(hdr))
for r in sorted(rows, key=lambda r: -(r[3] + r[5])):
    print(f"{r[0]:12} {r[1]:6} {r[2]:5} {r[3]:11} {r[4]:5} {r[5]:11}")
t = [sum(r[i] for r in rows) for i in range(1, 6)]
print("-" * len(hdr))
print(f"{'ALL':12} {t[0]:6} {t[1]:5} {t[2]:11} {t[3]:5} {t[4]:11}")
print(
    f"\n{t[1] + t[3]} messages, {t[2] + t[4]} bytes of prose moved between sessions"
    f" (~{(t[2] + t[4]) // 5} words), over {t[0]} assistant turns"
)

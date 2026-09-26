#!/usr/bin/env python3
"""bd's per-process latency distribution from the event log. The median is not the question;
the tail is, because a timeout budget is a bet against the tail."""
import glob, json, os, sys, statistics

# Run from the main checkout, or pass it as the first argument.
REPO = sys.argv[1] if len(sys.argv) > 1 else os.getcwd()
files = sorted(glob.glob(os.path.join(REPO, ".air/events/*.ndjson")))
per_day = {}
for f in files:
    day = f.split("/")[-1][:-7]
    samples = []
    for line in open(f, errors="replace"):
        if '"bd_ms"' not in line:
            continue
        try:
            v = json.loads(line)
        except Exception:
            continue
        ms, calls = v.get("bd_ms"), v.get("bd_calls")
        if not isinstance(ms, int) or not isinstance(calls, int) or calls <= 0:
            continue
        each = ms // calls
        samples += [each] * min(calls, 64)
    if samples:
        per_day[day] = samples

def pct(xs, p):
    xs = sorted(xs)
    i = min(len(xs) - 1, int(len(xs) * p))
    return xs[i]

print(f"{'day':12} {'calls':>7} {'p50':>7} {'p90':>7} {'p99':>7} {'max':>8}  {'>2s':>7} {'>5s':>6} {'>10s':>6}")
allx = []
for day, xs in per_day.items():
    allx += xs
    print(f"{day:12} {len(xs):7} {pct(xs,.5):7} {pct(xs,.9):7} {pct(xs,.99):7} {max(xs):8}"
          f"  {sum(1 for x in xs if x>2000):7} {sum(1 for x in xs if x>5000):6} {sum(1 for x in xs if x>10000):6}")
print(f"{'ALL':12} {len(allx):7} {pct(allx,.5):7} {pct(allx,.9):7} {pct(allx,.99):7} {max(allx):8}"
      f"  {sum(1 for x in allx if x>2000):7} {sum(1 for x in allx if x>5000):6} {sum(1 for x in allx if x>10000):6}")
print(f"\nover 2s: {100*sum(1 for x in allx if x>2000)/len(allx):.2f}% of calls"
      f" | over 5s: {100*sum(1 for x in allx if x>5000)/len(allx):.3f}%"
      f" | over 10s: {100*sum(1 for x in allx if x>10000)/len(allx):.3f}%")

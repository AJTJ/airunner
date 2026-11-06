#!/usr/bin/env python3
"""Write the trial copy's `.claude/air.local.json` for `TRIAL_PODS=1 make trial` (air-1bn):
five targets, coordinator, lane, worker-1, worker-2 and worker-3, each on its own replica of
the owner's deployment through this checkout's scripts/kube-exec-nth, each with its own path,
and first a check that the first replica answers.

Usage: pod-targets.py SOURCE SCRIPT OUT

SOURCE is the owner's own `.claude/air.local.json` in this checkout: its first target whose
program is kube-exec-nth supplies the kubeconfig, the namespace, the deployment, the container
and the `env PATH=` tail, so no cluster value is typed twice. SCRIPT is the kube-exec-nth to
run, this checkout's. The replica numbers follow the fleet's own rule (coordinator 1, lane 2,
worker-N N+2) and the path is `<the parent of the model entry's path>/trial/<session>`, so a
trial never touches the fleet's own checkouts on the pods. Before OUT is written, the
coordinator's prefix runs `hostname` on replica 1; if that fails, the reason (kubectl's own
line and the script's) is printed, OUT is not written and the exit is 1, so `make trial` stops
before any session starts. The 2026-11-05 pod trial's copy was built by hand (air-fzl);
this is that copy's recipe. Removed when the trial no longer runs on pods.
"""
import json
import os
import subprocess
import sys

SESSIONS = [("coordinator", 1), ("lane", 2), ("worker-1", 3), ("worker-2", 4), ("worker-3", 5)]


def fail(msg):
    print(f"trial: pods: {msg}", file=sys.stderr)
    sys.exit(1)


def model_target(cfg, source):
    """The first target whose program is kube-exec-nth with its six arguments."""
    targets = cfg.get("targets") if isinstance(cfg, dict) else None
    for target in (targets or {}).values():
        if not isinstance(target, dict) or not isinstance(target.get("path"), str):
            continue
        words = target.get("exec")
        if (
            isinstance(words, list)
            and len(words) >= 7
            and os.path.basename(str(words[0])) == "kube-exec-nth"
        ):
            return target
    fail(f"{source} has no target whose exec runs kube-exec-nth with its six arguments")


def main(argv):
    if len(argv) != 4:
        print("usage: pod-targets.py SOURCE SCRIPT OUT", file=sys.stderr)
        sys.exit(2)
    source, script, out = argv[1:]
    try:
        with open(source) as f:
            cfg = json.load(f)
    except (OSError, ValueError) as e:
        fail(f"cannot read {source}: {e}")
    model = model_target(cfg, source)
    words = [str(w) for w in model["exec"]]
    values = words[1:5]
    tail = words[7:]
    base = os.path.dirname(model["path"].rstrip("/"))
    targets = {
        session: {
            "exec": [script, *values, str(n), session, *tail],
            "path": f"{base}/trial/{session}",
        }
        for session, n in SESSIONS
    }
    probe = targets["coordinator"]["exec"] + ["hostname"]
    where = " ".join(probe[:5])
    try:
        run = subprocess.run(
            probe, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=120
        )
    except OSError as e:
        fail(f"cannot run {probe[0]}: {e}")
    except subprocess.TimeoutExpired:
        fail(f"replica 1 did not answer `hostname` within 120 s through {where}")
    answer = run.stdout.strip().splitlines()
    if run.returncode != 0 or not answer:
        # kubectl's own line (an expired login, a deployment that is not there) comes before
        # the script's; both are the reason, so both are printed.
        lines = run.stderr.strip().splitlines() or [f"exit {run.returncode} with no output"]
        fail(f"replica 1 did not answer `hostname` through {where}:\n  " + "\n  ".join(lines))
    tmp = out + ".new"
    with open(tmp, "w") as f:
        json.dump({"targets": targets}, f, indent=2)
        f.write("\n")
    os.replace(tmp, out)
    print(
        f"trial: pods: replica 1 answers as {answer[-1]}; {out} names {len(targets)} targets"
        f" under {base}/trial"
    )


if __name__ == "__main__":
    main(sys.argv)

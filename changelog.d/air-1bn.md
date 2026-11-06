Added `TRIAL_PODS=1 make trial` runs the trial fleet on pods: the copy gets a gitignored
  `.claude/air.local.json` of five targets, one replica and one path per session, built by
  `scripts/trial/pod-targets.py` from the owner's own overlay through this checkout's
  `scripts/kube-exec-nth`, after replica 1 has answered `hostname`; unset, nothing changes.

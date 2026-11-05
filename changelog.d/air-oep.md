Changed `.claude/air.json` gives the coordinator, the lane and each worker a target of its
  own, each pinned to a different replica of the dev deployment: `scripts/kube-exec-nth`
  resolves the session's pod when the command runs (coordinator, lane, worker-N, sorted by
  name), and a session
  whose replica is missing fails with one line naming it. The setup guide's new section
  "One machine per session" covers VMs reachable by ssh (one entry each, no resolver) and
  replicas of one deployment (air-oep).

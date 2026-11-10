SurfaceChange {
    id: "closed-not-landed-removed",
    since: "2026-11-09 (air-4xj.3)",
    headline: "The `closed-not-landed` condition is gone from `air status`, the channel and \
               `air audit`: a worker declares done and the landing closes, so a bead a worker \
               closed is on the base branch by construction and the state it reported cannot \
               arise. `handover-not-green` is renamed `done-refused` and keyed on the same \
               counter: a worker whose `air done` was refused and has not passed since, with \
               the attempt count and the last attempt's time; a green at the head no longer \
               enters into it.",
    silent_break: true,
    action: "A script that read `closed-not-landed` reads nothing now; a bead the coordinator \
             closes on an unlanded branch is still on the `batch-ready` line as closed and \
             waiting for the lane. Read `done-refused` where you read `handover-not-green`.",
}

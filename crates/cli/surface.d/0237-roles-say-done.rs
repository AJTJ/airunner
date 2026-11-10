SurfaceChange {
    id: "roles-say-done",
    since: "2026-11-09 (air-4xj.4)",
    headline: "`.air/roles.md` and the air-* skills say what the round built: a worker or the \
               lane declares its bead done with `air done <id> --reason-file <proof>` right \
               after the commit and the precheck where the repo names one, the landing that \
               puts that head on the base branch closes the bead, and a worker's `air close` \
               is refused. The `batch green` notice is gone from the worker's list, the \
               `main moved` notice names the beads the landing closed and any still waiting, \
               and the coordinator's channel list reads `done-refused` where it read \
               `handover-not-green` and `closed-not-landed`.",
    silent_break: false,
    action: "`air install --write` refreshes `.air/roles.md`, `.air/docs/` and the air-* \
             skills; restart sessions to read them.",
}

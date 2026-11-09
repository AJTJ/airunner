SurfaceChange {
    id: "air-done",
    since: "2026-11-09 (air-4xj.1)",
    headline: "`air done <id> --reason-file <proof>` declares a bead done with proof and no green, \
               for a worker or the lane: the gate checks the bead's commit, your claim and a \
               declared precheck, the proof goes on the bead and the judged head on your claim, \
               and `air land` closes the bead once the base branch contains that head (at once \
               when it already does). The claim stays yours until then. The `air_done` MCP tool \
               is the same command. A bead nobody declared done is never closed by a landing.",
    silent_break: false,
    action: "Nothing yet: a worker's `air close` still works. Workers may run `air done` as \
             soon as the work is committed, so dependents are claimable one batch sooner.",
}

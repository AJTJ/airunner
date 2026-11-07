SurfaceChange {
    id: "closed-not-landed-names-the-cut",
    since: "2026-11-07 (air-zni)",
    headline: "The `closed-not-landed` condition's detail also says why the lane would not cut \
               the branch, when it would not: the first batch-ready fact it lacks, as `air \
               status --json` lists it under `not_batch_ready`, with its fix (`no green \
               precheck at this head ... run \\`air record precheck ...\\``). The printed line \
               gains `not cut by the lane: ...` under it, and the JSON row a `not_batch_ready` \
               string, absent when the branch is batch-ready. Pushed once per (bead, head) as \
               before.",
    silent_break: false,
    action: "Nothing.",
}

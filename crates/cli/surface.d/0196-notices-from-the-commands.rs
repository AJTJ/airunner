SurfaceChange {
    id: "notices-from-the-commands",
    since: "2026-09-29 (air-4lci)",
    headline: "`air record`, `air claim`, `air release` and `air close` queue the fleet's \
               notices after their write (beads are ready, batch-ready, the ready queue is \
               empty), so they reach workers and the lane while the coordinator's session is \
               stopped. The coordinator's channel poll still runs as a sweeper; each notice is \
               sent once.",
    silent_break: false,
    action: "",
}

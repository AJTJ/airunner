SurfaceChange {
    id: "no-chatter",
    since: "2026-09-26 (air-uzh2)",
    headline: "roles.md no longer tells a worker to message the coordinator when it closes a \
               bead; it messages only when blocked, needing a decision or finding work \
               outside its bead, and then through `air capture`. The coordinator files and \
               prioritises beads and does not message a worker to hand one out unless the \
               owner asks. `air audit` prints worker-to-coordinator messages per closed bead.",
    silent_break: true,
    action: "If the repo's own prose tells workers to report each close, or the coordinator \
             to assign beads by message, remove it.",
}

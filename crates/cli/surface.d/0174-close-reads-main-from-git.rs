SurfaceChange {
    id: "close-reads-main-from-git",
    since: "2026-09-28 (air-5ynn)",
    headline: "`air close` passes a bead whose every `Bead:` trailer commit is in main, \
               asked of git rather than of the landing row, so a close in the seconds \
               between `air land`'s fast-forward and its row is no longer refused. `batch \
               green` is read against the member's current head: it tells the member to \
               close only the beads the batch fully contains, names a bead with a later \
               commit as waiting for the next batch, and tells a member with no such bead \
               (the coordinator) that nothing is to close.",
    silent_break: false,
    action: "",
}

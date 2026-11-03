SurfaceChange {
    id: "landed-not-closed-removed",
    since: "2026-09-28 (air-cizz)",
    headline: "The `landed-not-closed` condition is gone from `air status`, the channel and \
               `air audit`, and `air land` no longer marks acceptance clauses `ok`, `MISS` or \
               `?`: it lists each landed bead with its acceptance text and no verdict. A \
               landing row's result is `landed`, never `landed-refuted`.",
    silent_break: true,
    action: "A script that read `landed-not-closed`, `landed-refuted` or the clause marks \
             reads nothing now; read the bead's acceptance in the `air land` output.",
}

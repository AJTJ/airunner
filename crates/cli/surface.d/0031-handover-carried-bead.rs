SurfaceChange {
    id: "handover-carried-bead",
    since: "2026-09-05 (air-60x)",
    headline: "`air handover <bead>` accepts a bead the branch CARRIES by a `Bead:` \
               trailer in main..HEAD, not only one the worker has claimed. A branch that \
               supersedes another worker's closed bead now has a hand-over path, and the \
               gate agrees with `air land` on what makes a branch handable.",
    silent_break: false,
    action: "Nothing to run. The claim refusal's fix is now the trailer (`Bead: <id>` on \
             the commit that does the work), never `air claim <id>`: the bead may be \
             closed and another worker's. The Stop advisory also speaks to a worker whose \
             branch carries a bead by trailer, not only to one holding a claim. Delete any \
             rule of yours that said a superseding branch must be handed over by hand.",
}

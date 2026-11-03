SurfaceChange {
    id: "land-fast-forward",
    since: "2026-08-29 (air-odv)",
    headline: "`air land` no longer merges, verifies on main, and rewinds on red. It builds \
               the landing commit off main with `git commit-tree` and fast-forwards onto \
               it, and it RUNS NO VERIFY: the branch must contain main, so that commit's \
               tree is byte-identical to the one the worker already recorded a green for. \
               No armed window, no rollback, and the dirty-main refusal went with the \
               `git reset --hard` that was its only reason.",
    silent_break: false,
    action: "If you have your own lander, this is FYI and not an instruction to adopt \
             ours: the point is that Air's changed shape, so a script that assumed `air \
             land` could leave main mid-merge, that a verify runs during a landing, or \
             that a dirty-tree refusal would stop one, is reasoning about behaviour that \
             is gone. If you do use `air land`, nothing changes at the call site - but \
             note the safety now rests entirely on the two preconditions it already \
             enforced (branch contains main, recorded green at the branch head), so do \
             not relax either.",
}

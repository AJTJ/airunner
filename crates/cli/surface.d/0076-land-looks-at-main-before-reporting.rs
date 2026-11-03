SurfaceChange {
    id: "land-looks-at-main-before-reporting",
    since: "2026-09-06 (air-htmn)",
    headline: "A fast-forward that TIMED OUT after succeeding was reported and recorded as \
               \"main is untouched\". `git merge --ff-only` updates the ref atomically and \
               Air kills the child on expiry, which does not undo a ref update, so an \
               error there means \"I stopped waiting\" and never \"it did not happen\". Air \
               now asks `merge-base --is-ancestor` and reports what it finds. The landing \
               row is the half that mattered: the incident row carried an empty \
               `merge_commit` and no `landed` row at all, so the record of that landing was \
               ABSENT and the beads it carried were attributed to nothing.",
    silent_break: true,
    action: "Nothing to run. If your landings rows have a `refused / fast-forward` with an \
             empty `merge_commit` while main carries the merge, that is this bug and the \
             beads on it are unattributed — Air has no command that records a landing after \
             the fact, so re-attribute by hand if you need it. When the look itself fails, \
             NOTHING is recorded: the in-flight row already says a landing started and has \
             not reported, which is precisely true.",
}

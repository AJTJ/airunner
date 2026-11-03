SurfaceChange {
    id: "close-asks-the-recorded-main",
    since: "2026-09-06 (air-9ij)",
    headline: "The close gate stopped expiring when main moves. `contains main` is now \
               asked of the main the verify run was RECORDED over (schema v19, \
               `verify_runs.main_sha`), not of main at the moment of the question, so a \
               landing or an ordinary commit on main no longer retracts a batch green cut \
               before it — an adopter's coordinator invalidated a whole batch with one \
               prose commit. A bead whose every commit is already in main closes on the \
               landing that put it there. `air land` is unchanged and still asks about \
               current main.",
    silent_break: false,
    action: "Nothing to run; the migration is automatic, and rows written before it fall \
             back to the old question, so re-record a green if an old one is refused. If \
             your coordinator holds main still from the batch cut until every close is \
             confirmed, that workaround can go. Landing ORDER still matters: a branch is \
             landable only while it contains current main, so every write to main, a \
             landing or the coordinator's own commit, costs every other branch its \
             landability.",
}

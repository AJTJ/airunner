SurfaceChange {
    id: "batch-members-are-what-it-took",
    since: "2026-09-06 (air-vsvt)",
    headline: "A verify run records, per worker branch, the sha the batch actually TOOK \
               from it, not that worktree's head at recording time. A worker that commits \
               between the lane's merge and the lane's `air record` used to drop out of \
               its own batch's member list, and because the list is written to the row it \
               stayed wrong. An adopter's lane saw that five times in one night. The \
               reporting path is unchanged: it always read what was recorded.",
    silent_break: false,
    action: "Nothing to run, and nothing changes for a batch whose branches sat still. \
             Member shas on `verify_runs.members` and on landing rows are now the merged \
             sha rather than a later head, so anything of yours that compared a member \
             sha against a worktree's current head will now see them differ, which is the \
             point. Rows written before this keep whatever they recorded.",
}

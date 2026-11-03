SurfaceChange {
    id: "coordinator-commits-land-with-no-bead",
    since: "2026-09-26 (0.4.4 live trial)",
    headline: "`air land --worker <lane>` lands a batch whose only non-merge commits are \
               the coordinator's (its `coordinator` worktree branch) carrying no bead, as \
               the batch-ready rule already took them. Any other commit with no `Bead:` \
               trailer is still refused, and the lane is now told to have the member add \
               a trailer and cut again rather than to `git commit --amend`.",
    silent_break: false,
    action: "",
}

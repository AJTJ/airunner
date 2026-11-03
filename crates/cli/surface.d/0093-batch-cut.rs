SurfaceChange {
    id: "batch-cut",
    since: "2026-09-25 (owner)",
    headline: "New `air batch cut`, run in the lane's worktree: the batch-ready set, oldest \
               ready first (committer time of the listed head); a `git merge-tree` pre-check \
               of each member against main and each earlier member; then `git merge main` and \
               each clean member at its listed sha, judged by `git ls-files -u` and conflict \
               markers. A conflicting member is dropped and named with the other side and the \
               paths, and each drop is an event line. `--dry-run` merges nothing. It needs \
               git 2.38 or later, and it neither verifies nor lands.",
    silent_break: false,
    action: "A lane that cuts by hand or with its own merge script can replace that with \
             `air batch cut` followed by `air record verify -- <verify command>`. Start the \
             lane's branch from main: commits already on it and not in main are carried into \
             the batch and listed as `carried`.",
}

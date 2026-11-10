SurfaceChange {
    id: "coordinator-does-not-implement",
    since: "2026-09-26 (0.4.4 live trial)",
    headline: "`.air/roles.md` tells the coordinator it does not implement: anything to \
               build, fix or write, a helper script included, goes to a worker as a bead, \
               and it commits in its own worktree only when no worker can make the change.",
    silent_break: false,
    action: "If the repo's own prose has the coordinator doing implementation work, move \
             that to beads.",
}

SurfaceChange {
    id: "coordinator-worktree",
    since: "2026-09-25 (owner rulings 2026-09-14 and 2026-09-25, air-jc2p.1)",
    headline: "`air coordinator` now creates or reuses `.claude/worktrees/coordinator` \
               (branch `worktree-coordinator`) and always starts in the tmux session \
               `<project>-coordinator`, attaching when it is already running. The \
               coordinator's session is recorded as `coordinator`, not `main`. Its branch is \
               batch-ready with no claimed bead once it has a commit main lacks, so its \
               commits reach main in the lane's batch.",
    silent_break: true,
    action: "Start the coordinator with `air coordinator` and attach with `tmux attach -t \
             <project>-coordinator`; it needs tmux. `.mcp.json` must be tracked so the \
             channel server is present in the worktree. Commit coordinator prose in its \
             worktree, not in the main checkout. Anything that looked up the coordinator's \
             session or events as `main` reads `coordinator` from now on.",
}

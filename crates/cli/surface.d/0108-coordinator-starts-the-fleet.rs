SurfaceChange {
    id: "coordinator-starts-the-fleet",
    since: "2026-09-25 (owner, air-jc2p.5)",
    headline: "`air coordinator` asks on its terminal \"Start the fleet (lane + N workers)? \
               [Y/n]\" before it starts. Yes, or the new `air fleet up`, starts the lane and \
               N workers (`\"workers\"` in `.claude/air.json`, default 3, 0 allowed) as \
               `lane` and `worker-<N>`, each in its worktree and a detached tmux session, \
               leaving one already running alone. Workers start with no task; the lane is \
               told to start its loop. `--fleet` and `--no-fleet` answer in advance; with no \
               terminal the answer is no. Workers and the lane are denied `air fleet`.",
    silent_break: false,
    action: "A script that runs `air coordinator` with a terminal passes `--no-fleet` to \
             keep today's behaviour. A repo whose workers use other names (`w1`) gets \
             `worker-<N>` sessions beside them from `air fleet up`.",
}

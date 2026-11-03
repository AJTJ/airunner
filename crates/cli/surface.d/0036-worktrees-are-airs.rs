SurfaceChange {
    id: "worktrees-are-airs",
    since: "2026-09-05 (air-fdz)",
    headline: "`air worker <name>` creates `.claude/worktrees/<name>` itself (branch \
               `worktree-<name>`) and copies the repo's `.worktreeinclude` files into it \
               before claude starts; `air worker <name> --remove` removes it, refusing \
               while it holds uncommitted work, a harness lock or a tmux session. claude \
               is still handed the worktree by name, so its isolation is unchanged.",
    silent_break: false,
    action: "Nothing to change in how you launch. Check `.worktreeinclude` still gives a \
             worktree that builds: Air matches its lines with git's own glob engine \
             (`git ls-files --ignored` over `:(glob)` pathspecs), the same files the \
             harness copied, but a negated line (`!x`) is reported and not honoured. A \
             relaunch re-copies, so a worktree gets the current `.env`. Remove lanes with \
             `air worker <name> --remove` rather than `rm -rf`; it names what is holding \
             the worktree and keeps the branch.",
}

SurfaceChange {
    id: "no-harness-worktree-flag",
    since: "2026-09-06 (air-8gj)",
    headline: "`air worker` no longer passes `--worktree` (or `--tmux`) to claude. Air \
               creates the worktree and starts claude IN it, so the harness's own worktree \
               isolation is off; one PreToolUse check replaces it, denying an \
               Edit/Write/MultiEdit whose RESOLVED path leaves the worker's worktree. In \
               The adopter's record the harness block stopped no observed write to the main \
               checkout and cost 455 refusals in five days, 388 of them (88%) with no git \
               token in the command.",
    silent_break: false,
    action: "Nothing to run. Workers gain back the operations the harness was refusing \
             (native builds, unattended commands) and lose one block; the tmux path is \
             Air's on both routes, so `AIR_TMUX_MODE` and the iTerm2 native pane are gone \
             with the flag. A Bash `cd ../..` is deliberately out of scope — the harness \
             never caught that either — so if your repo needs it, that is a cwd-scoped \
             command guard of your own.",
}

SurfaceChange {
    id: "names-carry-role",
    since: "2026-09-25 (owner, air-jc2p.4)",
    headline: "Names carry the role and the project. `air worker` with no name makes \
               `worker-<N>` (was `w<N>`); `main`, `coordinator` and `lane` are refused as \
               worker names. Every launch passes `claude --name <project>-<name>`, the same \
               string as its tmux session, so `ListAgents` and `SendMessage` use it. \
               `<project>` is `\"project\"` in `.claude/air.json` when set, else the beads \
               prefix as before. A launch whose tmux session already exists is attached to \
               when it was started in the same worktree, and refused, naming the other \
               directory, when it was not.",
    silent_break: false,
    action: "Existing `w<N>` worktrees keep working as workers; nothing to rename. A repo \
             whose project name collides with another fleet on the same machine sets \
             `\"project\"` in `.claude/air.json`. Anything that sends to a worker's session \
             by its old harness-made name uses `<project>-<name>` after a relaunch.",
}

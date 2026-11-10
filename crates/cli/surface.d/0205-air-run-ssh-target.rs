SurfaceChange {
    id: "air-run-ssh-target",
    since: "2026-11-02 (air-opq.1)",
    headline: "`air run -- <command>` runs a command on the session's target from \
               `\"targets\"` in `.claude/air.json` (kind `ssh`: push HEAD to \
               `refs/air/<session>` there, check it out, run, stream the output back), and \
               locally with nothing added when the session has none.",
    silent_break: false,
    action: "Optional: add `\"targets\": {\"<session>\": {\"kind\": \"ssh\", \"host\": …, \
             \"user\": …, \"path\": \"~/air/<project>\"}}` for sessions whose heavy or \
             Linux-only commands should run elsewhere. `air run --help` has the detail.",
}

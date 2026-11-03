Added `air run -- <command>` runs a command on the calling session's target from `"targets"` in
  `.claude/air.json` (kind `ssh`: push HEAD to `refs/air/<session>` on the host, check it out, run
  there, stream the output back, pass the exit code through), and locally with nothing added
  when the session has no target.

Fixed `air fleet up`, `air worker` and `air lane` started a second session in a worker's worktree
  when its tmux session had another name (after `"project"` changed). A worker the ledger has a
  live session for is now left running, with `<name>: running (session <id>)`.

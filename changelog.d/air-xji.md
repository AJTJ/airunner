Added `worker_allow` and `coordinator_allow` in `.claude/air.json`: permission patterns the
  launcher puts on that role's `--settings` allow list, the lane taking the worker's, as the
  two deny keys already reach its deny list. For the adopter's own machine commands, which Air
  does not name; the deny list wins where both name a command.

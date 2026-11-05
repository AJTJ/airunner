Fixed twelve landing tests failed on a host with no global git config (`commit-tree` exited
  128, author identity unknown): the shared test helper now gives every spawned process its own
  git identity and no global config, so the suite passes and fails the same on every host.

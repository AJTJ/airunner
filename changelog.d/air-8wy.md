Changed The coordinator's heartbeat is opt-in. `.air/roles.md` and the SessionStart context tell
  it to create one only where `.claude/air.json` sets `heartbeat_minutes`, and `air status`
  prints the expected interval when the key is set.

Fixed a file a command wrote in the target checkout blocked the next checkout there of a
  commit that tracks it, so the precheck or the lane's verify recorded nothing (the pod trial,
  2026-11-06): `air run` and `air record` now run `git clean -qfd` and a forced checkout before
  the command, so the target holds only commits; ignored files there stay.

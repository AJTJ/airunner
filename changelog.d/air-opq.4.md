Added `air run --pull <glob>` (repeatable): after the command exits, the matching paths under the
  target's checkout come back as one tar stream over the same ssh and are unpacked into the
  worktree, overwriting; the run's row names the pulled paths and their byte total.

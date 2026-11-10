Changed "Upgrading an existing installation" in `docs/rules/adopting-air.md` says what a repo
  with a live fleet does: in an unpinned repo the hooks and every command switch binaries the
  moment the `air` on `PATH` changes, so the coordinator stops the fleet first or the repo is
  pinned; a local edit to an `air-*` skill is overwritten; the commit on main is the owner's;
  a reused worktree needs `git merge main` for the refreshed skills; the restart is per tmux
  session; and the closing grep covers the notice phrases that changed. The sentence that a
  running session keeps the old hooks is gone.

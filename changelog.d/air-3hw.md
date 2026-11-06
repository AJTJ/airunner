Changed the trial request's last scenario: the coordinator renames the scratch copy's `main` to
  `trunk` and sets `"base_branch": "trunk"` itself, so no row of `make trial`'s request waits on
  the owner.

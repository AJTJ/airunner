Fixed a repo's `coordinator_deny` patterns were appended after the Metis flags when `metis` is
  true, where the harness read them as positional arguments and denied nothing; they now go
  among the `--disallowed-tools` values. Worker argv was already right.

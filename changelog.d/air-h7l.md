Added `base_branch` in `.claude/air.json`: the branch the fleet lands onto, `main` by
  default. Every gate, cut, landing, notice and refusal reads it where it used to say `main`,
  and names it; the session named `main` keeps its name. `air status` prints it when set and
  `--json` carries it. A selftest scenario runs a fleet whose only branch is `trunk` (air-h7l).

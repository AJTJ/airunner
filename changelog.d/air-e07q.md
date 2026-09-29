Changed `make trial` commits the scenarios' config (precheck, the `serve` lease, the precheck line in
  `CLAUDE.md`), so the trial request has no setup step and no session edits a config file.
  `air install --write --remove-stale` removes a hook `air install` reports as STALE HOOK instead
  of asking for `.claude/settings.json` to be edited by hand.

Added `air status` names each top-level key in `.claude/air.json` the running binary does not
  read, with the binary's version and build commit, and `--json` lists them as
  `unknown_config_keys`; a fleet ran a binary thirteen days behind main against a `targets` key
  and nothing said so.

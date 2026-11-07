Changed `make trial`: it marks its scratch copy trusted in Claude Code's `~/.claude.json` itself
  (`scripts/trial/trust-copy.py`, one key for that one path), so no folder-trust dialog stops the
  trial on any machine. The launcher still refuses an untrusted repository.

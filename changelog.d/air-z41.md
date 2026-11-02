Changed `air worker`, `air lane`, `air coordinator` and `air fleet up` pre-answer the start-up
  prompts Claude Code documents an answer for (`--add-dir <main checkout>`, `--permission-mode
  auto`, `skipAutoPermissionPrompt`) and refuse to launch, naming the fix, when the main checkout
  has no `hasTrustDialogAccepted` in `~/.claude.json`. Air never writes that key.

SurfaceChange {
    id: "launch-no-startup-prompts",
    since: "2026-11-02 (air-z41)",
    headline: "`air worker`, `air lane`, `air coordinator` and `air fleet up` pre-answer the \
               start-up prompts Claude Code documents an answer for: `--add-dir <main \
               checkout>` (so reading `.air/` never prompts), `--permission-mode auto` and \
               `skipAutoPermissionPrompt` on `--settings`. Before starting anything they \
               check `hasTrustDialogAccepted` for the main checkout in `~/.claude.json` \
               (under `CLAUDE_CONFIG_DIR` when set) and refuse with the fix when it is \
               absent; Air never writes that key.",
    silent_break: false,
    action: "If a launch is refused for folder trust, run `claude` once in the main \
             checkout and answer Yes. Restart running sessions to pick up the flags.",
}

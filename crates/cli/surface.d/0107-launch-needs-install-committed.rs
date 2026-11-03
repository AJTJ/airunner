SurfaceChange {
    id: "launch-needs-install-committed",
    since: "2026-09-25 (owner, air-rr98)",
    headline: "`air worker` and `air lane` refuse, naming the files, while `air install`'s \
               output (`.claude/settings.json`, `.mcp.json`, `.claude/skills/air-*`) has \
               uncommitted changes in the main checkout. A new worktree gets only \
               committed files, so such a session ran without hooks, channel or skills.",
    silent_break: false,
    action: "Commit those files on main after every `air install --write`.",
}

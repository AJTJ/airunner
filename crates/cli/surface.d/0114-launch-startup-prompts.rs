SurfaceChange {
    id: "launch-startup-prompts",
    since: "2026-09-25 (air-oe9k)",
    headline: "Every launched session approves the `air` server from `.mcp.json` in its own \
               `--settings` (`enabledMcpjsonServers`), so the \"New MCP server found\" \
               question, whose default dropped the channel, is gone. `air coordinator` and \
               `air fleet up` print once the prompts that remain: folder trust until the \
               main checkout is trusted (answer yes; one yes covers every worktree) and the \
               coordinator's development-channels warning.",
    silent_break: false,
    action: "",
}

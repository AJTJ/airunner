SurfaceChange {
    id: "coordinator-deny-placement",
    since: "2026-11-06 (air-b70)",
    headline: "A repo's `coordinator_deny` patterns from `.claude/air.json` are inserted among \
               the coordinator's `--disallowed-tools` values. Before, with `\"metis\": true` \
               the argv ended with `--mcp-config <path>`, the patterns were appended after it, \
               the harness read them as positional arguments, and they denied nothing. A \
               worker's argv already ended with its deny values, so `worker_deny` is unchanged.",
    silent_break: true,
    action: "If your coordinator relied on a `coordinator_deny` pattern that never applied, it \
             applies from this version; `air coordinator --print` shows the argv.",
}

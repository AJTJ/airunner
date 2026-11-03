SurfaceChange {
    id: "mcp-tool-budgets",
    since: "2026-09-25 (air-se4n)",
    headline: "`air mcp` no longer cuts every tool off at 20 s. A tool whose command runs bd \
               at the full bd budget gets one more of those than its command can spend, \
               from the same function: close 2 x (60 s + 5 s per id), triage and handover \
               2 x 65 s, release 3 x 65 s, claim 7 x 65 s, all capped 10 s under Claude \
               Code's MCP_TOOL_TIMEOUT (default 300 s). Status, attention, holdings, inbox \
               and capture stay at 20 s. A timed-out tool now kills its whole \
               process group, so no bd process it started keeps running.",
    silent_break: false,
    action: "",
}

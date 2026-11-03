SurfaceChange {
    id: "land-waits-only-on-runs-at-risk",
    since: "2026-09-28 (air-9yx6)",
    headline: "`air land` refuses only while a verify runs in the main checkout or the \
               lane's tree. A worker's verify in its own worktree no longer holds a landing, \
               and `--despite-inflight` records only the runs a landing can destroy.",
    silent_break: false,
    action: "",
}

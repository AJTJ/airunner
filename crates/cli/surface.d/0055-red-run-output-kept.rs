SurfaceChange {
    id: "red-run-output-kept",
    since: "2026-09-06 (air-5ik)",
    headline: "`air record` keeps what a NON-GREEN run printed: the last 64 KiB, both \
               streams in arrival order, at `.air/logs/<run-id>.log`, with the path on \
               `verify_runs.log_path` — a column that has existed since schema v1 and was \
               NULL on every row ever written. `air record` prints the path and \
               `air status` puts it beside a worker's `not green`. A green run writes \
               nothing.",
    silent_break: false,
    action: "Nothing to run; `.air/` is already gitignored. The store is bounded at write \
             time by COUNT — 20 logs, so 1.25 MB at most, ever — and prunes itself, so \
             unlike `.air/events/` there is nothing to collect and no `gc` window to \
             choose. If your build prints more than 64 KiB you get its tail, which is \
             where a verify fails.",
}

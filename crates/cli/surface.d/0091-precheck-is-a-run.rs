SurfaceChange {
    id: "precheck-is-a-run",
    since: "2026-09-25 (owner)",
    headline: "`air record precheck -- <cmd>` records a worker's precheck like a verify, and \
               it is never read as a verify green. A running one is shown as `precheck in \
               flight` and keeps the worker from reading as idle; it does not hold a landing. \
               With `\"precheck\": true` in `.claude/air.json`, a branch is batch-ready only \
               with a green precheck at its head (`no-precheck` otherwise).",
    silent_break: false,
    action: "Under a lane, run the repo's precheck through `air record precheck --`. To have \
             the lane cut only prechecked heads, set `\"precheck\": true` and retire any log \
             file or \"checked at <sha>\" message the lane reads for it.",
}

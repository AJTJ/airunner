SurfaceChange {
    id: "air-ui",
    since: "2026-11-10 (air-o6x.1)",
    headline: "`air ui [--port N]` serves a read-only view of the fleet to a browser on \
               127.0.0.1, port 7311 by default. It opens the ledger read-only, writes nothing, \
               and runs from `/` so it is no worktree's reader. This release ships the \
               skeleton: a placeholder page and five JSON endpoints in their final shapes, \
               with only the sessions filled.",
    silent_break: false,
    action: "None. Run it from the main checkout or any worktree; pick another port with \
             --port if 7311 is taken.",
}

SurfaceChange {
    id: "session-liveness",
    since: "2026-09-28 (air-sai7)",
    headline: "A session is alive only when its pid runs with the start time it registered \
               with and is not a zombie; `air status --json` gives `liveness` (alive, dead or \
               unknown) per session in place of `pid_alive`. Dead and unknown sessions hear \
               no ready notice and raise no idle-without-claim. A new session under a name \
               replaces that name's earlier rows, and the main checkout's session shows as \
               the owner, never a coordinator.",
    silent_break: true,
    action: "A script reading `pid_alive` from `air status --json` reads nothing: read \
             `liveness`.",
}

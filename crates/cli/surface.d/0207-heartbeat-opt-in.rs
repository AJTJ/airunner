SurfaceChange {
    id: "heartbeat-opt-in",
    since: "2026-11-02 (air-8wy)",
    headline: "The coordinator's 5-minute heartbeat is opt-in: `.air/roles.md` and the \
               SessionStart context tell it to create one only where `.claude/air.json` \
               sets `heartbeat_minutes`, and `air status` names the interval.",
    silent_break: true,
    action: "If you want the heartbeat that used to start by default, add \
             `\"heartbeat_minutes\": 5` to `.claude/air.json`. Without it a limited \
             coordinator session is not woken by a timer; `air status` still shows \
             `STOPPED at <t>`.",
}

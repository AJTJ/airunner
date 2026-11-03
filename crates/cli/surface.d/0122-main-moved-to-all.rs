SurfaceChange {
    id: "main-moved-to-all",
    since: "2026-09-26 (air-1vri.4)",
    headline: "When `air land` moves main, the coordinator and every worker hear \"main \
               moved to <sha>: landed <beads>; files changed: <paths>\" once per landing \
               (event line `fanout / main-moved`). The lane that ran the landing is not \
               told again. A member's copy also says its beads are in and may be closed, \
               and replaces the separate `landed in main` message, which is gone. \
               `.air/roles.md` says what it means for each role; it needs no reply.",
    silent_break: false,
    action: "",
}

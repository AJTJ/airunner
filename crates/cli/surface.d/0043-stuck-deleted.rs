SurfaceChange {
    id: "stuck-deleted",
    since: "2026-09-06 (air-12k)",
    headline: "The `stuck` session state and attention condition are gone: no session \
               reads it, no condition raises it, `air audit` no longer lists it, and \
               `AIR_ATTENTION_STUCK_MIN` does nothing. It was set only by the \
               PermissionRequest \
               hook, which never arrives in auto mode — zero in 39,071 event lines.",
    silent_break: true,
    action: "A script or dashboard that filters `air status --json` for `stuck` now reads \
             an empty set and says nobody is stuck, which is true and useless: it was \
             already always empty. Replace it with the coordinator's heartbeat — \
             `air status` on a timer — which is what actually caught every wedged worker; \
             `.air/roles.md` says so, and `air install --write` refreshes it.",
}

SurfaceChange {
    id: "coordinator-autobatch-off",
    since: "2026-11-03 (air-m8u)",
    headline: "The coordinator's branch is batch-ready only at a head it offered with `air \
               ready` (new, coordinator only; `air status --json` says `not-offered` until \
               then). Before, every coordinator commit main lacked was cut and verified by the \
               lane at once.",
    silent_break: true,
    action: "To keep the old behaviour, set `\"coordinator_autobatch\": true` in \
             `.claude/air.json`. Otherwise the coordinator runs `air ready` when its branch is \
             ready for the lane; `.air/roles.md` says so.",
}

SurfaceChange {
    id: "epic-ready-to-decompose",
    since: "2026-09-06 (air-84u)",
    headline: "`air status` names each ready epic that has no open child, as `epic ready \
               to decompose: <id> (0 open children, N closed)`, and `.air/roles.md` states \
               decomposition as the coordinator's standing duty rather than as a property \
               of a good queue. Nothing refuses and nothing pushes; the count was already \
               printed, this says which epic it is about.",
    silent_break: false,
    action: "Nothing to run; `air install --write` refreshes `.air/roles.md`. It costs one \
             `bd list --parent` per READY epic, so a tick with no ready epic pays nothing \
             and the cost only appears in the state the line exists to report.",
}

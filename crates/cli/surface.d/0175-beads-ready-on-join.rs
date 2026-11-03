SurfaceChange {
    id: "beads-ready-on-join",
    since: "2026-09-28 (air-oerm)",
    headline: "A worker that joins, or whose claim ends, while beads are ready is told \
               `beads are ready` with the set as it stands; before, the notice went out only \
               when the set gained a bead, so a fleet started over a full queue sat idle. \
               This includes the channel's first tick, which used to only record the set. \
               No worker is told the same set twice.",
    silent_break: false,
    action: "",
}

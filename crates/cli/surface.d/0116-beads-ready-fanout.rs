SurfaceChange {
    id: "beads-ready-fanout",
    since: "2026-09-26 (air-dkm1)",
    headline: "When the claimable ready set gains a bead, the coordinator's channel tells \
               every live, idle worker holding no claim \"beads are ready: <ids>\", once per \
               change (one `fanout / beads-ready` event line). The lane and workers holding \
               a claim are not told. It asks bd for the ready list at most once a minute. \
               The `idle-without-claim` condition no longer ends \"prompt them\". \
               `.air/roles.md` says what the message means.",
    silent_break: false,
    action: "",
}

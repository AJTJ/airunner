SurfaceChange {
    id: "roles-in-flight-additions-and-corrections",
    since: "2026-11-09 (air-bpn)",
    headline: "The coordinator's section of `.air/roles.md` says how to reach a worker about a \
               bead it holds now. An addition is a bead, filed as usual, plus a `SendMessage` \
               to the holder so it can fold the work into its branch; a correction goes as a \
               `SendMessage` first and is checked as received, and becomes a bead when it was \
               not. A hold sent only as a message arrived after the lane had cut the commit.",
    silent_break: false,
    action: "`air install --write` refreshes `.air/roles.md`; restart the coordinator to read it.",
}

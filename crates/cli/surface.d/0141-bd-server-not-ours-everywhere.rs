SurfaceChange {
    id: "bd-server-not-ours-everywhere",
    since: "2026-09-26 (owner ruling)",
    headline: "Every session Air launches has `BEADS_DOLT_AUTO_START=0`, so bd never starts its \
               own Dolt server. `air status`, `air doctor` and the coordinator's keep-alive \
               report a port answered by another process as not this project's server rather \
               than up; the keep-alive starts nothing there and tells the coordinator once. \
               Workers and the lane are also denied `bd ready --claim`, `bd orphans --fix` \
               and `bd events prune`; plain `bd ready` stays allowed.",
    silent_break: false,
    action: "If told the port is held by another process, stop that process and run \
             `air bd-server up`.",
}

SurfaceChange {
    id: "closed-bead-batch-ready",
    since: "2026-09-28 (air-voeb)",
    headline: "A branch is batch-ready when its commits name a bead its worker holds OR one \
               that is closed through `air close`. Before, a worker that closed on a green \
               and moved on held nothing its branch named, so a branch that main then moved \
               past was neither landable nor batch-ready. `air status` prints such a bead as \
               `<id>: closed, waiting for the lane`, and `air status --json` lists it under \
               `batch_ready[].closed`.",
    silent_break: false,
    action: "",
}

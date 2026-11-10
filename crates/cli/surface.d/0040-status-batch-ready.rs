SurfaceChange {
    id: "status-batch-ready",
    since: "2026-09-05 (air-80x.3)",
    headline: "`air status` lists the branches a verify lane may merge into its next \
               batch, as `batch-ready: <worker> at <sha> (<beads>)`: head contains main, \
               no green at that head, a `Bead:` trailer names a bead the worker holds. \
               `--json` carries `batch_ready` and `not_batch_ready` with the reason.",
    silent_break: false,
    action: "Nothing to run. A verify lane scripts its merge from `air status --json`'s \
             `batch_ready`; nothing pushes it, and a branch that is already green is \
             landable instead and never listed here.",
}

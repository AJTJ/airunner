SurfaceChange {
    id: "failed-cut-and-interrupted-run",
    since: "2026-09-28 (air-nwat)",
    headline: "`air batch cut` writes `batch-cut / failed` with its reason when it stops part \
               way, naming who holds a git lock. `air record` no longer dies on SIGINT, SIGTERM \
               or SIGHUP: it stops the check and writes the run with `failing_step` \
               `interrupted (<signal>)`, and `air status` shows `last verify interrupted ...` \
               under the worker.",
    silent_break: false,
    action: "",
}

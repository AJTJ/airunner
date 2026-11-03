SurfaceChange {
    id: "batch-cut-split",
    since: "2026-09-29 (air-5fo5)",
    headline: "`air batch cut --without <worker>...` leaves named workers' branches out of the \
               cut, and `--only <worker>...` takes just those, so a split after a red batch is \
               an ordinary recorded cut (members in batch_cuts) rather than a hand merge. The \
               output names who was left out (`left_out` in `--json`); leaving everyone out \
               is an empty cut and exits 1. The lane's roles text says it may split a red batch that way.",
    silent_break: false,
    action: "",
}

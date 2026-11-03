SurfaceChange {
    id: "batch-cut-records-members",
    since: "2026-09-26 (0.4.6 live trial, D2 and D3)",
    headline: "`air batch cut` records the members it merged in the ledger (schema v24, table \
               `batch_cuts`), and `air record` reads them, so a batch of one tells its member \
               the result. A branch whose head a red batch was recorded at is not batch-ready \
               (`red-at-head` in `air status --json`) until it has a new commit.",
    silent_break: false,
    action: "A worker whose batch went red fixes it with a new commit; the lane no longer cuts the \
             same red head again.",
}

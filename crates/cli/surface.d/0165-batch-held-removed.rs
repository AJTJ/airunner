SurfaceChange {
    id: "batch-held-removed",
    since: "2026-09-28 (air-aepb)",
    headline: "`air batch cut` no longer sends a `batch-held` notice to a worker whose branch \
               has no green precheck, and writes no `batch-cut / no-precheck` line. The \
               branch is still left out, and `air status` still names `no-precheck` for it.",
    silent_break: false,
    action: "",
}

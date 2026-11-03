SurfaceChange {
    id: "precheck-hold-rereads",
    since: "2026-09-26 (0.4.9 live trial)",
    headline: "`air batch cut` re-reads the precheck record for a head immediately before \
               writing a `batch-held` notice, so a precheck recorded green while the cut was \
               running is not reported as missing.",
    silent_break: false,
    action: "",
}

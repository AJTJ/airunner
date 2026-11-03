SurfaceChange {
    id: "batch-events-pushed",
    since: "2026-09-26 (air-1vri.2)",
    headline: "The lane is told each branch that becomes batch-ready, and each member \
               worker is told its batch's result at once: `batch green ... Close them now` \
               from `air record`, `batch red` with the exit and the kept output, `landed in \
               main` from `air land`, and `dropped from batch` with the conflict from `air \
               batch cut` (event lines `fanout / batch-ready` and `fanout / batch-result`). \
               `air land` and a red batch's `air record` now end with the batch-ready set \
               and `next: air batch cut`, or `nothing is batch-ready`; a green batch's ends \
               with `next: air land --worker <lane>`, and `air land --json` carries \
               `batch_ready`. `air status` prints `loops (24 h):` with the median wait from \
               batch-ready to its batch verify and from batch green to close.",
    silent_break: false,
    action: "",
}

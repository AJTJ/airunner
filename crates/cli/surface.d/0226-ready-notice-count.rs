SurfaceChange {
    id: "ready-notice-count",
    since: "2026-11-06 (air-bcg)",
    headline: "The ready notice to a worker holding no claim reads `<n> bead(s) are claimable; \
               run \\`bd ready\\``, a count and no ids. The list it carried was stale by the \
               time a worker read it (five refused claims in one trial), and the rotation of \
               that list per recipient (`beads-ready-rotated`) goes with it. The roles text \
               says so.",
    silent_break: false,
    action: "Nothing. A script matching `beads are ready: <ids>` in deliveries matches the \
             count form instead.",
}

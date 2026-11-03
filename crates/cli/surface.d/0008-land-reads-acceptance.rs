SurfaceChange {
    id: "land-reads-acceptance",
    since: "2026-08-22 (air-ayp)",
    headline: "`air land` closes nothing. The worker closes its own bead with proof; the \
               landing prints each bead beside its acceptance and Air's verdict.",
    silent_break: true,
    action: "A repo whose landing pass assumed \"merged means closed\" will now find beads \
             merged and still open, because closing moved to the worker. The adopter closed \
             99 beads on branch containment alone, 14 partial and 1 not done. The close \
             reason is PROOF - a command and its output, a file:line, a passing test - and \
             the hand-over gate already covers `bd close`.",
}

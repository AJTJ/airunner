SurfaceChange {
    id: "dropped-head-not-batch-ready",
    since: "2026-09-26 (0.4.9 live trial)",
    headline: "A branch `air batch cut` dropped for a conflict is not batch-ready again at \
               that head (`dropped-at-head` in `air status --json`) until its worker commits. \
               The drop notice, and the cut's output, name the conflicting branch and its sha: \
               against main, `git merge main` now; against a peer's branch or the batch, \
               wait until it lands, then `git merge main` and resolve, or resolve against \
               main if it never lands. The cut no longer prints `Name each drop to its \
               worker`; the notice already does.",
    silent_break: false,
    action: "",
}

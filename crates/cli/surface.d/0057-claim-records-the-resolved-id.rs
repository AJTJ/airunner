SurfaceChange {
    id: "claim-records-the-resolved-id",
    since: "2026-09-06 (air-x1ha)",
    headline: "`air claim` records the id BD RESOLVED, not the string that was typed, so a \
               prefix claim and a full-id claim produce identical rows. And the status \
               reconcile now tells an id bd never had from one bd no longer holds: only \
               the second releases the row, and a kept one is reported by id. Before this, \
               a worker typed a prefix, bd claimed the full id, Air's row went under the \
               prefix, and the next reconcile released the claim while the work continued.",
    silent_break: false,
    action: "Nothing to run, and nothing is released that was not released before — this \
             only stops releases. If you have rows recorded under a prefix from before, \
             `air status` now names them under \"kept N claim(s) bd could not resolve\" \
             instead of silently dropping them: re-claim under the id bd knows, or \
             `air release <id> --reason unknown`. A `bd show` that TIMES OUT no longer \
             releases anything either, which it used to for every claim not in the \
             in-progress list.",
}

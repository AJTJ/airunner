SurfaceChange {
    id: "audit-reclaim-churn",
    since: "2026-09-06 (air-5nh)",
    headline: "`air audit` prints a `re-claim churn` section from the `claims` table: \
               claims in the window, how many released, how many inside 60 s and 300 s, \
               how many of those ended owner-gated, and the rows those counts read. \
               Nothing new is recorded.",
    silent_break: false,
    action: "Nothing to run; the numbers come from claims your ledger already holds, so \
             the first run covers your whole history if you pass `--since`. Read the rate \
             as what it says: owner-gated releases inside a minute over claims, which is \
             \"a worker took a bead it could not start\". The 10% threshold printed \
             beside it is the boundary that would reopen wrapping bd's filing with an \
             ordering edge; below it the ordering edge stays prose. If your workers do \
             not release with a reason naming the owner, the rate under-reads, which is \
             the safe direction but worth knowing.",
}

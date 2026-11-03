SurfaceChange {
    id: "undischarged-clause-wording",
    since: "2026-09-06 (air-k6uh, air-jy99)",
    headline: "The `landed-not-closed` line no longer says a merge CONTRADICTS a clause, \
               and no longer tells you to reopen the bead. It reports what the lookup \
               established — an acceptance clause naming a file the merge did not change — \
               and hands the decision back. Both renderers changed: `air status` and `air \
               land`'s summary.",
    silent_break: true,
    action: "Two things. Anything of yours matching `CONTRADICTS` or `reopen` in this \
             line's text stops matching; the condition kind (`landed-not-closed`) and the \
             ledger row are unchanged, so match on the kind. And re-read any bead you \
             acted on because of this alert: of nine standing firings here, SIX were \
             clauses that held — three satisfied in a commit other than the merge, three \
             naming a path the clause only mentions. The line asserted a contradiction it \
             had not established. It also prescribed reopening, which a flow that says \
             \"closed is closed\" forbids; Air reads no repo's flow and now names no \
             action here, so what to do about an undischarged clause is yours to decide.",
}

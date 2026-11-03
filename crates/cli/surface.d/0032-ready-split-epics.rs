SurfaceChange {
    id: "ready-split-epics",
    since: "2026-09-05 (air-f10)",
    headline: "The claimable count excludes epics and the `ready:` line names them apart \
               (`N epic(s) to decompose, not claimable`); `air claim <epic>` is refused. \
               The Stop nudge and `idle-without-claim` read the same split.",
    silent_break: true,
    action: "Your `ready:` line's claimable number may drop, and that is the true count: \
             `2 claimable` read as two workers' worth of work when both were containers. \
             A rule or script that claims an epic to \"own\" it now gets a refusal; \
             decompose it with `bd create` children instead. Nothing else to run.",
}

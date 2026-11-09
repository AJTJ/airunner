SurfaceChange {
    id: "red-batch-names-untested-members",
    since: "2026-11-09 (air-67q)",
    headline: "After a red batch of several members, `air record` and `air status` list the \
               members not yet verified alone, one line each, and end with `next: air batch \
               cut --only <first>`, until every member has been cut alone or a later green \
               carries it (owner ruling 2026-11-09: when a whole batch is red, each member is \
               tested individually). The red line now ends `the lane verifies each member \
               alone` where it said the lane splits the batch by hand, and the members' notice \
               and `.air/roles.md` say the same. `air batch cut --only` and `--without` choose \
               among every branch held by a red batch, not the newest red batch's members \
               only, so the cut Air prints after a member has gone red alone is not refused.",
    silent_break: false,
    action: "Nothing to run. If you run a lane, cut the member each `next:` line names until \
             the list is empty; `air install --write` refreshes `.air/roles.md`.",
}

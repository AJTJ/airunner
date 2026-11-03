SurfaceChange {
    id: "budgets-measured",
    since: "2026-09-06 (air-d75)",
    headline: "Every timing budget Air waits on records its elapsed time and whether it \
               was hit, in a `budgets` object on the event line; `air audit` prints per \
               budget the count, p50/p90/p99/max, hits and near misses with the fail \
               direction beside each, plus an unpaired-hook count for the one budget that \
               cannot record its own overruns. The SQLite busy timeout moves 200 ms -> 1 s \
               and is now a recording `busy_handler`.",
    silent_break: false,
    action: "Nothing to run. Event lines gain one optional key, so a reader that ignores \
             unknown fields is unaffected. Read `air audit`'s budget rows once a round: a \
             non-zero `hits` on `git` or `sqlite-lock` is a hook that failed open, which \
             is a refusal that did not happen. The harness's own Bash timeout is the one \
             budget Air cannot record; see adopting-air.md §1 step 5.",
}

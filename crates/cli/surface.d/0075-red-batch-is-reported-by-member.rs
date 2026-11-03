SurfaceChange {
    id: "red-batch-is-reported-by-member",
    since: "2026-09-05 (air-80x.4), unbounded 2026-09-06 (air-cyf)",
    headline: "A red verify at a batch head is reported BY MEMBER: `air record` prints \
               `batch red at <sha> (<lane>): members …; nothing lands on it, the lane \
               splits by hand`, and `air status` repeats the newest standing one until a \
               later green carries every member. air-cyf then removed the 20-run window \
               that decided \"standing\": a batch that stayed red across 20 further runs \
               silently stopped being reported, and a dropped report looked exactly like a \
               fixed one.",
    silent_break: false,
    action: "Nothing to run, and nothing lands, closes or claims differently on a red. If \
             you run a lane, expect the line to persist rather than age out — that is the \
             fix, not noise, and it clears when a green carries every member.",
}

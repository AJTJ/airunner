SurfaceChange {
    id: "holdings-tense",
    since: "2026-09-05 (air-v7o)",
    headline: "`air holdings` tags name their tense: `uncommitted now, edited 3 min ago`, \
               `uncommitted now, no edit journaled` (build or test output, with `verify in \
               flight` when one is running), `journaled 6 h ago, clean now`. The report \
               carries `at`, and `air status`'s `overlap:` lines print the same tags.",
    silent_break: true,
    action: "A script matching the old `[uncommitted]` / `[journaled]` tokens, or reading \
             `overlaps` from `air status --json` as bare worker names, sees the new \
             strings. `air holdings --json` gained `at`, `last_edit` and \
             `verify_in_flight`. Read the tense: `uncommitted now` is true of `at` only; \
             `journaled` is history with its age.",
}

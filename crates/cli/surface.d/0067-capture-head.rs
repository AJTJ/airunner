SurfaceChange {
    id: "capture-head",
    since: "2026-09-07 (air-6dj4)",
    headline: "A capture records the worker's HEAD sha at the time it was written, and \
               `air inbox` renders it (`<id>  <time>  <worker> at <sha8>  <text>`). Ledger \
               schema v21 adds `captures.head_sha` and `captures.head_absent`.",
    silent_break: true,
    action: "Re-read `air inbox` output if anything of yours parses it by column: the \
             worker field is now followed by ` at <sha8>`, or by ` [no head: <why>]` where \
             Air looked and found none, or by nothing at all on a row written before v21. \
             Those three are deliberately distinct — a row from before this change did not \
             have a head looked up, which is a different claim from having none, and \
             collapsing them would make old rows assert something nobody established. \
             `--json` gains `head`, either `{\"At\": \"<sha>\"}` or `{\"Absent\": \
             \"<why>\"}` or null. Why it exists: a capture's time is on the row and its \
             subject is in the body, and the body is what gets quoted onward, so \"the \
             batch is red\" used to arrive somewhere else with no way to say which batch.",
}

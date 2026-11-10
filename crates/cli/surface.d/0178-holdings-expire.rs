SurfaceChange {
    id: "holdings-expire",
    since: "2026-09-28 (air-84lz)",
    headline: "A recorded edit (a row in `edit_journal`) ends once its file is clean in the \
               holder's worktree and the holder's branch does not change it against main, \
               and for a holder with no worktree. `air holdings` and `air status` delete \
               ended rows when they read, and `air land` after it moves main, so the peer \
               warning no longer names edits that have landed or sessions that are gone. \
               `journaled ..., clean now` now appears only beside `committed`.",
    silent_break: false,
    action: "",
}

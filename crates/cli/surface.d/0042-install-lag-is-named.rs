SurfaceChange {
    id: "install-lag-is-named",
    since: "2026-09-06 (air-d61)",
    headline: "`air doctor` and `air status` print one line when `.air/installed.json` \
               records an older crate or surface version than the running binary: both \
               versions, the count of unread notices, and the fix. The adopter's hooks ran \
               0.2.18 for days on a record that said 0.1.0 / surface 2, with the ledger \
               already migrated and the installed skills stale, and nothing said so. \
               Printed, never refused; a repo with no record at all stays silent.",
    silent_break: false,
    action: "If the line appears, run `air install` to read the notices, then \
             `air install --write`; the line goes away with the record.",
}

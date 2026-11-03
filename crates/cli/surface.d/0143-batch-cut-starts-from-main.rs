SurfaceChange {
    id: "batch-cut-starts-from-main",
    since: "2026-09-26 (0.4.8 live trial, workaround 1)",
    headline: "`air batch cut` resets the lane's branch to main before merging, so a red \
               batch's merges no longer carry into the next batch. It refuses, changing \
               nothing, while the lane has uncommitted changes or while its head has a \
               recorded green that main does not contain (land it first). `--dry-run` names \
               the head it would reset from; in `--json`, `reset_from` replaces `carried`. `.air/roles.md` no longer lets the lane hold a \
               bead between batches, since its own commits do not survive a cut.",
    silent_break: true,
    action: "The lane stops resetting its branch by hand. Run `air install --write` to \
             refresh `.air/roles.md`.",
}

SurfaceChange {
    id: "closed-not-landed",
    since: "2026-09-07 (air-gazh)",
    headline: "New attention condition `closed-not-landed`: a bead bd has closed whose \
               commits are in no tree but its author's worktree. Pushed on the channel and \
               printed by `air status` (`closed, not landed: <worker> (<bead>) at <sha>`), \
               with `closed_not_landed` in `--json`.",
    silent_break: false,
    action: "Nothing to run. Read it if your fleet BATCHES: the state forms with no error \
             anywhere in it — a worker closes on a batch green, the coordinator commits to \
             main, that batch stops containing main, later cuts red or are killed, and the \
             branch simply never lands. An adopter had six at once, found while answering \
             an unrelated question. Nothing looks wrong from inside the worktree (bead \
             closed, branch green, tree clean), which is why the person best placed to \
             notice is the last who will. A fleet that lands each branch soon after its \
             close never reaches it, and a quiet line here measures your landing cadence \
             rather than the check.",
}

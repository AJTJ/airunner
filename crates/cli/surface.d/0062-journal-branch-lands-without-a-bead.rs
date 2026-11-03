SurfaceChange {
    id: "journal-branch-lands-without-a-bead",
    since: "2026-09-06 (air-kexg)",
    headline: "A branch whose only commits are session-journal entries (everything under \
               the repo's `journal_dir`) lands with no `Bead:` trailer. A journal entry is \
               not work on a bead, so it is the one commit a worker legitimately writes \
               that names none, and such a branch was refused with \"no commit declares a \
               bead\". A range MIXING journal commits with anything else is unchanged and \
               still needs a trailer.",
    silent_break: true,
    action: "If anything of yours reads `air status --json`'s `landable`, its `bead` field \
             is now `null` for a journal-only branch rather than always a string; every \
             other row is unchanged. A repo that declares no `journal_dir` has no journal \
             case and nothing changes for it. `air land <bead-id>` cannot select a journal \
             branch, since there is no id to name it by — use `--worker` or `--all`.",
}

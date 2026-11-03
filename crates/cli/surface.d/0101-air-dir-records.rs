SurfaceChange {
    id: "air-dir-records",
    since: "2026-09-25 (owner, air-1qnp)",
    headline: "Air's generated records go in the main checkout's gitignored `.air/`. \
               Session journals default to `.air/journal/` (`journal_dir` is now an \
               override), and `air init` no longer scaffolds `docs/journal/` or writes \
               `journal_dir`. New opt-in `\"digests\": true` in `.claude/air.json` makes the \
               close gate require a digest in `.air/digests/`, checked for existence rather \
               than git tracking. The worktree fence lets a worker write under those two \
               directories. A repo with `digest_dir` keeps the tracked check unchanged; one \
               with neither key still needs no digest.",
    silent_break: false,
    action: "Nothing required. To move journals out of your tree, drop `journal_dir` from \
             `.claude/air.json` and move the files into `.air/journal/`; a repo that wants \
             digests without committing them replaces `digest_dir` with `\"digests\": true`. \
             Both directories live on the machine running the fleet and have no git \
             history.",
}

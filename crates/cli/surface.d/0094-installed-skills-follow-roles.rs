SurfaceChange {
    id: "installed-skills-follow-roles",
    since: "2026-09-25 (air-vuwx)",
    headline: "The installed skills agree with `.air/roles.md`. `air-phase-transitions` is no \
               longer installed: its `awaiting_review` step, close-after-landing and \
               re-verifying landing contradicted the protocol. `air-decomposition` drops \
               per-worker queues and the `next` subcommand, which never existed. The \
               CLAUDE.md that `air init` scaffolds no longer says the hand-over flow is the \
               repo's own; it points at roles.md and keeps only what is the repo's.",
    silent_break: true,
    action: "`air install --write` removes `.claude/skills/air-phase-transitions/` (see \
             `install-removes-retired-skills`). If this repo's CLAUDE.md restates a close or hand-over sequence, cut it to \
             what roles.md leaves to the repo: the verify and precheck commands, worktree \
             setup, shared resources and domain rules.",
}

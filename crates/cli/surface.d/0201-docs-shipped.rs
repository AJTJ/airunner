SurfaceChange {
    id: "docs-shipped",
    since: "2026-09-29 (air-g9dx)",
    headline: "`air install --write` (and `air init --write`) writes `.air/docs/design.md`, \
               `.air/docs/solved-issues.md` and `.air/docs/adopting-air.md` from the copies \
               in the binary, and installs the `air-defect` skill, which reads them. \
               `.air/roles.md` names the first two for every role. `air doctor` names a copy \
               that is missing or differs.",
    silent_break: false,
    action: "`air install --write` writes them; `.air/` is already gitignored, so nothing \
             new to commit except `.claude/skills/air-defect/`.",
}

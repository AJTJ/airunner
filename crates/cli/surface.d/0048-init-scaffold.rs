SurfaceChange {
    id: "init-scaffold",
    since: "2026-09-06 (air-ej4)",
    headline: "`air init --write` now scaffolds four empty-but-ready things a fresh repo \
               needs and Air used to assume: a `Makefile` with a `verify` target that \
               FAILS until it is edited, a `.worktreeinclude` with a comment header, and \
               in the `CLAUDE.md` stub the hand-over sequence and the `Bead: <id>` trailer \
               rule. Each is created ONLY when absent and never edited, and `air init` \
               without `--write` lists what it would create.",
    silent_break: false,
    action: "Nothing changes for a repo that already has these: a present `Makefile`, \
             `.worktreeinclude` or `CLAUDE.md` is not touched, and a `Makefile` with no \
             `verify` target is REPORTED and still not edited. If you scaffold a new repo, \
             know that its `make verify` exits 1 on purpose until you put a real check in \
             it; that is what stops the first `air record verify` recording a green for an \
             empty check.",
}

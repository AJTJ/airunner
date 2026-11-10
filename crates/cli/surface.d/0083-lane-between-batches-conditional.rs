// ---- 2026-09-07, air-bh6n --------------------------------------------------------
// Three notices the sweep owed and did not cut, because it excluded `roles.md` by CLASS:
// prose, in a file that ships whole, refreshed by `air install --write`. That reasoning is
// about the ARTIFACT and presupposes the adopter runs install — which under a freeze they
// do not, and nothing prompts them to. The test that should have been applied is about the
// EFFECT: does a reader of the shipped text now do, or believe they may do, something
// different? `roles.md` is embedded in the binary and written to `.air/roles.md`, so a
// change to it that alters a permission or a duty is a surface change like any other.
SurfaceChange {
    id: "lane-between-batches-conditional",
    since: "2026-09-06 (air-4noi), notice cut 2026-09-07 (air-bh6n)",
    headline: "The verify lane's permission to hold a bead BETWEEN batches is conditional: \
               it may, `if its worktree survives the cut`. A lane that resets hard to main \
               on every cut would hold that bead in a tree it is about to wipe; a lane that \
               merges main forward and integrates on a throwaway branch keeps it. Which of \
               the two a repo runs is its own flow, and Air reads neither.",
    silent_break: true,
    action: "Read `.air/roles.md`'s verification-lane section again, and if it says only \
             \"between batches it may hold one\" with no condition, your copy predates \
             this and your lane has been reading an unconditional permission. **The \
             decision is yours and you may not have known it was**: whether your lane can \
             hold a bead between batches depends on how your lane makes a batch contain \
             main. `air install --write` refreshes the file; if you are under an install \
             freeze, read this notice as the change itself. Nothing refuses either way.",
}

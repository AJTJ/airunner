SurfaceChange {
    id: "bd-pin-1-3-0",
    since: "2026-09-26 (owner ruling)",
    headline: "Air now expects bd 1.3.0, not 1.2.2: `air doctor` compares `bd --version` \
               with 1.3.0 and reports 1.2.2 as off the pin. Known difference: bd 1.3.0 \
               refuses `bd update <id> -s open -a \"\"` on a bead another actor holds \
               in_progress, which is the reopen `air release` issues; a fix is pending.",
    silent_break: false,
    action: "`brew unpin beads && brew upgrade beads`, then `bd --version` should print \
             1.3.0. The store migrates on first use. Pin again with `brew pin beads` if \
             you pinned before.",
}

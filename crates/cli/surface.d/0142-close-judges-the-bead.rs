SurfaceChange {
    id: "close-judges-the-bead",
    since: "2026-09-26 (owner ruling)",
    headline: "`air close <bead>` and `air handover --bead <bead>` no longer refuse a bead as \
               behind main when every commit naming it in a `Bead:` trailer has landed or is \
               in a recorded green over the main it ran on, even if the branch has newer \
               commits for the next bead. A bead with no trailer gets the head check as before. \
               `.air/roles.md` tells a worker it may start the next bead at once and close the \
               earlier one when it passes.",
    silent_break: false,
    action: "Run `air install --write` to refresh `.air/roles.md`.",
}
